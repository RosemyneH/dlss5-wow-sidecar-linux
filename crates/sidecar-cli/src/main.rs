use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use sidecar_core::{SmartScanOptions, list_wow_windows, smart_scan_installs};
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
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("wowsidecar_linux=info".parse()?))
        .init();

    match Cli::parse().command {
        Commands::Scan { root } => {
            let opts = SmartScanOptions {
                extra_roots: root,
            };
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
            println!("session: {}", std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "?".into()));
            let installs = smart_scan_installs(&SmartScanOptions::default());
            println!("installs found: {}", installs.len());
            if let Some(best) = installs.first() {
                println!("best install: {} ({})", best.game_dir.display(), best.client_exe);
            }
            let windows = list_wow_windows();
            println!("wow windows: {}", windows.len());
            println!("pipewire: {}", which("pw-dump"));
            println!("hyprctl: {}", which("hyprctl"));
            println!("portal: {}", which("xdg-desktop-portal"));
            println!("\nneural pass: not implemented yet (see docs/ROADMAP.md)");
        }
    }

    Ok(())
}

fn which(cmd: &str) -> String {
    match std::process::Command::new("which").arg(cmd).output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "missing".into(),
    }
}
