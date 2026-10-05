mod install;
mod window;

pub use install::{is_wow_game_dir, smart_scan_installs, SmartScanOptions, WowInstall};
pub use window::{list_wow_windows, wow_window_match_score, DesktopWindow};
