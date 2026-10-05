# Windows parity gaps (v0.4)

Tracked in [`PARITY.md`](PARITY.md). **`./scripts/parity-loop.sh`** passes when P06–P16 Linux cells start with `yes`.

## Still open (not 1:1)

| ID | Windows | Linux today | Path to `yes` |
|----|---------|-------------|----------------|
| **P06** | WGC | Auto PW node scan scored against the Hypr/Sway WoW hint (default on, `WOWSIDECAR_CAPTURE_AUTO_NODE=0` disables); `WOWSIDECAR_CAPTURE_NODE` / `_ADDRESS` / `_HINT` overrides; fallback portal ScreenCast picker → `pw-record` | Auto node only matches when a window-cast node already exists and its props carry the window address/title; first run still needs the portal picker. `yes` = picker-free capture on a fresh session for Hyprland + Sway |
| **P07** | D3D11 AOT + rect track | winit + softbuffer AOT window (default). `zwlr_layer_shell_v1` presenter behind the non-default `sidecar-overlay/layer-shell` feature, selected at runtime by `WOW_SIDECAR_OVERLAY_BACKEND=layer-shell`, falls back to winit on init failure. Both backends poll WoW geometry | Layer-shell is not in default/CI builds (`sidecar-runtime` does not forward the feature) and uses top-left anchor + margin (no multi-monitor / scale handling). `yes` = layer-shell on by default on wlroots with verified geometry sync across outputs |
| **P11** | NGX DLSS 5 + ReShade | Sharpen / passthrough MVP | Linux inference backend (ONNX/TensorRT or vendor API when available) — **not** achievable by config alone |

Trying layer-shell with the daemon:

```bash
cargo build -p sidecar-runtime --features sidecar-overlay/layer-shell
WOW_SIDECAR_OVERLAY_BACKEND=layer-shell ./target/debug/wowsidecar-daemon
```

## Effectively matched

Operator flow (scan, config, doctor, daemon, manager, hotkeys, panic, setup, themes/i18n, probes, CI) is **yes** on Linux for the fork’s bar. **P02** uses Hypr/Sway instead of HWND.

## Proton / Windows zip

Running the official Windows release under Wine/Proton is **out of scope** for this repo; this tree is the native Linux implementation.
