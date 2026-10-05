# Windows parity gaps (v0.4)

Tracked in [`PARITY.md`](PARITY.md). **`./scripts/parity-loop.sh`** passes when P06–P16 Linux cells start with `yes`.

## Still open (not 1:1)

| ID | Windows | Linux today | Path to `yes` |
|----|---------|-------------|----------------|
| **P06** | WGC | PipeWire portal + env node/hint | Reliable Hyprland/Wayland window capture without manual `WOWSIDECAR_CAPTURE_*`; portal session UX |
| **P07** | D3D11 AOT + rect track | winit presenter | Wayland **layer-shell** (or compositor-specific overlay) + tight WoW geometry sync |
| **P11** | NGX DLSS 5 + ReShade | Sharpen / passthrough MVP | Linux inference backend (ONNX/TensorRT or vendor API when available) — **not** achievable by config alone |

## Effectively matched

Operator flow (scan, config, doctor, daemon, manager, hotkeys, panic, setup, themes/i18n, probes, CI) is **yes** on Linux for the fork’s bar. **P02** uses Hypr/Sway instead of HWND.

## Proton / Windows zip

Running the official Windows release under Wine/Proton is **out of scope** for this repo; this tree is the native Linux implementation.
