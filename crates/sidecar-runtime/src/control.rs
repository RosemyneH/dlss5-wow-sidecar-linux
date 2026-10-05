use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sidecar_probes::query_gpu_memory;
use tracing::{debug, info};

use crate::pipeline::Pipeline;
use crate::protocol::{ControlRequest, ControlResponse, SidecarCommand, SidecarStatus};
use crate::socket_path::{control_socket_path, runtime_dir};

pub type CommandHandler = Box<dyn Fn(SidecarCommand) + Send + Sync + 'static>;

pub struct ControlServer {
    listener: UnixListener,
    socket_path: std::path::PathBuf,
    status: Arc<Mutex<SidecarStatus>>,
    overlay_visible: Arc<Mutex<bool>>,
    hud_visible: Arc<Mutex<bool>>,
    stop_flag: Arc<Mutex<bool>>,
    pipeline_stop: Arc<AtomicBool>,
    pipeline: Arc<Mutex<Pipeline>>,
    handler: Arc<CommandHandler>,
}

impl ControlServer {
    pub fn create(handler: CommandHandler) -> anyhow::Result<Self> {
        Self::create_at(control_socket_path(), handler)
    }

    pub fn create_at(
        socket_path: std::path::PathBuf,
        handler: CommandHandler,
    ) -> anyhow::Result<Self> {
        if socket_path.exists() {
            if client::ping_socket(&socket_path)? {
                anyhow::bail!(
                    "another overlay daemon already owns {}",
                    socket_path.display()
                );
            }
            let _ = fs::remove_file(&socket_path);
        }
        if let Some(parent) = socket_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let listener = UnixListener::bind(&socket_path)?;
        listener.set_nonblocking(true)?;

        let status = SidecarStatus {
            process_id: std::process::id(),
            runtime_variant: "linux-pipeline".into(),
            pass_name: "idle".into(),
            ..SidecarStatus::default()
        };

        let overlay_visible = Arc::new(Mutex::new(true));
        let status = Arc::new(Mutex::new(status));
        let pipeline_stop = Arc::new(AtomicBool::new(false));
        let pipeline = Arc::new(Mutex::new(Pipeline::new(
            overlay_visible.clone(),
            status.clone(),
            pipeline_stop.clone(),
        )));

        Ok(Self {
            listener,
            socket_path,
            status,
            overlay_visible,
            hud_visible: Arc::new(Mutex::new(false)),
            stop_flag: Arc::new(Mutex::new(false)),
            pipeline_stop,
            pipeline,
            handler: Arc::new(handler),
        })
    }

    pub fn pipeline(&self) -> Arc<Mutex<Pipeline>> {
        self.pipeline.clone()
    }

    pub fn pipeline_stop_requested(&self) -> bool {
        self.pipeline_stop.load(Ordering::Acquire)
    }

    pub fn poll_vram_into_status(&self) {
        let mut status = self.status.lock().unwrap();
        if let Some(mem) = query_gpu_memory() {
            status.vram_used_mb = mem.used_mb;
            status.vram_budget_mb = mem.total_mb;
            status.sequence = status.sequence.wrapping_add(1);
        }
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub fn should_stop(&self) -> bool {
        *self.stop_flag.lock().unwrap()
    }

    pub fn stop_flag(&self) -> Arc<Mutex<bool>> {
        self.stop_flag.clone()
    }

    pub fn publish(&self, mut status: SidecarStatus) {
        let mut slot = self.status.lock().unwrap();
        let next = slot.sequence.wrapping_add(1);
        status.sequence = next;
        status.process_id = std::process::id();
        status.overlay_visible = u32::from(*self.overlay_visible.lock().unwrap());
        status.hud_visible = u32::from(*self.hud_visible.lock().unwrap());
        *slot = status;
    }

    pub fn pump_once(&self) -> anyhow::Result<bool> {
        match self.listener.accept() {
            Ok((stream, _)) => {
                self.handle_client(stream)?;
                Ok(true)
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    fn handle_client(&self, stream: UnixStream) -> anyhow::Result<()> {
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let req: ControlRequest = serde_json::from_str(line.trim()).unwrap_or(ControlRequest::Ping);
        let resp = self.dispatch(req);
        let mut stream = reader.into_inner();
        writeln!(stream, "{}", serde_json::to_string(&resp)?)?;
        stream.flush()?;
        Ok(())
    }

    fn dispatch(&self, req: ControlRequest) -> ControlResponse {
        match req {
            ControlRequest::Ping => ControlResponse::Pong,
            ControlRequest::GetStatus | ControlRequest::Status => ControlResponse::Status {
                status: self.status.lock().unwrap().clone(),
            },
            ControlRequest::Start => {
                let ok = self.pipeline.lock().unwrap().start().is_ok();
                ControlResponse::Ack { ok }
            }
            ControlRequest::Stop => {
                self.pipeline.lock().unwrap().stop();
                let ok = self.apply_command(SidecarCommand::Stop);
                ControlResponse::Ack { ok }
            }
            ControlRequest::Command { command } => {
                let ok = self.apply_command(command);
                ControlResponse::Ack { ok }
            }
        }
    }

    fn apply_command(&self, command: SidecarCommand) -> bool {
        debug!(?command, "control command");
        match command {
            SidecarCommand::None => return true,
            SidecarCommand::Stop => {
                self.pipeline.lock().unwrap().stop();
                *self.stop_flag.lock().unwrap() = true;
            }
            SidecarCommand::ShowOverlay => *self.overlay_visible.lock().unwrap() = true,
            SidecarCommand::HideOverlay => *self.overlay_visible.lock().unwrap() = false,
            SidecarCommand::ShowHud => *self.hud_visible.lock().unwrap() = true,
            SidecarCommand::HideHud => *self.hud_visible.lock().unwrap() = false,
            SidecarCommand::Panic => {
                *self.overlay_visible.lock().unwrap() = false;
                *self.hud_visible.lock().unwrap() = false;
                self.pipeline_stop.store(true, Ordering::Release);
                self.pipeline.lock().unwrap().stop();
            }
        }
        self.sync_status_flags();
        (self.handler)(command);
        true
    }

    fn sync_status_flags(&self) {
        let mut status = self.status.lock().unwrap();
        status.overlay_visible = u32::from(*self.overlay_visible.lock().unwrap());
        status.hud_visible = u32::from(*self.hud_visible.lock().unwrap());
        status.sequence = status.sequence.wrapping_add(1);
    }
}

impl Drop for ControlServer {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.socket_path);
    }
}

pub mod client {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::time::Duration;

    use crate::protocol::{ControlRequest, ControlResponse, SidecarCommand, SidecarStatus};
    use crate::socket_path::control_socket_path;

    pub fn is_running() -> bool {
        let path = control_socket_path();
        ping_socket(&path).unwrap_or(false)
    }

    pub fn ping_socket(path: &Path) -> anyhow::Result<bool> {
        match request_at(path, ControlRequest::Ping) {
            Ok(ControlResponse::Pong) => Ok(true),
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub fn send(command: SidecarCommand) -> bool {
        send_at(&control_socket_path(), command).unwrap_or(false)
    }

    pub fn send_toggle(command: SidecarCommand) -> bool {
        let resolved = match command {
            SidecarCommand::HideOverlay => {
                let visible = read().map(|s| s.overlay_visible != 0).unwrap_or(true);
                if visible {
                    SidecarCommand::HideOverlay
                } else {
                    SidecarCommand::ShowOverlay
                }
            }
            SidecarCommand::HideHud => {
                let visible = read().map(|s| s.hud_visible != 0).unwrap_or(false);
                if visible {
                    SidecarCommand::HideHud
                } else {
                    SidecarCommand::ShowHud
                }
            }
            other => other,
        };
        send(resolved)
    }

    pub fn send_at(path: &Path, command: SidecarCommand) -> anyhow::Result<bool> {
        match request_at(path, ControlRequest::Command { command })? {
            ControlResponse::Ack { ok } => Ok(ok),
            _ => Ok(false),
        }
    }

    pub fn start_overlay() -> anyhow::Result<bool> {
        start_overlay_at(&control_socket_path())
    }

    pub fn start_overlay_at(path: &Path) -> anyhow::Result<bool> {
        match request_at(path, ControlRequest::Start)? {
            ControlResponse::Ack { ok } => Ok(ok),
            _ => Ok(false),
        }
    }

    pub fn read() -> Option<SidecarStatus> {

        read_at(&control_socket_path()).ok().flatten()
    }

    pub fn read_at(path: &Path) -> anyhow::Result<Option<SidecarStatus>> {
        match request_at(path, ControlRequest::GetStatus)? {
            ControlResponse::Status { status } => Ok(Some(status)),
            ControlResponse::Error { message } => anyhow::bail!(message),
            _ => Ok(None),
        }
    }

    pub fn stop() -> bool {
        send(SidecarCommand::Stop)
    }

    pub fn status() -> anyhow::Result<Option<SidecarStatus>> {
        read_at(&control_socket_path())
    }

    pub fn start_daemon(exe: &Path) -> anyhow::Result<()> {
        if is_running() {
            return Ok(());
        }
        Command::new(exe)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        for _ in 0..20 {
            if is_running() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        anyhow::bail!("daemon did not bind control socket in time");
    }

    fn request_at(path: &Path, req: ControlRequest) -> anyhow::Result<ControlResponse> {
        if !path.exists() {
            anyhow::bail!("no daemon at {}", path.display());
        }
        let mut stream = UnixStream::connect(path)?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        writeln!(stream, "{}", serde_json::to_string(&req)?)?;
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        Ok(serde_json::from_str(line.trim())?)
    }

    pub fn socket_path() -> PathBuf {
        control_socket_path()
    }
}

pub fn run_daemon_loop(server: &ControlServer) -> anyhow::Result<()> {
    info!("control socket: {}", server.socket_path().display());
    let _ = server.pipeline().lock().unwrap().start();
    let mut last_vram_poll = Instant::now() - Duration::from_secs(10);

    while !server.should_stop() {
        while server.pump_once()? {}
        if last_vram_poll.elapsed() >= Duration::from_secs(2) {
            server.poll_vram_into_status();
            last_vram_poll = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(16));
    }

    server.pipeline().lock().unwrap().stop();
    info!("daemon stopping");
    Ok(())
}

pub fn ensure_runtime_dir() -> anyhow::Result<()> {
    fs::create_dir_all(runtime_dir())?;
    Ok(())
}

pub use client::{is_running, read, read_at, send, send_at, send_toggle, start_daemon, start_overlay, start_overlay_at, status, stop};
