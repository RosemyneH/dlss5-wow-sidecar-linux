mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{isolate_home, join_within, spawn_daemon, wait_for_status};
use sidecar_runtime::{
    control_socket_path_in, read_at, send_at, start_overlay_at, ControlServer, SidecarCommand,
};
use tempfile::tempdir;

const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn pipeline_start_mock_frames_and_overlay_visibility() {
    isolate_home();
    let dir = tempdir().unwrap();
    let sock = control_socket_path_in(dir.path());

    let server = Arc::new(ControlServer::create_mock_at(sock.clone(), Box::new(|_| {})).unwrap());
    let daemon = spawn_daemon(server.clone());

    assert!(start_overlay_at(&sock).unwrap());
    let st = wait_for_status(&sock, TIMEOUT, |st| st.frames >= 3);
    assert_eq!((st.width, st.height), (64, 64));
    assert!(
        st.last_error.is_empty(),
        "pipeline error: {}",
        st.last_error
    );

    assert!(send_at(&sock, SidecarCommand::HideOverlay).unwrap());
    let hidden = wait_for_status(&sock, TIMEOUT, |st| st.overlay_visible == 0);
    wait_for_status(&sock, TIMEOUT, |st| {
        st.frames > hidden.frames && st.overlay_visible == 0
    });
    assert!(server.pipeline().lock().unwrap().running());

    assert!(send_at(&sock, SidecarCommand::Stop).unwrap());
    join_within(daemon, TIMEOUT);
    assert!(!server.pipeline().lock().unwrap().running());
}

#[test]
fn start_is_idempotent_while_running() {
    isolate_home();
    let dir = tempdir().unwrap();
    let sock = control_socket_path_in(dir.path());

    let server = Arc::new(ControlServer::create_mock_at(sock.clone(), Box::new(|_| {})).unwrap());
    let daemon = spawn_daemon(server.clone());

    let before = wait_for_status(&sock, TIMEOUT, |st| st.frames >= 2);
    assert!(start_overlay_at(&sock).unwrap());
    assert!(start_overlay_at(&sock).unwrap());
    let after = read_at(&sock).unwrap().expect("status");
    assert!(
        after.frames >= before.frames,
        "start restarted the pipeline"
    );
    wait_for_status(&sock, TIMEOUT, |st| st.frames > after.frames);

    assert!(send_at(&sock, SidecarCommand::Stop).unwrap());
    join_within(daemon, TIMEOUT);
}
