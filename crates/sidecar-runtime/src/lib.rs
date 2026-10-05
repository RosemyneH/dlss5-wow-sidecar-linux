mod control;
mod fps;
mod hotkeys;
mod protocol;
mod socket_path;

pub use control::{
    ensure_runtime_dir, is_running, read, read_at, run_daemon_loop, send, send_at, send_named,
    start_daemon, status, stop, ControlServer,
};
pub use fps::FpsCounter;
pub use hotkeys::{spawn_hotkey_thread, HotkeyBindings};
pub use protocol::{ControlRequest, ControlResponse, SidecarCommand, SidecarStatus};
pub use socket_path::{
    control_socket_path, control_socket_path_in, runtime_dir, CONTROL_SOCKET_NAME, RUNTIME_DIR_NAME,
};

pub use control::client::socket_path;

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

    spawn_hotkey_thread(server.stop_flag(), HotkeyBindings::default());
    run_daemon_loop(&server)
}
