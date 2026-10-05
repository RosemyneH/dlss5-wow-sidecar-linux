use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tracing::{debug, info};

use crate::fps::FpsCounter;
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
            if control::ping_socket(&socket_path)? {
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

        let mut status = SidecarStatus::default();
        status.process_id = std::process::id();
        status.runtime_variant = "linux-stub".into();
        status.pass_name = "stub".into();

        Ok(Self {
            listener,
            socket_path,
            status: Arc::new(Mutex::new(status)),
            overlay_visible: Arc::new(Mutex::new(true)),
            hud_visible: Arc::new(Mutex::new(false)),
            stop_flag: Arc::new(Mutex::new(false)),
            handler: Arc::new(handler),
        })
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

    pub fn tick_fps_stubs(&self, capture: &mut FpsCounter, overlay: &mut FpsCounter) {
        let capture_fps = capture.stub_pulse();
        let overlay_fps = overlay.stub_pulse();
        let mut status = self.status.lock().unwrap().clone();
        status.capture_fps = capture_fps;
        status.fps = overlay_fps;
        status.frames = status.frames.saturating_add(1);
        self.publish(status);
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
            ControlRequest::Start => ControlResponse::Ack { ok: true },
            ControlRequest::Stop => {
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
                *self.stop_flag.lock().unwrap() = true;
            }
            SidecarCommand::ShowOverlay => *self.overlay_visible.lock().unwrap() = true,
            SidecarCommand::HideOverlay => *self.overlay_visible.lock().unwrap() = false,
            SidecarCommand::ShowHud => *self.hud_visible.lock().unwrap() = true,
            SidecarCommand::HideHud => *self.hud_visible.lock().unwrap() = false,
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

pub mod control {
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

    pub fn send_at(path: &Path, command: SidecarCommand) -> anyhow::Result<bool> {
        match request_at(path, ControlRequest::Command { command })? {
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
    let mut capture_fps = FpsCounter::new(Duration::from_secs(1));
    let mut overlay_fps = FpsCounter::new(Duration::from_secs(1));

    while !server.should_stop() {
        while server.pump_once()? {}
        server.tick_fps_stubs(&mut capture_fps, &mut overlay_fps);
        std::thread::sleep(Duration::from_millis(16));
    }

    info!("daemon stopping");
    Ok(())
}

pub fn ensure_runtime_dir() -> anyhow::Result<()> {
    fs::create_dir_all(runtime_dir())?;
    Ok(())
}

pub use control::{is_running, read, read_at, send, send_at, start_daemon, status, stop};
