# Parity with [dlss5-wow-sidecar](https://github.com/xilla420/dlss5-wow-sidecar) (Windows v0.4)

**Target:** same operator experience where Linux APIs allow. **Neural:** DLSS 5 identical only if Linux inference path exists; until then shader MVP + flag as partial.

| ID | Feature | Windows | Linux | Owner crate |
|----|---------|---------|-------|-------------|
| P01 | Smart WoW folder scan | yes | yes | sidecar-core |
| P02 | WoW window discovery | HWND | Hypr/Sway | sidecar-core |
| P03 | `sidecar.toml` config | yes | todo | sidecar-config |
| P04 | System checks / probes | yes | todo | sidecar-probes |
| P05 | Injector filename scan | yes | todo | sidecar-probes |
| P06 | PipeWire/window capture | WGC | todo | sidecar-capture |
| P07 | Overlay present | D3D11 | todo | sidecar-overlay |
| P08 | Overlay daemon + IPC | wowsidecar.exe | todo | sidecar-runtime |
| P09 | Hotkeys (no inject) | yes | todo | sidecar-runtime |
| P10 | Status: FPS, GPU, capture | yes | todo | sidecar-runtime |
| P11 | Neural pass | NGX+ReShade | todo | sidecar-neural |
| P12 | Presets / tuning | yes | todo | sidecar-config |
| P13 | Manager UI (5 sections) | ImGui | todo | wowsidecar-manager |
| P14 | Setup / component install | yes | partial | sidecar-install |
| P15 | Themes / i18n | yes | todo | wowsidecar-manager |
| P16 | Panic hotkey | yes | todo | sidecar-runtime |
| P17 | Safety: no game hooks | enforced | todo | sidecar-probes |
| P18 | CI + unit tests | yes | todo | repo root |

Update checkboxes as agents land work. Run: `./scripts/parity-status.sh`
