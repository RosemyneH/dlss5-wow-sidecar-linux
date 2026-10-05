mod common;

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use common::{isolate_home, join_within, spawn_daemon, spawn_pump, wait_for_status};
use sidecar_runtime::{
    control_socket_path_in, execute_panic_with, read_at, send_at, start_overlay_at, ControlServer,
    SidecarCommand,
};
use tempfile::tempdir;

const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn panic_hides_overlay_and_stops_pipeline() {
    let dir = tempdir().unwrap();
    let sock = control_socket_path_in(dir.path());

    let server = Arc::new(ControlServer::create_at(sock.clone(), Box::new(|_| {})).unwrap());
    let pump = spawn_pump(server.clone());

    assert!(send_at(&sock, SidecarCommand::ShowHud).unwrap());
    assert!(execute_panic_with(|cmd| send_at(&sock, cmd).unwrap()));
    assert!(server.pipeline_stop_requested());

    let st = read_at(&sock).unwrap().expect("status");
    assert_eq!(st.overlay_visible, 0);
    assert_eq!(st.hud_visible, 0);

    assert!(send_at(&sock, SidecarCommand::Stop).unwrap());
    pump.join().unwrap();
}

#[test]
fn panic_halts_running_mock_pipeline_and_start_recovers() {
    isolate_home();
    let dir = tempdir().unwrap();
    let sock = control_socket_path_in(dir.path());

    let server = Arc::new(ControlServer::create_mock_at(sock.clone(), Box::new(|_| {})).unwrap());
    let daemon = spawn_daemon(server.clone());

    wait_for_status(&sock, TIMEOUT, |st| st.frames >= 3);

    assert!(execute_panic_with(|cmd| send_at(&sock, cmd).unwrap()));
    assert!(server.pipeline_stop_requested());
    assert!(!server.pipeline().lock().unwrap().running());

    let frozen = read_at(&sock).unwrap().expect("status");
    assert_eq!((frozen.overlay_visible, frozen.hud_visible), (0, 0));
    thread::sleep(Duration::from_millis(150));
    let later = read_at(&sock).unwrap().expect("status");
    assert_eq!(later.frames, frozen.frames, "frames advanced after panic");

    assert!(start_overlay_at(&sock).unwrap());
    assert!(!server.pipeline_stop_requested());
    wait_for_status(&sock, TIMEOUT, |st| {
        st.frames != frozen.frames && st.overlay_visible == 0
    });

    assert!(send_at(&sock, SidecarCommand::Stop).unwrap());
    join_within(daemon, TIMEOUT);
}
