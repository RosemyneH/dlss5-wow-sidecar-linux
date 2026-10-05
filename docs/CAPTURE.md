# Capture (P06)

**Target:** Hyprland / Sway on Wayland via **xdg-desktop-portal ScreenCast** and PipeWire.

## Readiness check

```bash
wowsidecar-linux doctor
```

The **capture (P06)** section walks the same order as the Windows operator flow (OS capture support → game window → capture source → smoke test). Fix any `[!]` lines before expecting `capture-test` to work; `[~]` lines are degraded but usable.

| Check | `[!]` / `[~]` means |
|-------|---------------------|
| `wayland session` | Not on Wayland — ScreenCast is unavailable. |
| `compositor` | No `XDG_CURRENT_DESKTOP` / Hyprland / Sway socket — the portal cannot route to a backend. |
| `session dbus` | No session bus — `xdg-desktop-portal` cannot start. |
| `pipewire` | No `$XDG_RUNTIME_DIR/pipewire-0` socket — `systemctl --user start pipewire`. |
| `pipewire tools` | `pw-dump` missing — automatic node reuse is disabled. |
| `portal frontend` | `xdg-desktop-portal` binary not found (`PATH`, `/usr/lib`, `/usr/libexec`). |
| `screencast backend` | Parsed from `$XDG_DATA_DIRS/xdg-desktop-portal/portals/*.portal`. `[!]` when no backend implements ScreenCast or none lists this desktop in `UseIn`; `[~]` when only the monitor-only `wlr` backend serves it. |
| `wow window` | No WoW window (start it first), WoW is fullscreen (use borderless/windowed), or several candidates (pin with `WOWSIDECAR_CAPTURE_ADDRESS`). |
| `monitors` | More than one output — pick the WoW window, or the monitor WoW is on. |
| `capture source` | Shows whether `WOWSIDECAR_CAPTURE_NODE`, automatic node reuse, or the picker will be used. |

## Primary path

1. `sidecar-capture::start_capture(window_hint)` opens an **org.freedesktop.portal.ScreenCast** session (`ashpd`).
2. You pick a **window** (or monitor) in the portal dialog — on Hyprland use **xdg-desktop-portal-hyprland** so single-window capture works.
3. Frames arrive as **RGBA** (`CaptureFrame { rgba, width, height, timestamp }`).

Pass a `WindowHint` from `wow_window_hint()` / `sidecar_core::list_wow_windows()` so the CLI can log which WoW surface to select. Hyprland window addresses from `hyprctl` (`0x…` hex) are normalized when matching portal or PipeWire node metadata (`identifier_matches_hint`, `hyprland_addresses_equal`).

Portal failures are sent on the frame channel (not only as a silent timeout). `capture-test` prints `capture_error_remediation()` hints. Portal errors are prefixed with the failing stage (`connect`, `query source types`, `create session`, `select sources`, `start`, `open PipeWire remote`) and classified from the typed ashpd error:

| Portal outcome | Message |
|----------------|---------|
| Dialog cancelled / closed, or no stream returned | ScreenCast was **cancelled or denied** — rerun and pick the WoW window. |
| `org.freedesktop.portal.Error.NotAllowed` | **Denied by portal policy** (permission store / sandbox). |
| ScreenCast interface absent, `ServiceUnknown`, or no monitor/window source types | **Missing compositor backend** — names the backend package for Hyprland, Sway/wlroots, KDE, GNOME. |
| Backend returns failure / never answers | Backend refused or hung — check `journalctl --user -u 'xdg-desktop-portal*'`. |

Before the picker opens, `AvailableSourceTypes` is queried: monitor-only backends (e.g. `xdg-desktop-portal-wlr`) are requested as monitor-only with a warning instead of failing in the dialog.

### Multi-monitor

If you share a **monitor** while a WoW window hint is known, its portal position/size is checked against the WoW window centre. Sharing an output that does not contain WoW fails with a `multi-monitor:` error listing both rectangles; sharing the right monitor works but logs that picking the window avoids capturing other surfaces. Window shares skip this check.

## Automatic PipeWire node (no env vars)

When WoW is discovered (`wow_window_hint()` / `list_wow_windows()`), `start_capture` **scans the PipeWire registry** for an active video screencast node whose metadata matches the Hyprland address, title, or class. If exactly one capture node exists in the session, it is used when you have a window hint.

This reconnects to an **existing** portal screencast without `WOWSIDECAR_CAPTURE_NODE` or address overrides. Set `WOWSIDECAR_CAPTURE_AUTO_NODE=0` to disable auto-selection.

## Skip the portal dialog (manual node id)

After a successful ScreenCast session, note the PipeWire node id from logs (or `pw-cli ls Node | rg -i screen`). If that **screencast node is still alive** in your session, you can pin it explicitly:

```bash
export WOWSIDECAR_CAPTURE_NODE=123   # decimal PipeWire node id
wowsidecar-linux capture-test --frames 10
```

`WOWSIDECAR_CAPTURE_NODE` overrides auto-discovery. If the node is gone, unset the variable and use the portal path again.

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

Requires an interactive Wayland session and approving the portal prompt (unless `WOWSIDECAR_CAPTURE_NODE` is set and valid). On failure, stderr includes remediation text and a pointer to `doctor`.

| Symptom | Likely fix |
|---------|------------|
| Dialog closed / cancelled | Run `capture-test` again and select a window |
| `NotAllowed` / denied by portal policy | Allow screen sharing for the app in the portal permission store |
| No picker appears | Check `doctor` `session dbus`, `portal frontend`, `screencast backend` lines |
| "Missing compositor backend" | Install the backend named in the error / `doctor` `screencast backend` line |
| Hyprland only shows monitors | Install `xdg-desktop-portal-hyprland` |
| `multi-monitor:` error | Pick the WoW window, or the monitor WoW is on |
| Timeout on frame 0 | Approve ScreenCast or read the portal error printed on stderr |
| Stale node env | `unset WOWSIDECAR_CAPTURE_NODE` and use ScreenCast |
