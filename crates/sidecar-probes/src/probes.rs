use std::path::Path;

use sidecar_core::list_wow_windows;

use crate::gpu::detect_primary_gpu;
use crate::predicates::{find_injector_loaders, is_inside, path_looks_like_wow_install};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeState {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeResult {
    pub state: ProbeState,
    pub title: String,
    pub detail: String,
    pub remedy: String,
}

impl ProbeResult {
    fn ok(title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            state: ProbeState::Ok,
            title: title.into(),
            detail: detail.into(),
            remedy: String::new(),
        }
    }

    fn warn(
        title: impl Into<String>,
        detail: impl Into<String>,
        remedy: impl Into<String>,
    ) -> Self {
        Self {
            state: ProbeState::Warn,
            title: title.into(),
            detail: detail.into(),
            remedy: remedy.into(),
        }
    }

    fn fail(
        title: impl Into<String>,
        detail: impl Into<String>,
        remedy: impl Into<String>,
    ) -> Self {
        Self {
            state: ProbeState::Fail,
            title: title.into(),
            detail: detail.into(),
            remedy: remedy.into(),
        }
    }
}

pub fn probe_gpu() -> ProbeResult {
    let title = "Graphics adapter";
    match detect_primary_gpu() {
        None => ProbeResult::fail(
            title,
            "No NVIDIA GPU detected (nvidia-smi unavailable or empty).",
            "This sidecar needs an NVIDIA RTX 40 or RTX 50 card with a working driver.",
        ),
        Some(gpu) => {
            let detail = format!("{} — {}", gpu.name, gpu.arch.as_str());
            if gpu.arch.sidecar_supported() {
                ProbeResult::ok(title, detail)
            } else {
                ProbeResult::fail(
                    title,
                    detail,
                    "RTX 40 (Ada) or RTX 50 (Blackwell) is required. Older cards are \
                     refused rather than run badly.",
                )
            }
        }
    }
}

pub fn probe_driver() -> ProbeResult {
    let title = "Display driver";
    match detect_primary_gpu() {
        None => ProbeResult::fail(
            title,
            "No NVIDIA GPU to query.",
            "Install an NVIDIA RTX 40 or RTX 50 card and the proprietary driver.",
        ),
        Some(gpu) => match gpu.driver_version {
            Some(ver) if !ver.is_empty() => ProbeResult::ok(title, format!("Driver {ver}")),
            _ => ProbeResult::warn(
                title,
                "Could not read the driver version from nvidia-smi.",
                "Not fatal. Update to a current NVIDIA driver if capture misbehaves.",
            ),
        },
    }
}

pub fn probe_session() -> ProbeResult {
    let title = "Display session";
    let session = std::env::var("XDG_SESSION_TYPE")
        .unwrap_or_else(|_| String::new())
        .to_ascii_lowercase();
    let detail = if session.is_empty() {
        "Session type unknown (XDG_SESSION_TYPE unset).".to_string()
    } else {
        format!("{session} (XDG_SESSION_TYPE)")
    };

    if session == "wayland" {
        return ProbeResult::ok(title, detail);
    }
    if session == "x11" {
        return ProbeResult::fail(
            title,
            detail,
            "Wayland is required: PipeWire window capture and the overlay path target \
             a Wayland compositor, not X11 alone.",
        );
    }
    ProbeResult::warn(
        title,
        detail,
        "Expected wayland. If you are on X11, switch to a Wayland session or expect capture \
         to fail.",
    )
}

pub fn probe_wow_window() -> ProbeResult {
    let title = "World of Warcraft window";
    let windows = list_wow_windows();
    if windows.is_empty() {
        return ProbeResult::warn(
            title,
            "WoW is not running (no matching Hyprland/Sway window).",
            "Start the game in windowed or borderless mode, then run the probes again.",
        );
    }
    let w = &windows[0];
    let detail = format!(
        "{}x{} via {} ({})",
        w.width,
        w.height,
        w.compositor,
        if w.fullscreen {
            "fullscreen"
        } else {
            "windowed"
        }
    );
    if w.fullscreen {
        ProbeResult::fail(
            title,
            detail,
            "Use borderless windowed or windowed mode. Fullscreen often blocks the compositor \
             surface PipeWire needs.",
        )
    } else {
        ProbeResult::ok(title, detail)
    }
}

pub fn probe_injector_scan(wow_dir: &Path) -> ProbeResult {
    let title = "Injector scan of the WoW folder";
    if wow_dir.as_os_str().is_empty() || !wow_dir.is_dir() {
        return ProbeResult::warn(
            title,
            "No WoW folder set, so nothing was scanned.",
            "Point the manager at your WoW folder so it can check for injectors before launching.",
        );
    }

    let mut filenames = Vec::new();
    if let Ok(entries) = std::fs::read_dir(wow_dir) {
        for entry in entries.flatten() {
            if entry.path().is_file() {
                filenames.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }

    let found = find_injector_loaders(&filenames);
    if found.is_empty() {
        return ProbeResult::ok(title, "No injector loaders found.");
    }

    ProbeResult::fail(
        title,
        format!("Found: {}", found.join(", ")),
        "Remove these from your WoW folder. Blizzard bans accounts for in-process injectors, \
         and this tool refuses to run alongside one.",
    )
}

pub fn probe_sidecar_path(sidecar_dir: &Path, wow_dir: &Path) -> ProbeResult {
    let title = "Sidecar install location";
    let detail = sidecar_dir.display().to_string();
    if path_looks_like_wow_install(sidecar_dir) || is_inside(sidecar_dir, wow_dir) {
        ProbeResult::fail(
            title,
            detail,
            "Move the sidecar outside your WoW folder. Anything sitting next to Wow.exe looks \
             like an injector, which is the one thing this design exists to avoid.",
        )
    } else {
        ProbeResult::ok(title, detail)
    }
}

pub fn run_all_probes(sidecar_dir: &Path, wow_dir: &Path) -> Vec<ProbeResult> {
    vec![
        probe_gpu(),
        probe_driver(),
        probe_session(),
        probe_wow_window(),
        probe_sidecar_path(sidecar_dir, wow_dir),
        probe_injector_scan(wow_dir),
    ]
}

#[cfg(test)]
mod unit {
    use super::*;
    use crate::predicates::{find_injector_loaders, path_looks_like_wow_install};

    #[test]
    fn i7_wow_paths_recognised() {
        assert!(path_looks_like_wow_install(Path::new(
            "/games/World of Warcraft/_retail_"
        )));
        assert!(path_looks_like_wow_install(Path::new("/wow/Wow.exe")));
    }

    #[test]
    fn i7_ordinary_paths_allowed() {
        assert!(!path_looks_like_wow_install(Path::new(
            "/tools/dlss5-sidecar"
        )));
    }

    #[test]
    fn i8_injector_detection() {
        let found =
            find_injector_loaders(&["Wow.exe".into(), "dxgi.dll".into(), "ReShade.ini".into()]);
        assert_eq!(found.len(), 2);
        assert!(found.contains(&"dxgi.dll".into()));
        assert!(found.contains(&"ReShade.ini".into()));
    }

    #[test]
    fn i9_sidecar_inside_wow_fails() {
        let r = probe_sidecar_path(
            Path::new("/games/wow/_retail_/sidecar"),
            Path::new("/games/wow"),
        );
        assert_eq!(r.state, ProbeState::Fail);
        assert!(!r.remedy.is_empty());
    }

    #[test]
    fn non_ok_probes_carry_remedy() {
        let results = run_all_probes(Path::new("/tools/dlss5-sidecar"), Path::new(""));
        assert!(!results.is_empty());
        for r in &results {
            assert!(!r.title.is_empty());
            if r.state != ProbeState::Ok {
                assert!(!r.remedy.is_empty(), "missing remedy for {}", r.title);
            }
        }
    }
}
