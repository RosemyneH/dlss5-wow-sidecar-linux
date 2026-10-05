use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sidecar_capture::{capture_doctor_report, format_doctor_line};
use sidecar_config::{matching_preset, neural_strength_of, parse_config, PRESETS};
use sidecar_core::{list_wow_windows, smart_scan_installs, SmartScanOptions};
use sidecar_neural::processor_id_for_config;
use sidecar_runtime::{control_socket_path, is_running, read, send, send_toggle, SidecarCommand};
use tracing_subscriber::EnvFilter;

mod capture_test;

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
        /// Seconds to wait for each frame (first frame includes the portal picker)
        #[arg(long, default_value_t = 60)]
        timeout: u64,
        /// Use synthetic frames to verify the tooling without portal/PipeWire
        #[arg(long)]
        mock: bool,
        /// Print every frame instead of only the summary
        #[arg(long, short)]
        verbose: bool,
    },
    /// Run overlay daemon in this process (same as `wowsidecar-daemon`)
    Run,
    /// Foreground daemon for daily use (same as `run`; see README user flow)
    Serve,
    /// Spawn `wowsidecar-daemon` if not running
    Start,
    /// Tell daemon to stop
    Stop,
    /// Send control command (for compositor binds); see docs/HOTKEYS.md
    Send { command: String },
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
            println!(
                "portal: {}",
                sidecar_capture::resolve_portal_frontend_binary()
                    .unwrap_or_else(|| "missing".into())
            );
            println!("\ncapture (P06):");
            for line in capture_doctor_report() {
                println!("  {}", format_doctor_line(&line));
            }
            let fixes = capture_test::doctor_fixes();
            if !fixes.is_empty() {
                println!("\ncapture fixes:");
                for fix in fixes {
                    println!("  - {fix}");
                }
            }
            let (cfg, _) = parse_config("");
            let preset_name = matching_preset(&cfg)
                .map(|i| PRESETS[i].name)
                .unwrap_or("custom");
            let backend = processor_id_for_config(&cfg);
            let strength = neural_strength_of(&cfg)
                .map(|s| s.name().to_string())
                .unwrap_or_else(|| format!("custom ({} passes)", cfg.neural_passes));
            println!(
                "\nconfig: preset={} neural_backend={} neural_pass={} neural_passes={}",
                preset_name, backend, cfg.neural_pass, cfg.neural_passes
            );
            println!("neural strength: {}", strength);
            let neural_runtime = if is_running() {
                read()
                    .map(|st| format!("daemon active (live pass={})", st.pass_name))
                    .unwrap_or_else(|| "daemon up (status unavailable)".into())
            } else {
                "daemon stopped — pipeline reloads config each capture frame when running".into()
            };
            println!("neural runtime: {}", neural_runtime);
            println!("capture docs: docs/CAPTURE.md");
            println!(
                "runtime ipc: {} ({})",
                if is_running() {
                    "daemon up"
                } else {
                    "daemon down"
                },
                control_socket_path().display()
            );
            println!(
                "hotkeys: overlay={} hud={} start_stop={} panic=Ctrl+Alt+Backspace (evdev panic-only default; see docs/HOTKEYS.md)",
                if cfg.hotkeys.toggle_overlay.is_empty() {
                    "(off)"
                } else {
                    cfg.hotkeys.toggle_overlay.as_str()
                },
                if cfg.hotkeys.toggle_hud.is_empty() {
                    "(off)"
                } else {
                    cfg.hotkeys.toggle_hud.as_str()
                },
                if cfg.hotkeys.start_stop.is_empty() {
                    "(off)"
                } else {
                    cfg.hotkeys.start_stop.as_str()
                },
            );
            println!("compositor binds: docs/hyprland-hotkeys.conf");
        }
        Commands::CaptureTest {
            frames,
            any_window,
            timeout,
            mock,
            verbose,
        } => capture_test::run(&capture_test::CaptureTestOptions {
            frames,
            any_window,
            timeout: Duration::from_secs(timeout),
            mock,
            verbose,
        })?,
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
        Commands::Send { command } => {
            let Some(cmd) = SidecarCommand::from_cli_name(&command) else {
                anyhow::bail!("unknown command: {command} (see docs/HOTKEYS.md)");
            };
            let ok = if cmd.is_toggle() {
                send_toggle(cmd)
            } else {
                send(cmd)
            };
            if ok {
                println!("{} sent", command);
            } else if is_running() {
                println!("daemon rejected {}", command);
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
                let vram = if st.vram_budget_mb > 0 {
                    format!(" vram={}/{}MiB", st.vram_used_mb, st.vram_budget_mb)
                } else {
                    String::new()
                };
                println!(
                    "pid={} overlay={} hud={} fps={:.1} capture_fps={:.1} frames={} pass={}{}{}variant={}",
                    st.process_id,
                    st.overlay_visible,
                    st.hud_visible,
                    st.fps,
                    st.capture_fps,
                    st.frames,
                    st.pass_name,
                    vram,
                    if vram.is_empty() { "" } else { " " },
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
