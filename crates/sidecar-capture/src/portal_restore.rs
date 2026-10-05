use std::fs;
use std::path::PathBuf;

const TOKEN_FILE: &str = "screencast-restore.token";

fn token_path() -> PathBuf {
    let base = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".config/wowsidecar-linux");
    base.join(TOKEN_FILE)
}

pub fn load_screencast_restore_token() -> Option<String> {
    let path = token_path();
    let text = fs::read_to_string(&path).ok()?;
    let token = text.trim();
    if token.is_empty() {
        return None;
    }
    Some(token.to_string())
}

pub fn save_screencast_restore_token(token: &str) {
    let path = token_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, token);
}

pub fn clear_screencast_restore_token() {
    let _ = fs::remove_file(token_path());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    static TOKEN_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn lock_token_test() -> MutexGuard<'static, ()> {
        TOKEN_TEST_LOCK.lock().unwrap()
    }

    #[test]
    fn round_trip_token_under_temp_home() {
        let _g = lock_token_test();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", dir.path());
        clear_screencast_restore_token();
        assert!(load_screencast_restore_token().is_none());
        save_screencast_restore_token("abc-token");
        assert_eq!(
            load_screencast_restore_token().as_deref(),
            Some("abc-token")
        );
        clear_screencast_restore_token();
    }
}
