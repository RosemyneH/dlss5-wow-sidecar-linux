use crate::hint::{parse_capture_node_from_env, ENV_CAPTURE_NODE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureDoctorLevel {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone)]
pub struct CaptureDoctorLine {
    pub level: CaptureDoctorLevel,
    pub label: String,
    pub detail: String,
}

impl CaptureDoctorLine {
    fn ok(label: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            level: CaptureDoctorLevel::Ok,
            label: label.into(),
            detail: detail.into(),
        }
    }

    fn warn(label: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            level: CaptureDoctorLevel::Warn,
            label: label.into(),
            detail: detail.into(),
        }
    }

    fn fail(label: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            level: CaptureDoctorLevel::Fail,
            label: label.into(),
            detail: detail.into(),
        }
    }
}

pub fn capture_doctor_report() -> Vec<CaptureDoctorLine> {
    let mut lines = Vec::new();

    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".into());
    if wayland_ok() {
        lines.push(CaptureDoctorLine::ok(
            "wayland session",
            format!("XDG_SESSION_TYPE={session}"),
        ));
    } else {
        lines.push(CaptureDoctorLine::fail(
            "wayland session",
            format!(
                "XDG_SESSION_TYPE={session}; need Wayland (or WAYLAND_DISPLAY) for ScreenCast"
            ),
        ));
    }

    match std::env::var("DBUS_SESSION_BUS_ADDRESS") {
        Ok(addr) if !addr.is_empty() => {
            lines.push(CaptureDoctorLine::ok("session dbus", "DBUS_SESSION_BUS_ADDRESS set"));
        }
        _ if std::env::var("XDG_RUNTIME_DIR").is_ok() => {
            lines.push(CaptureDoctorLine::warn(
                "session dbus",
                "DBUS_SESSION_BUS_ADDRESS unset; portal may still work via XDG_RUNTIME_DIR bus",
            ));
        }
        _ => {
            lines.push(CaptureDoctorLine::fail(
                "session dbus",
                "no DBUS_SESSION_BUS_ADDRESS or XDG_RUNTIME_DIR — xdg-desktop-portal will not start",
            ));
        }
    }

    lines.push(tool_line("pipewire", &["pw-dump", "pw-cli"]));
    lines.push(tool_line("xdg-desktop-portal", &["xdg-desktop-portal"]));

    if which("hyprctl").is_some() {
        if which("xdg-desktop-portal-hyprland").is_some() {
            lines.push(CaptureDoctorLine::ok(
                "hyprland portal",
                "xdg-desktop-portal-hyprland present (per-window ScreenCast)",
            ));
        } else {
            lines.push(CaptureDoctorLine::warn(
                "hyprland portal",
                "hyprctl found but xdg-desktop-portal-hyprland missing — install for single-window pick",
            ));
        }
    }

    if let Some(node) = parse_capture_node_from_env() {
        lines.push(CaptureDoctorLine::ok(
            ENV_CAPTURE_NODE,
            format!("{node} (skips portal picker when node is still alive)"),
        ));
    }

    lines.push(CaptureDoctorLine::ok(
        "smoke test",
        "wowsidecar-linux capture-test --frames 3",
    ));

    lines
}

pub fn wayland_ok() -> bool {
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    if session.eq_ignore_ascii_case("wayland") {
        return true;
    }
    std::env::var("WAYLAND_DISPLAY").is_ok()
}

fn tool_line(label: &str, candidates: &[&str]) -> CaptureDoctorLine {
    for cmd in candidates {
        if let Some(path) = which(cmd) {
            return CaptureDoctorLine::ok(label, path);
        }
    }
    CaptureDoctorLine::warn(
        label,
        format!("none of [{}] on PATH", candidates.join(", ")),
    )
}

fn which(cmd: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(cmd);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

pub fn doctor_level_char(level: CaptureDoctorLevel) -> char {
    match level {
        CaptureDoctorLevel::Ok => '+',
        CaptureDoctorLevel::Warn => '~',
        CaptureDoctorLevel::Fail => '!',
    }
}

pub fn format_doctor_line(line: &CaptureDoctorLine) -> String {
    format!(
        "[{}] {} — {}",
        doctor_level_char(line.level),
        line.label,
        line.detail
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wayland_ok_accepts_wayland_display() {
        let prev_wl = std::env::var("WAYLAND_DISPLAY").ok();
        let prev_sess = std::env::var("XDG_SESSION_TYPE").ok();
        std::env::set_var("WAYLAND_DISPLAY", "wayland-1");
        std::env::set_var("XDG_SESSION_TYPE", "x11");
        assert!(wayland_ok());
        if let Some(v) = prev_wl {
            std::env::set_var("WAYLAND_DISPLAY", v);
        } else {
            std::env::remove_var("WAYLAND_DISPLAY");
        }
        if let Some(v) = prev_sess {
            std::env::set_var("XDG_SESSION_TYPE", v);
        } else {
            std::env::remove_var("XDG_SESSION_TYPE");
        }
    }

    #[test]
    fn report_includes_smoke_test_hint() {
        let lines = capture_doctor_report();
        assert!(lines.iter().any(|l| l.label == "smoke test"));
    }
}
