mod install;
mod window;

pub use install::{SmartScanOptions, WowInstall, is_wow_game_dir, smart_scan_installs};
pub use window::{DesktopWindow, list_wow_windows};
