# wowsidecar-linux

Linux-native companion to [dlss5-wow-sidecar](https://github.com/xilla420/dlss5-wow-sidecar): find WoW under Proton, track the game window on Wayland, and (eventually) capture → neural enhance → overlay **without** touching `Wow.exe`.

Inspired by the Windows tool’s safety model (out-of-process only). This is **not** a fork of its binaries — it is a new stack for **Hyprland / Sway / PipeWire**.

## Credits & upstream

**Based on** [xilla420/dlss5-wow-sidecar](https://github.com/xilla420/dlss5-wow-sidecar) — thanks to **xilla420** and everyone who built and maintained the original Windows sidecar. This Linux tree is an independent Rust codebase (portal capture, Vulkan/Wayland); feature parity is tracked in [docs/PARITY.md](docs/PARITY.md).

## Parity workflow

Windows reference: [github.com/xilla420/dlss5-wow-sidecar](https://github.com/xilla420/dlss5-wow-sidecar). Checklist: [docs/PARITY.md](docs/PARITY.md).

```bash
./scripts/parity-status.sh    # open items + build
./scripts/ci-check.sh         # same as GitHub Actions (fmt, clippy, test)
./scripts/parity-loop.sh 10   # clippy + test + fail until P06–P16 done (agent loop)
```

Ten parallel workstreams (config, probes, capture, overlay, runtime, neural, presets, manager UI, install, CI) each own a `crates/*` directory to reduce merge conflicts.

## Status (v0.1)

| Milestone | State |
|-----------|--------|
| Smart scan install paths (`_classic_beta_`, `WowB.exe`, …) | **Done** (`scan`) |
| Find WoW window (Hyprland / Sway) | **Done** (`windows`) |
| PipeWire / portal window capture | **Partial** (`capture-test`) |
| Overlay present (winit/softbuffer) | **Partial** (`sidecar-overlay`) |
| Manager UI (egui) | **Yes** — live daemon status, Start/Stop, setup rows (`wowsidecar-manager`) |
| Daemon + IPC | **Partial** (`wowsidecar-daemon`, `start`/`status`) |
| DLSS 5–class neural pass on Linux | **Partial** — `sidecar-neural` CPU sharpen MVP; see gap below |

## Build & run

```bash
git clone https://github.com/RosemyneH/dlss5-wow-sidecar-linux.git
cd dlss5-wow-sidecar-linux
cargo build --release
```

### End-to-end operator flow (Wayland)

1. **Prerequisites** — Wayland session (Hyprland or Sway), PipeWire, `xdg-desktop-portal` + compositor portal backend, WoW via Proton in **borderless/windowed** (not exclusive fullscreen).

2. **Discover install** — `./target/release/wowsidecar-linux scan` (optional `--root` for extra Steam libraries).

3. **Start WoW**, then **confirm the game window** — `./target/release/wowsidecar-linux windows`.

4. **Environment check** — `./target/release/wowsidecar-linux doctor` (PipeWire, portal, preset, daemon socket path).

5. **Optional: validate capture** — `./target/release/wowsidecar-linux capture-test --frames 10` (portal picker; WoW window hint when available). Optional direct PipeWire node: `WOWSIDECAR_CAPTURE_NODE=<id>` — see `docs/CAPTURE.md`.

6. **Run the sidecar daemon** (foreground, recommended for first run):
   ```bash
   ./target/release/wowsidecar-linux serve
   ```
   Same process as `run` / `wowsidecar-daemon`; logs the control socket and accepts IPC until Ctrl+C.

7. **Background daemon** (second terminal) — `./target/release/wowsidecar-linux start`, then `./target/release/wowsidecar-linux status` (`--json` for machine-readable).

8. **Manager UI** (optional) — `cargo build -p wowsidecar-manager && ./target/release/wowsidecar-manager`.

9. **Stop** — `./target/release/wowsidecar-linux stop` (or Ctrl+C when using `serve`).

```bash
./target/release/wowsidecar-linux scan
./target/release/wowsidecar-linux windows
./target/release/wowsidecar-linux doctor
./target/release/wowsidecar-linux capture-test --frames 10
# Optional PipeWire node (skip portal): WOWSIDECAR_CAPTURE_NODE=<id> — see docs/CAPTURE.md
./target/release/wowsidecar-linux serve
./target/release/wowsidecar-linux status
cargo build -p wowsidecar-manager && ./target/release/wowsidecar-manager
```

Extra search roots:

```bash
wowsidecar-linux scan --root /mnt/games
```

## Relation to the Windows sidecar

The Windows app uses **Windows Graphics Capture** + **NGX on D3D11**. On Linux we target **xdg-desktop-portal screen cast** (per-window) and a **Vulkan** present path. True DLSS 5 neural rendering for arbitrary captured frames may require a different ML backend than NGX until NVIDIA exposes an equivalent off-game API on Linux.

## Neural pass vs DLSS 5 (gap)

| | Windows sidecar | `sidecar-neural` (Linux) |
|--|-----------------|--------------------------|
| Backend | NVIDIA NGX (DLSS 5) + ReShade chain | CPU **unsharp mask** (`SimpleSharpen`) or **Passthrough** |
| Input | In-process D3D11 color (+ motion vectors in NGX path) | Planned: portal-captured **RGBA8** frames |
| Quality | Temporal super-resolution, trained on game content | Spatial sharpen only; no motion, no AI reconstruction |
| Future path | NGX updates on Windows | `OnnxProcessorConfig` / `OnnxRuntime` hook for ONNX Runtime; optional GPU compute shader later |

Parity item **P11** is **partial** (NGX/DLSS 5 gap) but the **sharpen chain MVP is functional**: `sidecar_neural::process_frame` + `build_processor_from_config` from `sidecar.toml`. See [crates/sidecar-neural/README.md](crates/sidecar-neural/README.md).

## License

MIT (same spirit as upstream; not affiliated with Blizzard or NVIDIA.)
