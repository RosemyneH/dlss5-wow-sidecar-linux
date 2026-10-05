mod install;
mod window;

pub use install::{is_wow_game_dir, smart_scan_installs, SmartScanOptions, WowInstall};
pub use window::{list_wow_windows, DesktopWindow};
