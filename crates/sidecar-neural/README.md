# sidecar-neural

CPU frame processors for wowsidecar-linux: map `sidecar.toml` neural settings to a `FrameProcessor`, optionally run **1–4** sharpen passes per RGBA frame (same range `sidecar-config` accepts for `neural_passes`).

## API

- `build_processor_from_config(&Config)` — `neural_pass`, `dlss_preset`, and `[neural]` knobs → `Passthrough`, `SimpleSharpen`, or ONNX (stub).
- `process_frame(config, layout, input, output)` — runs the configured processor `neural_passes` times (clamped to 1–4).
- `NeuralChain::from_config` / `reload(&Config)` — cached chain for per-frame config reloads. `reload` returns `true` when the spec (backend + pass count) changed; the processor is rebuilt only when the backend changes, and a failed rebuild (e.g. missing ONNX model) keeps the previous chain active.

## Windows presets

| Preset | `neural_pass` / `dlss_preset` | Processor | Sharpen amount |
|--------|-------------------------------|-----------|----------------|
| Recommended | `reshade` / `cnn-f` | `simple_sharpen` | `intensity` (1.0) |
| Softer | `reshade` / `cnn-f` | `simple_sharpen` | `intensity` (0.60) |
| Most stable | `reshade` / `cnn-e` | `simple_sharpen` | `intensity × 0.92` (0.85 → 0.782) |
| Off (A/B baseline) | `passthrough` | `passthrough` | — (identity at any pass count) |

Pass count (`neural_passes`) is independent of the preset, as on Windows: manager strengths map 1/2/3 passes, and TOML may set 4.
- `processor_id_for_config(&Config)` — stable id for doctor / status (`passthrough`, `simple_sharpen`, …).

## DLSS 5 gap (parity P11)

| Windows sidecar | This crate |
|-----------------|------------|
| NVIDIA NGX DLSS 5 inside the ReShade chain | No NGX on Linux for arbitrary portal captures |
| Motion vectors + temporal super-resolution | Spatial unsharp mask only (`simple_sharpen`) |
| `neural_pass = "reshade"` runs real DLSS | `reshade` is a **stand-in**: same config value, sharpen MVP |

**Functional MVP:** the sharpen chain (`reshade` → `SimpleSharpen`, multi-pass via `neural_passes`) is wired and testable. **P11 stays partial** until the daemon feeds captured frames through this path end-to-end and an inference backend matches NGX-class quality (planned: ONNX Runtime + optional GPU shader).

ONNX: set `neural_pass = "onnx"` and place `neural.onnx` under `wow_dir` or set `WOWSIDECAR_NEURAL_ONNX`. Builds without ORT still return `ProcessError::OnnxUnavailable`.
