//! Global hotkeys without injecting into WoW.
//!
//! # Limitations (Linux vs Windows `RegisterHotKey`)
//!
//! - **evdev** (`hotkeys-evdev` feature): reads `/dev/input/event*` directly. Requires
//!   membership in the `input` group (or root). Competes with the compositor for key
//!   events on some setups; X11 sessions may still deliver keys to focused apps first.
//! - **xdg-desktop-portal GlobalShortcuts**: correct for Wayland, but needs a persistent
//!   portal session and user approval in the desktop dialog. Not wired yet — manager focus
//!   hotkeys remain future work.
//! - **Panic** (`Ctrl+Alt+Backspace`) is handled in the daemon when evdev is enabled;
//!   otherwise use `wowsidecar-linux stop` or the manager Stop button.

use std::sync::{Arc, Mutex};

use tracing::warn;

#[cfg(feature = "hotkeys-evdev")]
use tracing::info;

#[cfg(feature = "hotkeys-evdev")]
use crate::protocol::SidecarCommand;

#[cfg(feature = "hotkeys-evdev")]
use crate::{is_running, read, send};

pub struct HotkeyBindings {
    pub start_stop: Option<String>,
    pub toggle_overlay: Option<String>,
    pub toggle_hud: Option<String>,
    pub panic_combo: Option<String>,
}

impl Default for HotkeyBindings {
    fn default() -> Self {
        Self {
            start_stop: Some("Ctrl+Alt+S".into()),
            toggle_overlay: Some("Ctrl+Alt+D".into()),
            toggle_hud: Some("Ctrl+Alt+H".into()),
            panic_combo: Some("Ctrl+Alt+Backspace".into()),
        }
    }
}

pub fn spawn_hotkey_thread(stop_flag: Arc<Mutex<bool>>, bindings: HotkeyBindings) {
    std::thread::spawn(move || {
        if let Err(e) = run_hotkeys(stop_flag, bindings) {
            warn!("hotkeys unavailable: {e:#}");
        }
    });
}

fn run_hotkeys(stop_flag: Arc<Mutex<bool>>, bindings: HotkeyBindings) -> anyhow::Result<()> {
    #[cfg(feature = "hotkeys-evdev")]
    {
        return run_evdev(stop_flag, bindings);
    }

    #[cfg(not(feature = "hotkeys-evdev"))]
    {
        let _ = (stop_flag, bindings);
        anyhow::bail!(
            "built without `hotkeys-evdev`; global shortcuts need evdev or portal (see module docs)"
        );
    }
}

#[cfg(feature = "hotkeys-evdev")]
fn run_evdev(stop_flag: Arc<Mutex<bool>>, bindings: HotkeyBindings) -> anyhow::Result<()> {
    use evdev::{Device, EventType, KeyCode, KeyEvent};

    let mut devices: Vec<Device> = evdev::enumerate()
        .filter_map(|(_, d)| d.open().ok())
        .collect();
    if devices.is_empty() {
        anyhow::bail!("no evdev nodes readable (add user to `input` group?)");
    }

    info!(
        "evdev hotkeys active ({} devices); portal GlobalShortcuts not implemented",
        devices.len()
    );

    let panic = bindings
        .panic_combo
        .as_deref()
        .unwrap_or("Ctrl+Alt+Backspace");
    let toggle_overlay = bindings.toggle_overlay.as_deref();
    let toggle_hud = bindings.toggle_hud.as_deref();
    let start_stop = bindings.start_stop.as_deref();

    let mut pressed = std::collections::HashSet::new();

    loop {
        if *stop_flag.lock().unwrap() {
            break;
        }
        for dev in &mut devices {
            for ev in dev.fetch_events()? {
                if ev.event_type() != EventType::KEY {
                    continue;
                }
                let ke = KeyEvent::new(ev.code(), ev.value());
                let code = ke.code();
                if ke.value() == 1 {
                    pressed.insert(code);
                } else if ke.value() == 0 {
                    pressed.remove(&code);
                } else {
                    continue;
                }

                if ke.value() != 1 {
                    continue;
                }

                if combo_matches(panic, &pressed, code) {
                    info!("panic hotkey");
                    let _ = send(SidecarCommand::Stop);
                    *stop_flag.lock().unwrap() = true;
                    continue;
                }
                if combo_matches_opt(toggle_overlay, &pressed, code) {
                    let visible = read().map(|s| s.overlay_visible != 0).unwrap_or(true);
                    let cmd = if visible {
                        SidecarCommand::HideOverlay
                    } else {
                        SidecarCommand::ShowOverlay
                    };
                    let _ = send(cmd);
                }
                if combo_matches_opt(toggle_hud, &pressed, code) {
                    let visible = read().map(|s| s.hud_visible != 0).unwrap_or(false);
                    let cmd = if visible {
                        SidecarCommand::HideHud
                    } else {
                        SidecarCommand::ShowHud
                    };
                    let _ = send(cmd);
                }
                if combo_matches_opt(start_stop, &pressed, code) {
                    if is_running() {
                        let _ = send(SidecarCommand::Stop);
                        *stop_flag.lock().unwrap() = true;
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(feature = "hotkeys-evdev")]
fn combo_matches(
    spec: &str,
    pressed: &std::collections::HashSet<KeyCode>,
    trigger: KeyCode,
) -> bool {
    combo_matches_opt(Some(spec), pressed, trigger)
}

#[cfg(feature = "hotkeys-evdev")]
use evdev::KeyCode;

#[cfg(feature = "hotkeys-evdev")]
fn combo_matches_opt(
    spec: Option<&str>,
    pressed: &std::collections::HashSet<KeyCode>,
    trigger: KeyCode,
) -> bool {
    let Some(spec) = spec else {
        return false;
    };
    let Some((mods, key)) = parse_combo(spec) else {
        return false;
    };
    if trigger != key {
        return false;
    }
    mods.iter().all(|m| pressed.contains(m))
}

#[cfg(feature = "hotkeys-evdev")]
fn parse_combo(spec: &str) -> Option<(Vec<KeyCode>, KeyCode)> {
    let parts: Vec<&str> = spec.split('+').map(|s| s.trim()).collect();
    if parts.is_empty() {
        return None;
    }
    let mut mods = Vec::new();
    for part in &parts[..parts.len().saturating_sub(1)] {
        mods.push(parse_modifier(part)?);
    }
    let key = parse_key(parts.last()?)?;
    Some((mods, key))
}

#[cfg(feature = "hotkeys-evdev")]
fn parse_modifier(name: &str) -> Option<KeyCode> {
    match name.to_ascii_lowercase().as_str() {
        "ctrl" | "control" => Some(KeyCode::KEY_LEFTCTRL),
        "alt" => Some(KeyCode::KEY_LEFTALT),
        "shift" => Some(KeyCode::KEY_LEFTSHIFT),
        "super" | "meta" | "win" => Some(KeyCode::KEY_LEFTMETA),
        _ => None,
    }
}

#[cfg(feature = "hotkeys-evdev")]
fn parse_key(name: &str) -> Option<KeyCode> {
    match name.to_ascii_lowercase().as_str() {
        "backspace" => Some(KeyCode::KEY_BACKSPACE),
        "escape" | "esc" => Some(KeyCode::KEY_ESC),
        s if s.len() == 1 => {
            let c = s.chars().next()?;
            let upper = c.to_ascii_uppercase().next()?;
            let offset = (upper as u32).saturating_sub(u32::from(b'A'));
            Some(KeyCode::KEY_A.code() + offset as u16)
        }
        _ => None,
    }
}
