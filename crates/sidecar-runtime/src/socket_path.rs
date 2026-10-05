use std::path::{Path, PathBuf};

pub const RUNTIME_DIR_NAME: &str = "wowsidecar";
pub const CONTROL_SOCKET_NAME: &str = "control.sock";

pub fn runtime_dir() -> PathBuf {
    let base = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(base).join(RUNTIME_DIR_NAME)
}

pub fn control_socket_path() -> PathBuf {
    runtime_dir().join(CONTROL_SOCKET_NAME)
}

pub fn control_socket_path_in(dir: &Path) -> PathBuf {
    dir.join(CONTROL_SOCKET_NAME)
}
