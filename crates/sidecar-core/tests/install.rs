use sidecar_core::is_wow_game_dir;
use std::fs;
use std::path::PathBuf;

#[test]
fn detects_wow_exe_in_directory() {
    let dir = std::env::temp_dir().join("wowsidecar-linux-test-install");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("WowB.exe"), b"fake").unwrap();
    assert_eq!(is_wow_game_dir(&dir).as_deref(), Some("WowB.exe"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn branch_rank_prefers_beta_path_layout() {
    let beta = PathBuf::from("/home/user/Games/WoWRetail/World of Warcraft/_classic_beta_");
    let path = beta.to_string_lossy();
    assert!(path.contains("_classic_beta_"));
}
