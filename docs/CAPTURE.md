# Capture (P06)

**Target:** Hyprland / Sway on Wayland via **xdg-desktop-portal ScreenCast** and PipeWire.

## Primary path

1. `sidecar-capture::start_capture(window_hint)` opens an **org.freedesktop.portal.ScreenCast** session (`ashpd`).
2. You pick a **window** (or monitor) in the portal dialog — on Hyprland use **xdg-desktop-portal-hyprland** so single-window capture works.
3. Frames arrive as **RGBA** (`CaptureFrame { rgba, width, height, timestamp }`).

Pass a `WindowHint` from `wow_window_hint()` / `sidecar_core::list_wow_windows()` so the CLI can log which WoW surface to select. Hyprland window addresses from `hyprctl` (`0x…` hex) are normalized when matching portal or PipeWire node metadata (`identifier_matches_hint`, `hyprland_addresses_equal`).

## Skip the portal dialog (repeat capture)

After a successful ScreenCast session, note the PipeWire node id from logs (or `pw-cli ls Node | rg -i screen`). If that **screencast node is still alive** in your session, you can reconnect without the picker:

```bash
export WOWSIDECAR_CAPTURE_NODE=123   # decimal PipeWire node id
wowsidecar-linux capture-test --frames 10
```

`start_capture` connects to the **default PipeWire socket** and links to that node. If the node is gone, unset the variable and use the portal path again.

## Window hint overrides (Hyprland / Sway)

When several surfaces match WoW heuristics, pin the target without editing code:

| Variable | Example | Meaning |
|----------|---------|---------|
| `WOWSIDECAR_CAPTURE_ADDRESS` | `0x1a2b3c4d` | Pick the open window with this Hyprland address (hex with or without `0x`). |
| `WOWSIDECAR_CAPTURE_HINT` | `hyprland:0x1a2b3c4d` | Same as address form; also `sway:42`, `title:World of Warcraft`, `class:gxwindow`. |
| `WOWSIDECAR_CAPTURE_HINT` | `0x1a2b3c4d` | Bare hex address (treated as Hyprland). |

`wow_window_hint()` applies these before falling back to the first detected WoW window.

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

Requires an interactive Wayland session and approving the portal prompt (unless `WOWSIDECAR_CAPTURE_NODE` is set and valid).
