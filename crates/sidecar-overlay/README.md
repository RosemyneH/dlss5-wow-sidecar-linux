# sidecar-overlay

Transparent always-on-top overlay for presenting upscaled RGBA frames over the WoW window.

## Default: winit + softbuffer

Borderless transparent window tracked via `poll_tracked_wow_geometry`.

## Optional: `layer-shell` feature

Experimental **smithay-client-toolkit** / `zwlr_layer_shell_v1` path for wlroots compositors.

| | |
|--|--|
| **Feature** | `layer-shell` |
| **Runtime** | `WOW_SIDECAR_OVERLAY_BACKEND=layer-shell` |
| **Fallback** | winit (feature off, env unset, or init failure) |

```bash
cargo build -p sidecar-overlay --features layer-shell
WOW_SIDECAR_OVERLAY_BACKEND=layer-shell cargo run -p wowsidecar-daemon
```

Layer surfaces use top-left anchor + margin (not full multi-monitor parity yet). See P07 in `docs/PARITY-GAPS.md`.
