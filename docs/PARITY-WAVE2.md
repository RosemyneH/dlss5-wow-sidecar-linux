# Parity wave 2 (open: P06–P16 partial)

Reference: `~/Repos/dlss5-wow-sidecar` v0.4. Goal: **end-to-end loop** in daemon — capture → neural → overlay — plus manager wiring.

| Agent | ID | Deliverable |
|-------|-----|-------------|
| 1 | P06 | Hyprland window capture hints; reduce portal friction; tests |
| 2 | P07 | winit always-on-top + borderless over game rect; poll `list_wow_windows` for moves |
| 3 | P08 | yes — `sidecar-runtime` pipeline thread (capture→neural→overlay; mock IPC test) |
| 4 | P09 | Global hotkeys: portal GlobalShortcuts or documented Hypr binds + IPC — **done** |
| 5 | P10 | Real `capture_fps` / `fps` in `SidecarStatus` from pipeline |
| 6 | P11 | Daemon applies `sidecar-config` neural backend per frame |
| 7 | P13 | Manager: Start/Stop overlay, live status, Setup install rows — **done** |
| 8 | P15 | Stormwind/Questlog/Dragonflight themes + en/ru i18n — **done** |
| 9 | P16 | Panic `Ctrl+Alt+Backspace` via evdev in default feature set |
| 10 | E2E | CLI `wowsidecar-linux serve` or document; integration test mock loop |

After wave: `cargo clippy --workspace -- -D warnings && cargo test --workspace && ./scripts/parity-loop.sh 10`
