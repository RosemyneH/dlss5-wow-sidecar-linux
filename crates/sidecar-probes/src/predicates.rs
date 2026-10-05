use std::path::Path;

const INJECTOR_LOADERS: &[&str] = &[
    "dxgi.dll",
    "d3d12.dll",
    "d3d11.dll",
    "dinput8.dll",
    "winmm.dll",
    "version.dll",
    "opengl32.dll",
];

const INJECTOR_CONFIGS: &[&str] = &["reshade.ini", "reshade.log"];

const WOW_SIGNATURES: &[&str] = &[
    "wow.exe",
    "wowb.exe",
    "_retail_",
    "_classic_beta_",
    "_classic_",
    "_classic_era_",
    "world of warcraft",
];

pub fn path_looks_like_wow_install(path: &Path) -> bool {
    let lowered = path.to_string_lossy().to_ascii_lowercase();
    WOW_SIGNATURES.iter().any(|sig| lowered.contains(sig))
}

pub fn find_injector_loaders(filenames: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    for name in filenames {
        let lowered = name.to_ascii_lowercase();
        let hit = INJECTOR_LOADERS
            .iter()
            .chain(INJECTOR_CONFIGS.iter())
            .any(|needle| lowered == *needle);
        if hit {
            found.push(name.clone());
        }
    }
    found
}

fn lexical_generic(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

pub(crate) fn is_inside(child: &Path, parent: &Path) -> bool {
    if parent.as_os_str().is_empty() {
        return false;
    }
    let mut p = lexical_generic(parent);
    let c = lexical_generic(child);
    if !p.is_empty() && p.ends_with('/') {
        p.pop();
    }
    if c.len() < p.len() {
        return false;
    }
    if !c.starts_with(&p) {
        return false;
    }
    c.len() == p.len() || c[p.len()..].starts_with('/')
}
