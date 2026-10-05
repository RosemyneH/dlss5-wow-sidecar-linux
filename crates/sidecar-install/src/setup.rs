use std::path::Path;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::component::{components, Component};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeDep {
    pub id: String,
    pub title: String,
    pub purpose: String,
    pub packages_hint: String,
    pub probe_binary: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentRow {
    pub component: Component,
    pub present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeDepStatus {
    pub dep: RuntimeDep,
    pub present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupPageData {
    pub components: Vec<ComponentRow>,
    pub runtime_deps: Vec<RuntimeDepStatus>,
    pub missing_required_components: usize,
    pub missing_required_runtime: usize,
}

pub fn runtime_deps() -> &'static [RuntimeDep] {
    static DEPS: OnceLock<Vec<RuntimeDep>> = OnceLock::new();
    DEPS.get_or_init(|| {
        vec![
            RuntimeDep {
                id: "vulkan".into(),
                title: "Vulkan loader & tools".into(),
                purpose: "Fullscreen overlay and shader neural MVP run on Vulkan. Without a \
                          working loader, nothing can present."
                    .into(),
                packages_hint: "vulkan-tools, vulkan-driver (e.g. nvidia-utils on Arch)".into(),
                probe_binary: "vulkaninfo".into(),
                required: true,
            },
            RuntimeDep {
                id: "pipewire".into(),
                title: "PipeWire session".into(),
                purpose: "Portal window capture (P06) pulls frames from PipeWire. Optional until \
                          capture lands; shader MVP can run without it."
                    .into(),
                packages_hint: "pipewire, wireplumber, xdg-desktop-portal".into(),
                probe_binary: "pw-dump".into(),
                required: false,
            },
        ]
    })
    .as_slice()
}

fn which(cmd: &str) -> Option<String> {
    let output = std::process::Command::new("which").arg(cmd).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

pub fn probe_runtime_dep(dep: &RuntimeDep) -> RuntimeDepStatus {
    let probe_path = which(&dep.probe_binary);
    RuntimeDepStatus {
        dep: dep.clone(),
        present: probe_path.is_some(),
        probe_path,
    }
}

pub fn setup_page_data(sidecar_dir: &Path) -> SetupPageData {
    let components: Vec<ComponentRow> = components()
        .iter()
        .map(|c| {
            let present = sidecar_dir.join(&c.installed_as).is_file();
            ComponentRow {
                component: c.clone(),
                present,
            }
        })
        .collect();

    let runtime_deps: Vec<RuntimeDepStatus> =
        runtime_deps().iter().map(probe_runtime_dep).collect();

    let missing_required_components = components
        .iter()
        .filter(|r| r.component.required && !r.present)
        .count();
    let missing_required_runtime = runtime_deps
        .iter()
        .filter(|r| r.dep.required && !r.present)
        .count();

    SetupPageData {
        components,
        runtime_deps,
        missing_required_components,
        missing_required_runtime,
    }
}
