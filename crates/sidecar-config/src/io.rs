use std::fs;
use std::path::{Path, PathBuf};

use crate::{parse_config, serialize_config, Config};

pub fn sidecar_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".config/wowsidecar-linux");
    }
    PathBuf::from(".")
}

pub fn default_config_path() -> PathBuf {
    sidecar_dir().join("sidecar.toml")
}

pub fn load_config(path: &Path) -> (Config, Vec<String>) {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return (Config::default(), Vec::new()),
    };
    parse_config(&text)
}

pub fn save_config(path: &Path, config: &Config) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
    }
    let text = serialize_config(config);
    fs::write(path, text).map_err(|e| format!("could not write {}: {e}", path.display()))
}
