use std::sync::Arc;
use std::thread;
use std::time::Duration;

use sidecar_runtime::{
    control_socket_path_in, execute_panic_with, read_at, send_at, ControlServer, SidecarCommand,
};
use tempfile::tempdir;

#[test]
fn panic_hides_overlay_and_stops_pipeline() {
    let dir = tempdir().unwrap();
    let sock = control_socket_path_in(dir.path());

    let server = Arc::new(ControlServer::create_at(sock.clone(), Box::new(|_| {})).unwrap());
    let stop = server.stop_flag();
    let pump_server = server.clone();
    thread::spawn(move || {
        while !*stop.lock().unwrap() {
            let _ = pump_server.pump_once();
            thread::sleep(Duration::from_millis(2));
        }
    });

    assert!(send_at(&sock, SidecarCommand::ShowHud).unwrap());
    assert!(execute_panic_with(|cmd| send_at(&sock, cmd).unwrap()));
    assert!(server.pipeline_stop_requested());

    for _ in 0..30 {
        if let Ok(Some(st)) = read_at(&sock) {
            if st.overlay_visible == 0 && st.hud_visible == 0 {
                return;
            }
        }
        thread::sleep(Duration::from_millis(5));
    }

    let st = read_at(&sock).unwrap().expect("status");
    assert_eq!(st.overlay_visible, 0);
    assert_eq!(st.hud_visible, 0);
}
