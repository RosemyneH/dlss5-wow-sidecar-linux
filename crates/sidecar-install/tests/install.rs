use sidecar_install::{
    build_manifest, component_for_file, components, file_matches_component, install_component,
    remove_all, runtime_deps, setup_page_data, uninstall_plan, write_build_manifest,
};

fn by_name(installed_as: &str) -> &sidecar_install::Component {
    components()
        .iter()
        .find(|c| c.installed_as == installed_as)
        .unwrap_or_else(|| panic!("no such component: {installed_as}"))
}

#[test]
fn every_component_names_file_source_and_purpose() {
    for c in components() {
        assert!(!c.installed_as.is_empty());
        assert!(!c.title.is_empty());
        assert!(!c.purpose.is_empty());
        assert!(!c.source.is_empty());
        assert!(!c.accepts.is_empty());
    }
}

#[test]
fn no_nvngx_dll_components() {
    for c in components() {
        let lower = c.installed_as.to_ascii_lowercase();
        assert!(
            !lower.contains("nvngx"),
            "Linux install must not list ngx dlls: {}",
            c.installed_as
        );
    }
}

#[test]
fn onnx_accept_aliases() {
    let onnx = by_name("neural.onnx");
    assert!(file_matches_component(onnx, "neural.onnx"));
    assert!(file_matches_component(onnx, "model.onnx"));
    assert!(!file_matches_component(onnx, "neural-mvp.frag.spv"));
}

#[test]
fn matching_ignores_case() {
    let frag = by_name("neural-mvp.frag.spv");
    assert!(file_matches_component(frag, "NEURAL-MVP.FRAG.SPV"));
}

#[test]
fn unknown_file_has_no_component() {
    assert_eq!(component_for_file("readme.txt"), None);
    assert_eq!(component_for_file(""), None);
    assert_eq!(component_for_file("neural.onnx"), Some(2));
}

#[test]
fn uninstall_plan_empty_for_unrelated_dir() {
    let dir = std::env::temp_dir().join("dlss5-sidecar-not-installed-here");
    let plan = uninstall_plan(&dir, true);
    assert!(plan.is_empty());
}

#[test]
fn install_and_manifest_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let sidecar = dir.path().join("sidecar");
    std::fs::create_dir_all(&sidecar).unwrap();

    let frag = by_name("neural-mvp.frag.spv");
    let src = dir.path().join("shader.spv");
    std::fs::write(&src, b"fake-spv-bytes").unwrap();

    let result = install_component(frag, &src, &sidecar);
    assert!(result.ok);

    let manifest = build_manifest(&sidecar, "0.1.0", "deadbeef").unwrap();
    assert_eq!(manifest.platform, "linux");
    assert_eq!(manifest.files.len(), 1);
    assert!(manifest.neural_mvp_shader_sha256.is_some());

    let manifest_path = sidecar.join("BUILD-MANIFEST.json");
    write_build_manifest(&manifest_path, &manifest).unwrap();
    let parsed: sidecar_install::BuildManifest =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    assert_eq!(parsed, manifest);

    let plan = uninstall_plan(&sidecar, false);
    assert_eq!(plan.len(), 1);
    let removed = remove_all(&plan);
    assert!(removed.ok);
}

#[test]
fn setup_page_data_counts_missing_required() {
    let dir = tempfile::tempdir().unwrap();
    let data = setup_page_data(dir.path());
    assert_eq!(data.missing_required_components, 2);
    assert!(data.components.len() >= 3);
    assert_eq!(data.runtime_deps.len(), runtime_deps().len());
}
