use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    pub installed_as: String,
    pub title: String,
    pub purpose: String,
    pub accepts: String,
    pub source: String,
    pub required: bool,
}

pub struct InstallResult {
    pub ok: bool,
    pub message: String,
}

pub type ComponentForFileResult = Option<usize>;

pub fn components() -> &'static [Component] {
    static COMPONENTS: OnceLock<Vec<Component>> = OnceLock::new();
    COMPONENTS
        .get_or_init(|| {
            vec![
                Component {
                    installed_as: "neural-mvp.vert.spv".into(),
                    title: "Neural MVP vertex shader".into(),
                    purpose:
                        "Vulkan SPIR-V for the fullscreen pass that feeds the sharpen/upscale \
                              chain. Without it the overlay has nothing to draw."
                            .into(),
                    accepts: "neural-mvp.vert.spv".into(),
                    source:
                        "Release bundle or build from sidecar-neural shaders (not ngx runtimes)"
                            .into(),
                    required: true,
                },
                Component {
                    installed_as: "neural-mvp.frag.spv".into(),
                    title: "Neural MVP fragment shader".into(),
                    purpose:
                        "Shader-based neural stand-in until a Linux ML backend ships. This is \
                              the minimum effect path — not DLSS 5 NGX."
                            .into(),
                    accepts: "neural-mvp.frag.spv".into(),
                    source: "Release bundle or build from sidecar-neural shaders".into(),
                    required: true,
                },
                Component {
                    installed_as: "neural.onnx".into(),
                    title: "ONNX inference model".into(),
                    purpose: "Optional. Only used if the ONNX capture path is enabled; shader MVP \
                              does not load it."
                        .into(),
                    accepts: "neural.onnx,model.onnx".into(),
                    source: "Operator-provided weights — nothing redistributable in-repo".into(),
                    required: false,
                },
            ]
        })
        .as_slice()
}

pub fn generated_files() -> &'static [&'static str] {
    &["sidecar.toml", "sidecar.log", "sidecar-manager.log"]
}

fn lower(s: &str) -> String {
    s.to_ascii_lowercase()
}

pub fn file_matches_component(component: &Component, file_name: &str) -> bool {
    let needle = lower(file_name);
    let accepts = lower(&component.accepts);
    let mut start = 0;
    while start <= accepts.len() {
        let comma = accepts[start..].find(',').map(|i| start + i);
        let end = comma.unwrap_or(accepts.len());
        if accepts[start..end] == needle {
            return true;
        }
        match comma {
            Some(c) => start = c + 1,
            None => break,
        }
    }
    false
}

pub fn component_for_file(file_name: &str) -> ComponentForFileResult {
    components()
        .iter()
        .enumerate()
        .find(|(_, c)| file_matches_component(c, file_name))
        .map(|(i, _)| i)
}

pub fn install_component(
    component: &Component,
    source: &Path,
    sidecar_dir: &Path,
) -> InstallResult {
    if !source.is_file() {
        return InstallResult {
            ok: false,
            message: if source.exists() {
                "That is not a file.".into()
            } else {
                "That file no longer exists.".into()
            },
        };
    }

    let destination = sidecar_dir.join(&component.installed_as);
    if source == destination {
        return InstallResult {
            ok: true,
            message: "Already installed.".into(),
        };
    }

    if let Err(e) = std::fs::copy(source, &destination) {
        return InstallResult {
            ok: false,
            message: format!("Could not copy it in: {e}"),
        };
    }

    InstallResult {
        ok: true,
        message: format!("Installed {}.", component.installed_as),
    }
}

pub fn uninstall_plan(sidecar_dir: &Path, include_generated_files: bool) -> Vec<PathBuf> {
    let mut plan = Vec::new();
    for c in components() {
        let path = sidecar_dir.join(&c.installed_as);
        if path.is_file() {
            plan.push(path);
        }
    }
    if include_generated_files {
        for name in generated_files() {
            let path = sidecar_dir.join(name);
            if path.is_file() {
                plan.push(path);
            }
        }
    }
    plan
}

pub fn remove_all(plan: &[PathBuf]) -> InstallResult {
    let mut removed = 0usize;
    for path in plan {
        if let Err(e) = std::fs::remove_file(path) {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            return InstallResult {
                ok: false,
                message: format!(
                    "Could not remove {name}. Stop the overlay and close anything using it, \
                     then try again: {e}"
                ),
            };
        }
        removed += 1;
    }
    InstallResult {
        ok: true,
        message: format!("Removed {removed} file(s)."),
    }
}

pub trait FileMatchesComponent {
    fn file_matches(&self, file_name: &str) -> bool;
}

impl FileMatchesComponent for Component {
    fn file_matches(&self, file_name: &str) -> bool {
        file_matches_component(self, file_name)
    }
}
