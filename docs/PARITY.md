# Parity with [dlss5-wow-sidecar](https://github.com/xilla420/dlss5-wow-sidecar) (Windows v0.4)

**Target:** same operator experience where Linux APIs allow. **Neural:** DLSS 5 identical only if Linux inference path exists; until then shader MVP + flag as partial.

| ID | Feature | Windows | Linux | Owner crate |
|----|---------|---------|-------|-------------|
| P01 | Smart WoW folder scan | yes | yes | sidecar-core |
| P02 | WoW window discovery | HWND | Hypr/Sway | sidecar-core |
| P03 | `sidecar.toml` config | yes | yes | sidecar-config |
| P04 | System checks / probes | yes | yes | sidecar-probes |
| P05 | Injector filename scan | yes | yes | sidecar-probes |
| P06 | PipeWire/window capture | WGC | partial (portal + `WOWSIDECAR_CAPTURE_NODE` direct PW; Hypr hint env) | sidecar-capture |
| P07 | Overlay present (AOT + track WoW rect) | D3D11 | partial (winit; no Wayland layer-shell) | sidecar-overlay |
| P08 | Overlay daemon + IPC | wowsidecar.exe | partial | sidecar-runtime |
| P09 | Hotkeys (no inject) | yes | partial | sidecar-runtime |
| P10 | Status: FPS, GPU, capture | yes | partial | sidecar-runtime |
| P11 | Neural pass | NGX+ReShade | partial (sharpen chain MVP: yes) | sidecar-neural |
| P12 | Presets / tuning | yes | yes | sidecar-config |
| P13 | Manager UI (5 sections) | ImGui | yes (egui; live IPC + setup rows) | wowsidecar-manager |
| P14 | Setup / component install | yes | yes | sidecar-install |
| P15 | Themes / i18n | yes | yes | wowsidecar-manager |
| P16 | Panic hotkey | yes | yes | sidecar-runtime |
| P17 | Safety: no game hooks | enforced | yes | sidecar-probes |
| P18 | CI + unit tests | yes | yes | repo root |

Update checkboxes as agents land work. Run: `./scripts/parity-status.sh`
