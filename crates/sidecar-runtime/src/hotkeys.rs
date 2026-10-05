//! Global hotkeys — panic sends `SidecarCommand::Panic` (evdev default on daemon).

use std::sync::{Arc, Mutex};

use tracing::warn;

use crate::protocol::SidecarCommand;
use crate::{is_running, read, send};

#[cfg(feature = "hotkeys-evdev")]
use tracing::info;

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

impl HotkeyBindings {
    pub fn from_config(hotkeys: &sidecar_config::Hotkeys) -> Self {
        fn opt(s: &str) -> Option<String> {
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        }
        Self {
            start_stop: opt(&hotkeys.start_stop),
            toggle_overlay: opt(&hotkeys.toggle_overlay),
            toggle_hud: opt(&hotkeys.toggle_hud),
            panic_combo: Some("Ctrl+Alt+Backspace".into()),
        }
    }

    pub fn apply_evdev_default_policy(self) -> Self {
        self
    }
}

pub fn execute_panic() -> bool {
    execute_panic_with(send)
}

pub fn execute_panic_with(send_cmd: impl Fn(SidecarCommand) -> bool) -> bool {
    send_cmd(SidecarCommand::Panic)
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
    return run_evdev(stop_flag, bindings);
    #[cfg(not(feature = "hotkeys-evdev"))]
    {
        let _ = (stop_flag, bindings);
        anyhow::bail!("built without hotkeys-evdev");
    }
}

#[cfg(feature = "hotkeys-evdev")]
fn run_evdev(stop_flag: Arc<Mutex<bool>>, bindings: HotkeyBindings) -> anyhow::Result<()> {
    use evdev::{Device, EventType, KeyCode, KeyEvent};

    let mut devices: Vec<Device> = evdev::enumerate().map(|(_, d)| d).collect();
    if devices.is_empty() {
        anyhow::bail!("no evdev nodes readable");
    }
    info!("evdev hotkeys active ({} devices)", devices.len());

    let panic = bindings.panic_combo.as_deref().unwrap_or("Ctrl+Alt+Backspace");
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
                let code = KeyCode(ev.code());
                let ke = KeyEvent::new(code, ev.value());
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
                    execute_panic();
                    *stop_flag.lock().unwrap() = true;
                    continue;
                }
                if combo_matches_opt(toggle_overlay, &pressed, code) {
                    let visible = read().map(|s| s.overlay_visible != 0).unwrap_or(true);
                    let _ = send(if visible {
                        SidecarCommand::HideOverlay
                    } else {
                        SidecarCommand::ShowOverlay
                    });
                }
                if combo_matches_opt(toggle_hud, &pressed, code) {
                    let visible = read().map(|s| s.hud_visible != 0).unwrap_or(false);
                    let _ = send(if visible {
                        SidecarCommand::HideHud
                    } else {
                        SidecarCommand::ShowHud
                    });
                }
                if combo_matches_opt(start_stop, &pressed, code) && is_running() {
                    let _ = send(SidecarCommand::Stop);
                    *stop_flag.lock().unwrap() = true;
                }
            }
        }
    }
    Ok(())
}

#[cfg(feature = "hotkeys-evdev")]
use evdev::KeyCode;

#[cfg(feature = "hotkeys-evdev")]
fn combo_matches(
    spec: &str,
    pressed: &std::collections::HashSet<KeyCode>,
    trigger: KeyCode,
) -> bool {
    combo_matches_opt(Some(spec), pressed, trigger)
}

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
    Some((mods, parse_key(parts.last()?)?))
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
        s if s.len() == 1 => {
            let c = s.chars().next()?;
            let upper = c.to_ascii_uppercase();
            let offset = (upper as u32).saturating_sub(u32::from(b'A'));
            Some(KeyCode(KeyCode::KEY_A.0 + offset as u16))
        }
        _ => None,
    }
}

#[cfg(all(test, feature = "hotkeys-evdev"))]
mod tests {
    use std::collections::HashSet;

    use evdev::KeyCode;

    use super::combo_matches;

    #[test]
    fn panic_combo_simulated_keypress() {
        let mut pressed = HashSet::new();
        pressed.insert(KeyCode::KEY_LEFTCTRL);
        pressed.insert(KeyCode::KEY_LEFTALT);
        assert!(combo_matches("Ctrl+Alt+Backspace", &pressed, KeyCode::KEY_BACKSPACE));
    }
}
