use std::thread;
use std::time::Duration;

use sidecar_runtime::{
    control_socket_path_in, execute_panic_with, read_at, send_at, ControlServer, SidecarCommand,
};
use tempfile::tempdir;

#[test]
fn panic_hides_overlay() {
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
    execute_panic_with(|cmd| send_at(&sock, cmd).unwrap());

    let mut overlay = 1;
    let mut hud = 1;
    for _ in 0..50 {
        if let Ok(Some(st)) = read_at(&sock) {
            overlay = st.overlay_visible;
            hud = st.hud_visible;
            if overlay == 0 && hud == 0 {
                break;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(overlay, 0);
    assert_eq!(hud, 0);
}
