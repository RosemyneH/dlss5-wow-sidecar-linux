use std::fmt::Write;

const MOD_CONTROL: u32 = 0x0002;
const MOD_ALT: u32 = 0x0001;
const MOD_SHIFT: u32 = 0x0004;
const MOD_WIN: u32 = 0x0008;

const VK_F1: u32 = 0x70;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hotkey {
    pub modifiers: u32,
    pub vk: u32,
}

fn lower_flat(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c == ' ' || c == '\t' {
            continue;
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

fn parse_key(key: &str) -> Option<u32> {
    if key.len() == 1 {
        let c = key.as_bytes()[0];
        if c.is_ascii_lowercase() {
            return Some(u32::from(c - b'a' + b'A'));
        }
        if c.is_ascii_digit() {
            return Some(u32::from(c));
        }
        return None;
    }
    if key.starts_with('f') && key.len() <= 3 {
        let n: u32 = key[1..].parse().ok()?;
        if (1..=24).contains(&n) {
            return Some(VK_F1 + n - 1);
        }
        return None;
    }

    const NAMED: [(&str, u32); 22] = [
        ("backspace", 0x08),
        ("tab", 0x09),
        ("enter", 0x0D),
        ("space", 0x20),
        ("pageup", 0x21),
        ("pagedown", 0x22),
        ("end", 0x23),
        ("home", 0x24),
        ("left", 0x25),
        ("up", 0x26),
        ("right", 0x27),
        ("down", 0x28),
        ("insert", 0x2D),
        ("delete", 0x2E),
        ("pause", 0x13),
        ("scrolllock", 0x91),
        ("numpad0", 0x60),
        ("numpad1", 0x61),
        ("numpad2", 0x62),
        ("numpad3", 0x63),
        ("ins", 0x2D),
        ("del", 0x2E),
    ];
    for (name, vk) in NAMED {
        if key == name {
            return Some(vk);
        }
    }
    None
}

pub fn parse_hotkey(text: &str) -> Option<Hotkey> {
    let flat = lower_flat(text);
    if flat.is_empty() {
        return None;
    }

    let mut hotkey = Hotkey {
        modifiers: 0,
        vk: 0,
    };
    let mut have_key = false;
    let mut start = 0usize;
    while start <= flat.len() {
        let plus = flat[start..].find('+').map(|i| start + i);
        let part = match plus {
            Some(p) => &flat[start..p],
            None => &flat[start..],
        };
        if part.is_empty() {
            return None;
        }

        match part {
            "ctrl" | "control" => hotkey.modifiers |= MOD_CONTROL,
            "alt" => hotkey.modifiers |= MOD_ALT,
            "shift" => hotkey.modifiers |= MOD_SHIFT,
            "win" => hotkey.modifiers |= MOD_WIN,
            _ => {
                if have_key {
                    return None;
                }
                let vk = parse_key(part)?;
                hotkey.vk = vk;
                have_key = true;
            }
        }

        match plus {
            Some(p) => start = p + 1,
            None => break,
        }
    }

    if !have_key {
        return None;
    }
    if hotkey.modifiers & (MOD_CONTROL | MOD_ALT | MOD_WIN) == 0 {
        return None;
    }
    Some(hotkey)
}

pub fn format_hotkey(hotkey: &Hotkey) -> String {
    let mut out = String::new();
    if hotkey.modifiers & MOD_CONTROL != 0 {
        out.push_str("Ctrl+");
    }
    if hotkey.modifiers & MOD_ALT != 0 {
        out.push_str("Alt+");
    }
    if hotkey.modifiers & MOD_SHIFT != 0 {
        out.push_str("Shift+");
    }
    if hotkey.modifiers & MOD_WIN != 0 {
        out.push_str("Win+");
    }

    let vk = hotkey.vk;
    if let Some(ch) = char::from_u32(vk) {
        if ch.is_ascii_uppercase() || ch.is_ascii_digit() {
            out.push(ch);
            return out;
        }
    }
    if (VK_F1..=VK_F1 + 23).contains(&vk) {
        let _ = write!(out, "F{}", vk - VK_F1 + 1);
        return out;
    }

    const NAMED: [(&str, u32); 22] = [
        ("Backspace", 0x08),
        ("Tab", 0x09),
        ("Enter", 0x0D),
        ("Space", 0x20),
        ("PageUp", 0x21),
        ("PageDown", 0x22),
        ("End", 0x23),
        ("Home", 0x24),
        ("Left", 0x25),
        ("Up", 0x26),
        ("Right", 0x27),
        ("Down", 0x28),
        ("Insert", 0x2D),
        ("Delete", 0x2E),
        ("Pause", 0x13),
        ("ScrollLock", 0x91),
        ("Numpad0", 0x60),
        ("Numpad1", 0x61),
        ("Numpad2", 0x62),
        ("Numpad3", 0x63),
        ("Insert", 0x2D),
        ("Delete", 0x2E),
    ];
    for (name, code) in NAMED {
        if code == vk {
            out.push_str(name);
            return out;
        }
    }
    let _ = write!(out, "0x{:02X}", vk);
    out
}

pub fn normalize_hotkey_string(text: &str) -> Option<String> {
    parse_hotkey(text).map(|h| format_hotkey(&h))
}

/// XDG GlobalShortcuts `preferred_trigger` (see shortcut-keys spec).
pub fn hotkey_to_portal_trigger(text: &str) -> Option<String> {
    let flat = lower_flat(text);
    if flat.is_empty() {
        return None;
    }
    let parts: Vec<&str> = flat.split('+').collect();
    if parts.is_empty() {
        return None;
    }
    let mut out = String::new();
    for part in &parts[..parts.len().saturating_sub(1)] {
        let token = match *part {
            "ctrl" | "control" => "Control",
            "alt" => "Alt",
            "shift" => "Shift",
            "win" | "super" | "meta" => "Super",
            _ => return None,
        };
        out.push('<');
        out.push_str(token);
        out.push('>');
    }
    let key = parts.last()?;
    let key_token = portal_key_token(key)?;
    out.push('<');
    out.push_str(key_token);
    out.push('>');
    Some(out)
}

fn portal_key_token(key: &str) -> Option<&'static str> {
    if key.len() == 1 {
        let c = key.chars().next()?;
        if c.is_ascii_alphabetic() {
            return Some(match c.to_ascii_uppercase() {
                'A' => "A",
                'B' => "B",
                'C' => "C",
                'D' => "D",
                'E' => "E",
                'F' => "F",
                'G' => "G",
                'H' => "H",
                'I' => "I",
                'J' => "J",
                'K' => "K",
                'L' => "L",
                'M' => "M",
                'N' => "N",
                'O' => "O",
                'P' => "P",
                'Q' => "Q",
                'R' => "R",
                'S' => "S",
                'T' => "T",
                'U' => "U",
                'V' => "V",
                'W' => "W",
                'X' => "X",
                'Y' => "Y",
                'Z' => "Z",
                _ => return None,
            });
        }
        if c.is_ascii_digit() {
            return Some(match c {
                '0' => "0",
                '1' => "1",
                '2' => "2",
                '3' => "3",
                '4' => "4",
                '5' => "5",
                '6' => "6",
                '7' => "7",
                '8' => "8",
                '9' => "9",
                _ => return None,
            });
        }
    }
    if key.starts_with('f') && key.len() <= 3 {
        let n: u32 = key[1..].parse().ok()?;
        if (1..=24).contains(&n) {
            return Some(match n {
                1 => "F1",
                2 => "F2",
                3 => "F3",
                4 => "F4",
                5 => "F5",
                6 => "F6",
                7 => "F7",
                8 => "F8",
                9 => "F9",
                10 => "F10",
                11 => "F11",
                12 => "F12",
                13 => "F13",
                14 => "F14",
                15 => "F15",
                16 => "F16",
                17 => "F17",
                18 => "F18",
                19 => "F19",
                20 => "F20",
                21 => "F21",
                22 => "F22",
                23 => "F23",
                24 => "F24",
                _ => return None,
            });
        }
    }
    match key {
        "backspace" => Some("BackSpace"),
        "escape" | "esc" => Some("Escape"),
        "delete" | "del" => Some("Delete"),
        "insert" | "ins" => Some("Insert"),
        "space" => Some("space"),
        "tab" => Some("Tab"),
        "enter" => Some("Return"),
        "home" => Some("Home"),
        "end" => Some("End"),
        "pageup" => Some("Page_Up"),
        "pagedown" => Some("Page_Down"),
        "left" => Some("Left"),
        "right" => Some("Right"),
        "up" => Some("Up"),
        "down" => Some("Down"),
        _ => None,
    }
}
