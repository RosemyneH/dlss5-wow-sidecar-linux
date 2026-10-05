mod common;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::Arc;
use std::time::Duration;

use common::{spawn_pump, wait_for_status};
use sidecar_runtime::{
    control_socket_path_in, read_at, send_at, ControlResponse, ControlServer, SidecarCommand,
};
use tempfile::tempdir;

const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn ipc_ping_status_and_stop() {
    let dir = tempdir().unwrap();
    let sock = control_socket_path_in(dir.path());

    let server = Arc::new(ControlServer::create_at(sock.clone(), Box::new(|_| {})).unwrap());
    let pump = spawn_pump(server.clone());

    assert!(send_at(&sock, SidecarCommand::ShowHud).unwrap());
    wait_for_status(&sock, TIMEOUT, |st| st.hud_visible == 1);

    assert!(send_at(&sock, SidecarCommand::Stop).unwrap());
    pump.join().unwrap();
    assert!(server.should_stop());
}

#[test]
fn misbehaving_clients_do_not_break_server() {
    let dir = tempdir().unwrap();
    let sock = control_socket_path_in(dir.path());
    let server = ControlServer::create_at(sock.clone(), Box::new(|_| {})).unwrap();

    drop(UnixStream::connect(&sock).unwrap());
    assert!(server.pump_once().unwrap());

    let mut garbage = UnixStream::connect(&sock).unwrap();
    writeln!(garbage, "not json").unwrap();
    assert!(server.pump_once().unwrap());
    let mut line = String::new();
    BufReader::new(&garbage).read_line(&mut line).unwrap();
    let resp: ControlResponse = serde_json::from_str(line.trim()).unwrap();
    assert!(matches!(resp, ControlResponse::Error { .. }), "{resp:?}");

    assert!(!server.pump_once().unwrap());

    let server = Arc::new(server);
    let pump = spawn_pump(server.clone());
    assert!(read_at(&sock).unwrap().is_some());
    assert!(send_at(&sock, SidecarCommand::Stop).unwrap());
    pump.join().unwrap();
}

#[test]
fn create_rejects_live_daemon_and_replaces_stale_socket() {
    let dir = tempdir().unwrap();
    let sock = control_socket_path_in(dir.path());

    drop(UnixListener::bind(&sock).unwrap());
    assert!(sock.exists());
    let server = Arc::new(ControlServer::create_at(sock.clone(), Box::new(|_| {})).unwrap());
    let pump = spawn_pump(server.clone());

    assert!(ControlServer::create_at(sock.clone(), Box::new(|_| {})).is_err());

    assert!(send_at(&sock, SidecarCommand::Stop).unwrap());
    pump.join().unwrap();
    drop(server);
    assert!(!sock.exists());
}
