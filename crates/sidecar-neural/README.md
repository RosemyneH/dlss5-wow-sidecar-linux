# sidecar-neural

CPU frame processors for wowsidecar-linux: map `sidecar.toml` neural settings to a `FrameProcessor`, optionally run **1–3** sharpen passes per RGBA frame.

## API

- `build_processor_from_config(&Config)` — `neural_pass`, `dlss_preset`, and `[neural]` knobs → `Passthrough`, `SimpleSharpen`, or ONNX (stub).
- `process_frame(config, layout, input, output)` — runs the configured processor `neural_passes` times (clamped to 1–3).
- `processor_id_for_config(&Config)` — stable id for doctor / status (`passthrough`, `simple_sharpen`, …).

## DLSS 5 gap (parity P11)

| Windows sidecar | This crate |
|-----------------|------------|
| NVIDIA NGX DLSS 5 inside the ReShade chain | No NGX on Linux for arbitrary portal captures |
| Motion vectors + temporal super-resolution | Spatial unsharp mask only (`simple_sharpen`) |
| `neural_pass = "reshade"` runs real DLSS | `reshade` is a **stand-in**: same config value, sharpen MVP |

**Functional MVP:** the sharpen chain (`reshade` → `SimpleSharpen`, multi-pass via `neural_passes`) is wired and testable. **P11 stays partial** until the daemon feeds captured frames through this path end-to-end and an inference backend matches NGX-class quality (planned: ONNX Runtime + optional GPU shader).

ONNX: set `neural_pass = "onnx"` and place `neural.onnx` under `wow_dir` or set `WOWSIDECAR_NEURAL_ONNX`. Builds without ORT still return `ProcessError::OnnxUnavailable`.
