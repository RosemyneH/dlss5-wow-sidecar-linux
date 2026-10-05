mod control;
mod fps;
mod hotkeys;
mod pipeline;
mod protocol;
mod socket_path;

pub use control::{
    ensure_runtime_dir, is_running, read, read_at, run_daemon_loop, send, send_at, start_daemon,
    status, stop, ControlServer,
};
pub use fps::FpsCounter;
pub use hotkeys::{spawn_hotkey_thread, HotkeyBindings};
pub use pipeline::Pipeline;
pub use protocol::{ControlRequest, ControlResponse, SidecarCommand, SidecarStatus};
pub use socket_path::{
    control_socket_path, control_socket_path_in, runtime_dir, CONTROL_SOCKET_NAME, RUNTIME_DIR_NAME,
};

pub use control::client::socket_path;

fn opt_hotkey(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

pub fn run_daemon() -> anyhow::Result<()> {
    ensure_runtime_dir()?;
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("sidecar_runtime=info".parse()?),
        )
        .init();

    let server = ControlServer::create(Box::new(|cmd| {
        tracing::info!(?cmd, "daemon command");
    }))?;

    let (config, warnings) = sidecar_config::load_config(&sidecar_config::default_config_path());
    for w in warnings {
        tracing::warn!("config: {w}");
    }
    let mut bindings = HotkeyBindings {
        start_stop: opt_hotkey(&config.hotkeys.start_stop),
        toggle_overlay: opt_hotkey(&config.hotkeys.toggle_overlay),
        toggle_hud: opt_hotkey(&config.hotkeys.toggle_hud),
        panic_combo: Some("Ctrl+Alt+Backspace".into()),
    };
    #[cfg(all(feature = "hotkeys-evdev-panic", not(feature = "hotkeys-evdev-full")))]
    {
        bindings.start_stop = None;
        bindings.toggle_overlay = None;
        bindings.toggle_hud = None;
    }
    spawn_hotkey_thread(server.stop_flag(), bindings);
    run_daemon_loop(&server)
}
