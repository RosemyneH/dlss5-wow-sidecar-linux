use std::thread;
use std::time::Duration;

use sidecar_capture::enable_mock_capture;
use sidecar_runtime::{
    control_socket_path_in, read_at, run_daemon_loop, send_at, start_overlay_at, ControlServer,
    SidecarCommand,
};
use tempfile::tempdir;

#[test]
fn pipeline_start_mock_frames_and_overlay_visibility() {
    enable_mock_capture();
    let dir = tempdir().unwrap();
    let sock = control_socket_path_in(dir.path());

    let server = ControlServer::create_at(sock.clone(), Box::new(|_| {})).unwrap();
    let daemon = thread::spawn(move || {
        let _ = run_daemon_loop(&server);
    });

    assert!(start_overlay_at(&sock).unwrap());

    let mut frames = 0u64;
    for _ in 0..100 {
        if let Ok(Some(st)) = read_at(&sock) {
            frames = st.frames;
            if frames >= 3 {
                break;
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(
        frames >= 3,
        "pipeline should advance frames with mock capture"
    );

    assert!(send_at(&sock, SidecarCommand::HideOverlay).unwrap());
    let mut hidden = 0u32;
    for _ in 0..50 {
        if let Ok(Some(st)) = read_at(&sock) {
            hidden = st.overlay_visible;
            if hidden == 0 {
                break;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(hidden, 0);

    assert!(send_at(&sock, SidecarCommand::Stop).unwrap());
    thread::sleep(Duration::from_millis(50));
    let _ = daemon.join();
}
