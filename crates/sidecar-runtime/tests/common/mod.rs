#![allow(dead_code)]

use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use sidecar_runtime::{read_at, run_daemon_loop, ControlServer, SidecarStatus};
use tempfile::TempDir;

pub fn isolate_home() {
    static HOME: OnceLock<TempDir> = OnceLock::new();
    HOME.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", dir.path());
        dir
    });
}

pub fn wait_for_status(
    sock: &Path,
    timeout: Duration,
    pred: impl Fn(&SidecarStatus) -> bool,
) -> SidecarStatus {
    let deadline = Instant::now() + timeout;
    let mut last = None;
    while Instant::now() < deadline {
        if let Ok(Some(st)) = read_at(sock) {
            if pred(&st) {
                return st;
            }
            last = Some(st);
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("status condition not met within {timeout:?}; last status: {last:?}");
}

pub fn spawn_daemon(server: Arc<ControlServer>) -> JoinHandle<anyhow::Result<()>> {
    thread::spawn(move || run_daemon_loop(&server))
}

pub fn spawn_pump(server: Arc<ControlServer>) -> JoinHandle<()> {
    let stop = server.stop_flag();
    thread::spawn(move || {
        while !*stop.lock().unwrap() {
            server.pump_once().unwrap();
            thread::sleep(Duration::from_millis(2));
        }
    })
}

pub fn join_within(handle: JoinHandle<anyhow::Result<()>>, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while !handle.is_finished() {
        assert!(
            Instant::now() < deadline,
            "daemon did not exit within {timeout:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
    handle.join().unwrap().unwrap();
}
