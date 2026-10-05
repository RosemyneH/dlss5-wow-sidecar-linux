use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sidecar_capture::{start_capture, wow_window_hint};
use sidecar_config::{matching_preset, parse_config, PRESETS};
use sidecar_core::{list_wow_windows, smart_scan_installs, SmartScanOptions};
use sidecar_runtime::{control_socket_path, is_running, read, send, SidecarCommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "wowsidecar-linux",
    about = "Linux-native WoW sidecar (capture + neural overlay — work in progress)"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Find WoW install folders (Wow.exe / WowB.exe) on disk
    Scan {
        #[arg(long, action = clap::ArgAction::Append)]
        root: Vec<PathBuf>,
    },
    /// List compositor windows that look like WoW (Hyprland / Sway)
    Windows,
    /// Quick environment check
    Doctor,
    /// Capture frames from portal ScreenCast (or pw-record fallback)
    CaptureTest {
        #[arg(long, default_value_t = 10)]
        frames: u32,
        /// Do not prefer the first detected WoW window as a capture hint
        #[arg(long)]
        any_window: bool,
    },
    /// Run overlay daemon in this process (same as `wowsidecar-daemon`)
    Run,
    /// Foreground daemon for daily use (same as `run`; see README user flow)
    Serve,
    /// Spawn `wowsidecar-daemon` if not running
    Start,
    /// Tell daemon to stop
    Stop,
    /// Query daemon status
    Status {
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("wowsidecar_linux=info".parse()?),
        )
        .init();

    match Cli::parse().command {
        Commands::Scan { root } => {
            let opts = SmartScanOptions { extra_roots: root };
            let installs = smart_scan_installs(&opts);
            if installs.is_empty() {
                println!("no WoW installs found");
                return Ok(());
            }
            for (i, install) in installs.iter().enumerate() {
                let tag = if i == 0 { " (best)" } else { "" };
                println!(
                    "{}{}\n  client: {}\n  rank: {}\n",
                    install.game_dir.display(),
                    tag,
                    install.client_exe,
                    install.branch_rank
                );
            }
        }
        Commands::Windows => {
            let windows = list_wow_windows();
            if windows.is_empty() {
                println!("no WoW-like windows (start the game in borderless/windowed)");
                return Ok(());
            }
            for w in windows {
                println!(
                    "[{}] {} | class={} | {}x{}@({},{}) fs={}\n  id={}",
                    w.compositor,
                    w.title,
                    w.class,
                    w.width,
                    w.height,
                    w.x,
                    w.y,
                    w.fullscreen,
                    w.address
                );
            }
        }
        Commands::Doctor => {
            println!(
                "session: {}",
                std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "?".into())
            );
            let installs = smart_scan_installs(&SmartScanOptions::default());
            println!("installs found: {}", installs.len());
            if let Some(best) = installs.first() {
                println!(
                    "best install: {} ({})",
                    best.game_dir.display(),
                    best.client_exe
                );
            }
            let windows = list_wow_windows();
            println!("wow windows: {}", windows.len());
            println!("pipewire: {}", which("pw-dump"));
            println!("hyprctl: {}", which("hyprctl"));
            println!("portal: {}", which("xdg-desktop-portal"));
            let (cfg, _) = parse_config("");
            let preset_name = matching_preset(&cfg)
                .map(|i| PRESETS[i].name)
                .unwrap_or("custom");
            println!(
                "\nconfig: preset={} neural_passes={} (1x=1, 2x=2, 3x=3 passes)",
                preset_name, cfg.neural_passes
            );
            println!("neural pass runtime: not implemented yet (see docs/ROADMAP.md)");
            println!("capture: docs/CAPTURE.md (`wowsidecar-linux capture-test`)");
            println!(
                "runtime ipc: {} ({})",
                if is_running() {
                    "daemon up"
                } else {
                    "daemon down"
                },
                control_socket_path().display()
            );
        }
        Commands::CaptureTest { frames, any_window } => {
            run_capture_test(frames, any_window)?;
        }
        Commands::Run => sidecar_runtime::run_daemon()?,
        Commands::Serve => {
            eprintln!(
                "wowsidecar-linux serve: control socket {}",
                control_socket_path().display()
            );
            eprintln!("stop with: wowsidecar-linux stop (or Ctrl+C in this terminal)");
            sidecar_runtime::run_daemon()?
        }
        Commands::Stop => {
            if send(SidecarCommand::Stop) {
                println!("stop sent");
            } else {
                println!("no daemon listening at {}", control_socket_path().display());
            }
        }
        Commands::Status { json } => {
            if !is_running() {
                println!("daemon not running ({})", control_socket_path().display());
                return Ok(());
            }
            let Some(st) = read() else {
                println!("daemon did not return status");
                return Ok(());
            };
            if json {
                println!("{}", serde_json::to_string_pretty(&st)?);
            } else {
                println!(
                    "pid={} overlay={} hud={} fps={:.1} capture_fps={:.1} frames={} variant={}",
                    st.process_id,
                    st.overlay_visible,
                    st.hud_visible,
                    st.fps,
                    st.capture_fps,
                    st.frames,
                    st.runtime_variant
                );
            }
        }
        Commands::Start => {
            if is_running() {
                println!(
                    "daemon already running ({})",
                    control_socket_path().display()
                );
                return Ok(());
            }
            let daemon = daemon_exe_path()?;
            if !daemon.is_file() {
                anyhow::bail!(
                    "missing {} — build with: cargo build -p sidecar-runtime",
                    daemon.display()
                );
            }
            std::process::Command::new(&daemon)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .context("spawn wowsidecar-daemon")?;
            for _ in 0..20 {
                if is_running() {
                    println!("daemon started ({})", control_socket_path().display());
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            anyhow::bail!("daemon did not bind control socket in time");
        }
    }

    Ok(())
}

fn run_capture_test(frames: u32, any_window: bool) -> Result<()> {
    let hint = if any_window { None } else { wow_window_hint() };
    if let Some(h) = &hint {
        println!(
            "hint: [{}] {} ({}) — select this window in the portal dialog",
            h.compositor, h.title, h.address
        );
    } else {
        println!("no WoW window hint; pick any window in the portal dialog");
    }

    let stream = start_capture(hint).context("start_capture")?;
    for i in 0..frames {
        let frame = stream
            .next_frame_timeout(Duration::from_secs(60))
            .with_context(|| format!("frame {i}"))?;
        if !frame.validate() {
            anyhow::bail!("frame {i}: invalid RGBA buffer");
        }
        println!(
            "frame {}: {}x{} rgba_bytes={} timestamp={}",
            i,
            frame.width,
            frame.height,
            frame.rgba.len(),
            frame.timestamp
        );
    }
    println!("captured {} frame(s)", frames);
    Ok(())
}

fn daemon_exe_path() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("current_exe")?;
    let daemon = exe
        .parent()
        .map(|d| d.join("wowsidecar-daemon"))
        .unwrap_or_else(|| PathBuf::from("wowsidecar-daemon"));
    Ok(daemon)
}

fn which(cmd: &str) -> String {
    match std::process::Command::new("which").arg(cmd).output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "missing".into(),
    }
}
