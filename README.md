# wowsidecar-linux

Linux-native companion to [dlss5-wow-sidecar](https://github.com/xilla420/dlss5-wow-sidecar): find WoW under Proton, track the game window on Wayland, and (eventually) capture → neural enhance → overlay **without** touching `Wow.exe`.

Inspired by the Windows tool’s safety model (out-of-process only). This is **not** a fork of its binaries — it is a new stack for **Hyprland / Sway / PipeWire**.

## Parity workflow

Windows reference: `~/Repos/dlss5-wow-sidecar`. Checklist: [docs/PARITY.md](docs/PARITY.md).

```bash
./scripts/parity-status.sh    # open items + build
./scripts/parity-loop.sh 10   # test + fail until P06–P18 done (agent loop)
```

Ten parallel workstreams (config, probes, capture, overlay, runtime, neural, presets, manager UI, install, CI) each own a `crates/*` directory to reduce merge conflicts.

## Status (v0.1)

| Milestone | State |
|-----------|--------|
| Smart scan install paths (`_classic_beta_`, `WowB.exe`, …) | **Done** (`scan`) |
| Find WoW window (Hyprland / Sway) | **Done** (`windows`) |
| PipeWire / portal window capture | Planned |
| Fullscreen overlay (Vulkan) | Planned |
| DLSS 5–class neural pass on Linux | **Research** — no Windows `nvngx_*.dll`; see [docs/ROADMAP.md](docs/ROADMAP.md) |

## Build & run

```bash
cd /home/emma/Repos/dlss5-wow-sidecar-linux
cargo build --release

./target/release/wowsidecar-linux scan
./target/release/wowsidecar-linux windows   # game running, borderless/windowed
./target/release/wowsidecar-linux doctor
```

Extra search roots:

```bash
wowsidecar-linux scan --root /mnt/games
```

## Relation to the Windows sidecar

The Windows app uses **Windows Graphics Capture** + **NGX on D3D11**. On Linux we target **xdg-desktop-portal screen cast** (per-window) and a **Vulkan** present path. True DLSS 5 neural rendering for arbitrary captured frames may require a different ML backend than NGX until NVIDIA exposes an equivalent off-game API on Linux.

## License

MIT (same spirit as upstream; not affiliated with Blizzard or NVIDIA.)
