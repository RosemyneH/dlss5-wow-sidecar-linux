use std::fs;
use std::path::Path;

use sidecar_probes::{
    architecture_from_name, find_injector_loaders, path_looks_like_wow_install,
    probe_injector_scan, ProbeState,
};
use tempfile::tempdir;

#[test]
fn i7_case_insensitive() {
    assert!(path_looks_like_wow_install(Path::new(
        "/Games/WORLD OF WARCRAFT/_RETAIL_"
    )));
    assert!(path_looks_like_wow_install(Path::new("/games/wow/wow.exe")));
}

#[test]
fn i8_all_loader_filenames() {
    for name in [
        "dxgi.dll",
        "d3d12.dll",
        "d3d11.dll",
        "dinput8.dll",
        "winmm.dll",
        "version.dll",
        "opengl32.dll",
    ] {
        assert_eq!(find_injector_loaders(&[name.into()]).len(), 1, "{name}");
    }
}

#[test]
fn i8_clean_directory() {
    let found =
        find_injector_loaders(&["Wow.exe".into(), "Data".into(), "Logs".into(), "WTF".into()]);
    assert!(found.is_empty());
}

#[test]
fn i8_case_insensitive_loaders() {
    assert_eq!(find_injector_loaders(&["DXGI.DLL".into()]).len(), 1);
    assert_eq!(find_injector_loaders(&["reshade.ini".into()]).len(), 1);
}

#[test]
fn injector_scan_on_disk() {
    let dir = tempdir().expect("tempdir");
    fs::write(dir.path().join("Wow.exe"), b"").unwrap();
    fs::write(dir.path().join("dxgi.dll"), b"").unwrap();
    let r = probe_injector_scan(dir.path());
    assert_eq!(r.state, ProbeState::Fail);
    assert!(r.detail.contains("dxgi.dll"));
}

#[test]
fn gpu_memory_csv_parses() {
    if let Some(mem) = sidecar_probes::query_gpu_memory() {
        assert!(mem.total_mb >= mem.used_mb);
    }
}

#[test]
fn gpu_arch_from_marketing_name() {
    assert_eq!(
        architecture_from_name("NVIDIA GeForce RTX 4090"),
        sidecar_probes::GpuArch::Ada
    );
    assert_eq!(
        architecture_from_name("NVIDIA GeForce RTX 5090"),
        sidecar_probes::GpuArch::Blackwell
    );
}
