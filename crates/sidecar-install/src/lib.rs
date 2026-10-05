//! Linux setup parity with Windows `Install.cpp`: file components beside the
//! sidecar binary, optional runtime probes (Vulkan, PipeWire), and release
//! `BUILD-MANIFEST.json`. No `nvngx_*.dll` — neural MVP is SPIR-V (+ optional ONNX).

mod component;
mod manifest;
mod setup;

pub use component::{
    component_for_file, components, file_matches_component, generated_files, install_component,
    remove_all, uninstall_plan, Component, ComponentForFileResult, FileMatchesComponent,
    InstallResult,
};
pub use manifest::{build_manifest, write_build_manifest, BuildManifest, ManifestFileEntry};
pub use setup::{
    probe_runtime_dep, runtime_deps, setup_page_data, RuntimeDep, RuntimeDepStatus, SetupPageData,
};
