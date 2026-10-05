# Global hotkeys (Linux, no game injection)

Windows sidecar uses `RegisterHotKey`. On Linux we support three paths:

1. **xdg-desktop-portal GlobalShortcuts** (Wayland, preferred when built in)
2. **evdev** (`hotkeys-evdev` Cargo feature on `sidecar-runtime`)
3. **Compositor binds** calling `wowsidecar-linux send …` (always available)

Bindings come from `[hotkeys]` in `sidecar.toml` (`sidecar-config`). Empty string disables a binding. **Panic** is always `Ctrl+Alt+Backspace` (not configurable).

| Config key | Default | `send` helper |
|------------|---------|---------------|
| `toggle_overlay` | `Ctrl+Alt+D` | `toggle-overlay` |
| `toggle_hud` | `Ctrl+Alt+H` | `toggle-hud` |
| `start_stop` | `Ctrl+Alt+S` | `stop` |
| panic | `Ctrl+Alt+Backspace` | `panic` |

## CLI / IPC

```bash
wowsidecar-linux send toggle-overlay
wowsidecar-linux send panic
wowsidecar-linux status
wowsidecar-linux doctor   # prints configured hotkeys
```

Commands map to `SidecarCommand` on the Unix control socket (`sidecar-runtime`).

## Cargo features (`sidecar-runtime`)

| Feature | Effect |
|---------|--------|
| `hotkeys-evdev-panic` (default) | evdev listens for **panic only** |
| `hotkeys-evdev-full` | evdev also handles overlay/HUD/start-stop from config |
| `hotkeys-portal` | Try portal GlobalShortcuts first, then evdev if enabled |

Full daemon build example:

```bash
cargo build -p sidecar-runtime --features hotkeys-evdev-full,hotkeys-portal
```

### evdev notes

- Add your user to the **`input`** group (or run as root) so `/dev/input/event*` is readable.
- evdev can compete with the compositor; prefer portal or Hypr binds when possible.

## Hyprland example

See [hyprland-hotkeys.conf](hyprland-hotkeys.conf). Source from `hyprland.conf`:

```ini
source = ~/.config/hypr/wowsidecar-hotkeys.conf
```

Adjust key combos to match your `sidecar.toml`.
