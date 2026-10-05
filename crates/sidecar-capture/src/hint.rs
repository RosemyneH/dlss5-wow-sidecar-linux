use sidecar_core::DesktopWindow;

pub const ENV_CAPTURE_NODE: &str = "WOWSIDECAR_CAPTURE_NODE";
pub const ENV_CAPTURE_ADDRESS: &str = "WOWSIDECAR_CAPTURE_ADDRESS";
pub const ENV_CAPTURE_HINT: &str = "WOWSIDECAR_CAPTURE_HINT";

#[derive(Debug, Clone)]
pub struct WindowHint {
    pub compositor: String,
    pub address: String,
    pub title: String,
    pub class: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl From<&DesktopWindow> for WindowHint {
    fn from(w: &DesktopWindow) -> Self {
        Self {
            compositor: w.compositor.clone(),
            address: w.address.clone(),
            title: w.title.clone(),
            class: w.class.clone(),
            x: w.x,
            y: w.y,
            width: w.width,
            height: w.height,
        }
    }
}

impl From<DesktopWindow> for WindowHint {
    fn from(w: DesktopWindow) -> Self {
        Self::from(&w)
    }
}

/// Hyprland `hyprctl clients` addresses are `0x` + hex; portal metadata may omit the prefix or embed it.
pub fn normalize_hyprland_address(address: &str) -> String {
    let trimmed = address.trim();
    let hex = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    hex.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

pub fn hyprland_addresses_equal(a: &str, b: &str) -> bool {
    let na = normalize_hyprland_address(a);
    let nb = normalize_hyprland_address(b);
    !na.is_empty() && na == nb
}

/// Match a portal / PipeWire node identifier against a compositor window hint.
pub fn identifier_matches_hint(hint: &WindowHint, identifier: &str) -> bool {
    let id = identifier.trim();
    if id.is_empty() {
        return false;
    }

    if hint.compositor.eq_ignore_ascii_case("hyprland")
        && hyprland_addresses_equal(&hint.address, id)
    {
        return true;
    }

    if id.contains(&hint.address) {
        return true;
    }

    if hint.compositor.eq_ignore_ascii_case("hyprland") {
        let norm = normalize_hyprland_address(&hint.address);
        if !norm.is_empty() && id.to_ascii_lowercase().contains(&norm) {
            return true;
        }
    }

    if hint.compositor.eq_ignore_ascii_case("sway") && id == hint.address {
        return true;
    }

    false
}

pub fn parse_capture_node_from_env() -> Option<u32> {
    std::env::var(ENV_CAPTURE_NODE)
        .ok()
        .as_deref()
        .and_then(parse_capture_node_value)
}

pub fn parse_capture_node_value(raw: &str) -> Option<u32> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    raw.parse().ok().filter(|id| *id > 0)
}

pub fn capture_address_override_from_env() -> Option<String> {
    let from_addr = std::env::var(ENV_CAPTURE_ADDRESS)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if from_addr.is_some() {
        return from_addr;
    }
    parse_capture_hint_from_env().map(|h| h.address)
}

/// `WOWSIDECAR_CAPTURE_HINT` forms:
/// - `hyprland:0xdeadbeef` or `sway:123`
/// - `0xdeadbeef` (compositor defaults to hyprland when address looks hex)
/// - `title:World of Warcraft` (match by title substring, case-insensitive)
/// - `class:gxwindow` (match by WM class / app_id)
pub fn parse_capture_hint_from_env() -> Option<WindowHint> {
    let raw = std::env::var(ENV_CAPTURE_HINT).ok()?;
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    if let Some((kind, value)) = raw.split_once(':') {
        let kind = kind.trim();
        let value = value.trim();
        if value.is_empty() {
            return None;
        }
        return Some(hint_from_parsed(kind, value));
    }

    if looks_like_hyprland_address(raw) {
        return Some(empty_hint_with_address("hyprland", raw));
    }

    if raw.chars().all(|c| c.is_ascii_digit()) {
        return Some(empty_hint_with_address("sway", raw));
    }

    None
}

fn hint_from_parsed(kind: &str, value: &str) -> WindowHint {
    let kind_lower = kind.to_ascii_lowercase();
    if kind_lower == "title" {
        return WindowHint {
            compositor: String::new(),
            address: String::new(),
            title: value.to_string(),
            class: String::new(),
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        };
    }
    if kind_lower == "class" || kind_lower == "app_id" {
        return WindowHint {
            compositor: String::new(),
            address: String::new(),
            title: String::new(),
            class: value.to_string(),
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        };
    }

    let compositor = if kind_lower == "sway" {
        "sway"
    } else {
        "hyprland"
    };
    empty_hint_with_address(compositor, value)
}

fn empty_hint_with_address(compositor: &str, address: &str) -> WindowHint {
    WindowHint {
        compositor: compositor.into(),
        address: address.into(),
        title: String::new(),
        class: String::new(),
        x: 0,
        y: 0,
        width: 0,
        height: 0,
    }
}

fn looks_like_hyprland_address(s: &str) -> bool {
    let s = s.trim();
    let hex = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn pick_wow_hint(windows: &[DesktopWindow]) -> Option<WindowHint> {
    if let Some(parsed) = parse_capture_hint_from_env() {
        if let Some(w) = windows
            .iter()
            .find(|w| desktop_window_matches_parsed(w, &parsed))
        {
            return Some(WindowHint::from(w));
        }
        if !parsed.address.is_empty() || !parsed.title.is_empty() || !parsed.class.is_empty() {
            return Some(parsed);
        }
    }

    if let Some(addr) = capture_address_override_from_env() {
        if let Some(w) = windows
            .iter()
            .find(|w| hyprland_addresses_equal(&w.address, &addr) || w.address == addr)
        {
            return Some(WindowHint::from(w));
        }
    }

    windows.first().map(WindowHint::from)
}

fn desktop_window_matches_parsed(w: &DesktopWindow, parsed: &WindowHint) -> bool {
    if !parsed.compositor.is_empty() && !w.compositor.eq_ignore_ascii_case(&parsed.compositor) {
        return false;
    }
    if !parsed.address.is_empty() {
        if w.compositor.eq_ignore_ascii_case("hyprland") {
            if !hyprland_addresses_equal(&w.address, &parsed.address) {
                return false;
            }
        } else if w.address != parsed.address {
            return false;
        }
    }
    if !parsed.title.is_empty()
        && !w
            .title
            .to_ascii_lowercase()
            .contains(&parsed.title.to_ascii_lowercase())
    {
        return false;
    }
    if !parsed.class.is_empty()
        && !w
            .class
            .to_ascii_lowercase()
            .contains(&parsed.class.to_ascii_lowercase())
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_0x_and_case() {
        assert_eq!(normalize_hyprland_address("0xAbCd"), "abcd");
        assert_eq!(normalize_hyprland_address("abcd"), "abcd");
        assert_eq!(normalize_hyprland_address(" 0X1a2b "), "1a2b");
    }

    #[test]
    fn hyprland_address_equality() {
        assert!(hyprland_addresses_equal("0x1a2b", "1A2B"));
        assert!(hyprland_addresses_equal("0x1a2b", "0x1a2b"));
        assert!(!hyprland_addresses_equal("0x1a2b", "0x1a2c"));
    }

    #[test]
    fn identifier_matches_embedded_hypr_address() {
        let hint = empty_hint_with_address("hyprland", "0xdeadbeef");
        assert!(identifier_matches_hint(&hint, "window:0xdeadbeef:foo"));
        assert!(identifier_matches_hint(&hint, "deadbeef"));
        assert!(!identifier_matches_hint(&hint, "deadbee0"));
    }

    #[test]
    fn parse_hint_compositor_address() {
        let h = hint_from_parsed("hyprland", "0xabc");
        assert_eq!(h.compositor, "hyprland");
        assert_eq!(h.address, "0xabc");
    }

    #[test]
    fn parse_hint_title_and_class() {
        let t = hint_from_parsed("title", "World of Warcraft");
        assert_eq!(t.title, "World of Warcraft");
        let c = hint_from_parsed("class", "gxwindow");
        assert_eq!(c.class, "gxwindow");
    }

    #[test]
    fn parse_capture_node_value_valid() {
        assert_eq!(parse_capture_node_value("42"), Some(42));
        assert_eq!(parse_capture_node_value("  99 "), Some(99));
    }

    #[test]
    fn parse_capture_node_value_rejects_zero() {
        assert_eq!(parse_capture_node_value("0"), None);
        assert_eq!(parse_capture_node_value(""), None);
    }

    #[test]
    fn pick_wow_hint_prefers_address_override() {
        let windows = [DesktopWindow {
            compositor: "hyprland".into(),
            address: "0xaaaa".into(),
            title: "WoW A".into(),
            class: "gxwindow".into(),
            x: 0,
            y: 0,
            width: 100,
            height: 100,
            fullscreen: false,
        }];
        let parsed = empty_hint_with_address("hyprland", "0xAAAA");
        assert!(desktop_window_matches_parsed(&windows[0], &parsed));
    }
}
