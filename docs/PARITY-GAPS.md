# Windows parity gaps (v0.4)

Tracked in [`PARITY.md`](PARITY.md). **`./scripts/parity-loop.sh`** passes when P06–P16 Linux cells start with `yes`.

## Still open (not 1:1)

| ID | Windows | Linux today | Path to `yes` |
|----|---------|-------------|----------------|
| **P11** | NGX DLSS 5 + ReShade | Sharpen / passthrough MVP | Linux inference backend (ONNX/TensorRT or vendor API when available) — **not** achievable by config alone |

**P06 / P07** are marked `yes` in [`PARITY.md`](PARITY.md) for the Linux operator bar: portal grant once (restore token), auto PW node reuse, layer-shell overlay on default daemon builds. Multi-monitor edge cases and monitor-only `wlr` backends remain documented in [`CAPTURE.md`](CAPTURE.md).

Force winit overlay:

```bash
WOWSIDECAR_OVERLAY_BACKEND=winit ./target/debug/wowsidecar-daemon
```

## Effectively matched

Operator flow (scan, config, doctor, daemon, manager, hotkeys, panic, setup, themes/i18n, probes, CI) is **yes** on Linux for the fork’s bar. **P02** uses Hypr/Sway instead of HWND.

## Proton / Windows zip

Running the official Windows release under Wine/Proton is **out of scope** for this repo; this tree is the native Linux implementation.
