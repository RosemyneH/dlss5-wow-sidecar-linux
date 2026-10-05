# sidecar-overlay

Transparent always-on-top overlay for presenting upscaled RGBA frames over the WoW window.

## Default: winit + softbuffer

Borderless transparent window tracked via `poll_tracked_wow_geometry`.

## Optional: `layer-shell` feature

Experimental **smithay-client-toolkit** / `zwlr_layer_shell_v1` path for wlroots compositors (Sway, Hyprland, river, niri, KDE Plasma 6).

| | |
|--|--|
| **Feature** | `layer-shell` |
| **Runtime** | `WOW_SIDECAR_OVERLAY_BACKEND=layer-shell` (or `layer_shell`, case-insensitive) |
| **Fallback** | winit (feature off, env unset, or init failure — logged as `layer-shell overlay unavailable`) |

```bash
cargo build -p sidecar-overlay --features layer-shell
WOW_SIDECAR_OVERLAY_BACKEND=layer-shell cargo run -p sidecar-runtime --bin wowsidecar-daemon
```

### Behaviour

- **Layer:** `overlay`, namespace `wow-sidecar-overlay`, top-left anchor with margins at the tracked WoW rect.
- **Input passthrough:** empty input region and no keyboard interactivity, so clicks and keys reach WoW.
- **Exclusive zone:** `-1`, so panels and bars do not offset the overlay.
- **Pixels:** RGBA frames are converted to premultiplied `ARGB8888` in a `wl_shm` pool.
- **Pacing:** redraws are driven by `wl_surface.frame` callbacks and only happen when a new frame arrived.
- **Hide/show:** hiding attaches a null buffer (unmap); showing recommits and waits for a fresh configure.
- **Errors:** a lost Wayland connection or compositor `closed` event ends the pump with `Exit(1)` / `Exit(0)`.

### Limits

Margins are relative to the compositor-chosen output, so multi-monitor placement is not at parity with winit yet. See P07 in `docs/PARITY-GAPS.md`.

### Lint

```bash
cargo clippy -p sidecar-overlay --all-targets --features layer-shell -- -D warnings
```
