use std::path::{Path, PathBuf};

const BRANCH_DIRS: &[&str] = &["_retail_", "_classic_beta_", "_classic_", "_classic_era_"];

const WOW_EXE_NAMES: &[&str] = &["Wow.exe", "WowB.exe", "WowClassic.exe", "WowClassicT.exe"];

const LAYOUT_SUFFIXES: &[&str] = &[
    "World of Warcraft/_retail_",
    "World of Warcraft/_classic_beta_",
    "World of Warcraft/_classic_",
    "World of Warcraft/_classic_era_",
    "Games/WoWRetail/World of Warcraft/_retail_",
    "Games/WoWRetail/World of Warcraft/_classic_beta_",
    "Games/WoWRetail/World of Warcraft/_classic_",
    "Games/WoW335",
];

#[derive(Debug, Clone, Default)]
pub struct SmartScanOptions {
    pub extra_roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WowInstall {
    pub game_dir: PathBuf,
    pub client_exe: String,
    pub branch_rank: u8,
}

pub fn is_wow_game_dir(dir: &Path) -> Option<String> {
    if !dir.is_dir() {
        return None;
    }
    for name in WOW_EXE_NAMES {
        if dir.join(name).is_file() {
            return Some(name.to_string());
        }
    }
    None
}

fn branch_rank(path: &Path) -> u8 {
    let lowered = path.to_string_lossy().to_ascii_lowercase();
    if lowered.contains("_retail_") {
        return 0;
    }
    if lowered.contains("_classic_beta_") {
        return 1;
    }
    if lowered.contains("_classic_era_") {
        return 3;
    }
    if lowered.contains("_classic_") {
        return 2;
    }
    4
}

fn scan_roots(opts: &SmartScanOptions) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        let home_path = PathBuf::from(&home);
        roots.push(home_path.clone());
        roots.push(home_path.join("Games"));
    }
    roots.push(PathBuf::from("/mnt"));
    roots.push(PathBuf::from("/media"));
    roots.extend(opts.extra_roots.clone());
    roots
}

fn try_layouts_under(root: &Path, found: &mut Vec<WowInstall>) {
    for suffix in LAYOUT_SUFFIXES {
        let candidate = root.join(suffix);
        if let Some(exe) = is_wow_game_dir(&candidate) {
            push_unique(found, candidate, exe);
        }
    }
}

fn walk_shallow(dir: &Path, depth: usize, found: &mut Vec<WowInstall>) {
    if depth == 0 {
        return;
    }
    if let Some(exe) = is_wow_game_dir(dir) {
        push_unique(found, dir.to_path_buf(), exe);
    }
    try_layouts_under(dir, found);

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        let interesting = name == "games"
            || name == "world of warcraft"
            || name.contains("wow")
            || name == "steamapps";
        if !interesting {
            continue;
        }
        walk_shallow(&path, depth - 1, found);
    }
}

fn push_unique(found: &mut Vec<WowInstall>, game_dir: PathBuf, client_exe: String) {
    if found.iter().any(|i| i.game_dir == game_dir) {
        return;
    }
    let rank = branch_rank(&game_dir);
    found.push(WowInstall {
        game_dir,
        client_exe,
        branch_rank: rank,
    });
}

fn from_battle_net_layout(parent: &Path, found: &mut Vec<WowInstall>) {
    for branch in BRANCH_DIRS {
        let candidate = parent.join(branch);
        if let Some(exe) = is_wow_game_dir(&candidate) {
            push_unique(found, candidate, exe);
        }
    }
}

pub fn smart_scan_installs(opts: &SmartScanOptions) -> Vec<WowInstall> {
    let mut found = Vec::new();

    for root in scan_roots(opts) {
        if !root.exists() {
            continue;
        }
        try_layouts_under(&root, &mut found);
        walk_shallow(&root, 3, &mut found);
        if root.file_name().is_some_and(|n| n == "Games") {
            continue;
        }
        let games = root.join("Games");
        if games.is_dir() {
            walk_shallow(&games, 4, &mut found);
        }
        let wow_parent = root.join("World of Warcraft");
        if wow_parent.is_dir() {
            from_battle_net_layout(&wow_parent, &mut found);
        }
    }

    found.sort_by(|a, b| {
        a.branch_rank
            .cmp(&b.branch_rank)
            .then_with(|| a.game_dir.cmp(&b.game_dir))
    });
    found
}
