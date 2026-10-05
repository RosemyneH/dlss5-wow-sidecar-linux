# Capture (P06)

**Target:** Hyprland / Sway on Wayland via **xdg-desktop-portal ScreenCast** and PipeWire.

## Primary path

1. `sidecar-capture::start_capture(window_hint)` opens an **org.freedesktop.portal.ScreenCast** session (`ashpd`).
2. You pick a **window** (or monitor) in the portal dialog — on Hyprland use **xdg-desktop-portal-hyprland** so single-window capture works.
3. Frames arrive as **RGBA** (`CaptureFrame { rgba, width, height, timestamp }`).

Pass a `WindowHint` from `sidecar_core::list_wow_windows()` so the CLI can log which WoW surface to select; auto-matching the picker is portal-specific and still manual in this MVP.

## Fallback: `pw-record`

If the portal path cannot start (no session bus, denied dialog, missing portal implementation), the crate falls back to a **`pw-record` CLI** subprocess:

```bash
pw-record --media-type=Video --media-category=Capture --format rgba -a -
```

Raw RGBA is read from stdout using dimensions from `WindowHint` (or 1920×1080). Many distro builds of `pw-record` are **audio-only**; if the fallback exits immediately, fix PipeWire/portal packages and use the ScreenCast path.

## CLI smoke test

```bash
wowsidecar-linux capture-test --frames 10
```

Requires an interactive Wayland session and approving the portal prompt.
