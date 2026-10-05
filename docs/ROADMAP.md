# Roadmap

## M1 — Discovery (current)

- Filesystem smart scan for Battle.net layouts and common `~/Games` trees.
- Compositor window enumeration (Hyprland `hyprctl`, Sway `swaymsg`).
- CLI: `scan`, `windows`, `doctor`.

## M2 — Capture

- `ashpd` / `xdg-desktop-portal` **ScreenCast** with **single-window** selection (match Hyprland address / app_id).
- PipeWire stream → RGBA frames, timestamped.
- Document latency and FPS caps (portal/compositor dependent).

## M3 — Overlay

- Borderless fullscreen layer (Vulkan + `winit` or `smithay`-friendly approach on Hyprland).
- Track game window geometry; align overlay to client area.
- Hotkeys via compositor/global shortcuts or evdev (no injection into WoW).

## M4 — Neural pass

Options to evaluate (in order of practicality):

1. **Shader-based** sharpen / upscale (not DLSS 5, but shippable).
2. **ONNX / TensorRT** custom model on captured frames (heavy; legal/model weights TBD).
3. **NGX on Linux** if/when usable outside game-integrated paths on RTX 40/50.

Until M4 lands, the project still helps Linux players **verify install path and window** before any effect runs.

## Non-goals

- DLL injection, `dxgi.dll` next to `Wow.exe`, or ReShade-in-game (Blizzard ban risk).
- Promising parity with Windows DLSS 5 before a real Linux inference path exists.
