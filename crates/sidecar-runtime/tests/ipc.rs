use std::thread;
use std::time::Duration;

use sidecar_runtime::{control_socket_path_in, read_at, send_at, ControlServer, SidecarCommand};
use tempfile::tempdir;

#[test]
fn ipc_ping_status_and_stop() {
    let dir = tempdir().unwrap();
    let sock = control_socket_path_in(dir.path());

    let server = ControlServer::create_at(sock.clone(), Box::new(|_| {})).unwrap();
    let stop = server.stop_flag();
    thread::spawn(move || {
        while !*stop.lock().unwrap() {
            let _ = server.pump_once();
            thread::sleep(Duration::from_millis(5));
        }
    });

    assert!(send_at(&sock, SidecarCommand::ShowHud).unwrap());
    let mut hud = 0;
    for _ in 0..50 {
        if let Ok(Some(st)) = read_at(&sock) {
            hud = st.hud_visible;
            if hud == 1 {
                break;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(hud, 1);

    assert!(send_at(&sock, SidecarCommand::Stop).unwrap());
    thread::sleep(Duration::from_millis(20));
}
