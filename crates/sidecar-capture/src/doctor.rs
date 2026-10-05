use std::process::Command;

use sidecar_core::{list_wow_windows, DesktopWindow};

use crate::hint::{parse_capture_node_from_env, ENV_CAPTURE_ADDRESS, ENV_CAPTURE_NODE};
use crate::portal_restore::load_screencast_restore_token;
use crate::pw_node::{capture_auto_node_enabled, ENV_CAPTURE_AUTO_NODE};

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct PortalBackend {
    name: String,
    screencast: bool,
    use_in: Vec<String>,
}

impl PortalBackend {
    fn serves(&self, desktops: &[String]) -> bool {
        self.use_in
            .iter()
            .any(|u| desktops.iter().any(|d| d.eq_ignore_ascii_case(u)))
    }

    fn monitor_only(&self) -> bool {
        self.name == "wlr"
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
            format!("XDG_SESSION_TYPE={session}; need Wayland (or WAYLAND_DISPLAY) for ScreenCast"),
        ));
    }

    let desktops = current_desktops();
    lines.push(if desktops.is_empty() {
        CaptureDoctorLine::warn(
            "compositor",
            "XDG_CURRENT_DESKTOP unset and no Hyprland/Sway socket — portal cannot pick a backend",
        )
    } else {
        CaptureDoctorLine::ok("compositor", desktops.join(":"))
    });

    match std::env::var("DBUS_SESSION_BUS_ADDRESS") {
        Ok(addr) if !addr.is_empty() => {
            lines.push(CaptureDoctorLine::ok(
                "session dbus",
                "DBUS_SESSION_BUS_ADDRESS set",
            ));
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

    lines.push(pipewire_line());
    lines.push(tool_line("pipewire tools", &["pw-dump", "pw-cli"]));

    lines.push(match find_executable("xdg-desktop-portal") {
        Some(path) => CaptureDoctorLine::ok("portal frontend", path),
        None => CaptureDoctorLine::fail(
            "portal frontend",
            "xdg-desktop-portal not found — install it; ScreenCast has no entry point",
        ),
    });
    lines.push(screencast_backend_line(
        &installed_portal_backends(),
        &desktops,
    ));

    let windows = list_wow_windows();
    lines.push(wow_window_line(&windows));
    if let Some(count) = monitor_count() {
        lines.push(monitor_line(count));
    }

    lines.push(match parse_capture_node_from_env() {
        Some(node) => CaptureDoctorLine::ok(
            "capture source",
            format!("{ENV_CAPTURE_NODE}={node} (skips portal picker while the node is alive)"),
        ),
        None if capture_auto_node_enabled() => {
            let restore = load_screencast_restore_token()
                .map(|_| "saved ScreenCast restore token; ")
                .unwrap_or_default();
            CaptureDoctorLine::ok(
                "capture source",
                format!(
                    "{restore}portal picker on first grant, auto PW node reuse when a cast exists"
                ),
            )
        }
        None => CaptureDoctorLine::ok(
            "capture source",
            format!("portal picker every run ({ENV_CAPTURE_AUTO_NODE}=0)"),
        ),
    });

    lines.push(CaptureDoctorLine::ok(
        "smoke test",
        "wowsidecar-linux capture-test --frames 3",
    ));

    lines
}

fn current_desktops() -> Vec<String> {
    let from_env: Vec<String> = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    if !from_env.is_empty() {
        return from_env;
    }
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        return vec!["Hyprland".into()];
    }
    if std::env::var_os("SWAYSOCK").is_some() {
        return vec!["sway".into()];
    }
    Vec::new()
}

fn pipewire_line() -> CaptureDoctorLine {
    let socket =
        std::env::var_os("XDG_RUNTIME_DIR").map(|d| std::path::PathBuf::from(d).join("pipewire-0"));
    match socket {
        Some(p) if p.exists() => CaptureDoctorLine::ok("pipewire", p.display().to_string()),
        _ => CaptureDoctorLine::fail(
            "pipewire",
            "no $XDG_RUNTIME_DIR/pipewire-0 socket — `systemctl --user start pipewire`",
        ),
    }
}

fn parse_portal_file(name: &str, contents: &str) -> PortalBackend {
    let mut backend = PortalBackend {
        name: name.to_owned(),
        screencast: false,
        use_in: Vec::new(),
    };
    for line in contents.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let items = value.split(';').map(str::trim).filter(|s| !s.is_empty());
        match key.trim() {
            "Interfaces" => {
                backend.screencast = items
                    .clone()
                    .any(|i| i == "org.freedesktop.impl.portal.ScreenCast")
            }
            "UseIn" => backend.use_in = items.map(str::to_owned).collect(),
            _ => {}
        }
    }
    backend
}

fn installed_portal_backends() -> Vec<PortalBackend> {
    let data_dirs =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    let mut out: Vec<PortalBackend> = Vec::new();
    for dir in std::env::split_paths(&data_dirs) {
        let Ok(entries) = std::fs::read_dir(dir.join("xdg-desktop-portal/portals")) else {
            continue;
        };
        for path in entries.flatten().map(|e| e.path()) {
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if out.iter().any(|b| b.name == name) {
                continue;
            }
            if let Ok(contents) = std::fs::read_to_string(&path) {
                out.push(parse_portal_file(name, &contents));
            }
        }
    }
    out
}

fn screencast_backend_line(backends: &[PortalBackend], desktops: &[String]) -> CaptureDoctorLine {
    const LABEL: &str = "screencast backend";
    const INSTALL: &str = "Hyprland: xdg-desktop-portal-hyprland, Sway/wlroots: xdg-desktop-portal-wlr, KDE: xdg-desktop-portal-kde, GNOME: xdg-desktop-portal-gnome";

    let screencast: Vec<&PortalBackend> = backends.iter().filter(|b| b.screencast).collect();
    if screencast.is_empty() {
        return CaptureDoctorLine::fail(
            LABEL,
            format!("no portal backend implements ScreenCast — install one ({INSTALL})"),
        );
    }
    let serving: Vec<&PortalBackend> = screencast
        .iter()
        .copied()
        .filter(|b| b.serves(desktops))
        .collect();
    let names = |list: &[&PortalBackend]| {
        list.iter()
            .map(|b| b.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    if serving.is_empty() {
        return CaptureDoctorLine::fail(
            LABEL,
            format!(
                "ScreenCast backends [{}] do not list desktop {} in UseIn — install the one for this compositor ({INSTALL})",
                names(&screencast),
                desktops.join(":")
            ),
        );
    }
    if serving.iter().all(|b| b.monitor_only()) {
        return CaptureDoctorLine::warn(
            LABEL,
            format!(
                "{} is monitor-only — pick the output showing WoW; xdg-desktop-portal-hyprland adds per-window capture on Hyprland",
                names(&serving)
            ),
        );
    }
    CaptureDoctorLine::ok(LABEL, names(&serving))
}

fn wow_window_line(windows: &[DesktopWindow]) -> CaptureDoctorLine {
    const LABEL: &str = "wow window";
    match windows {
        [] => CaptureDoctorLine::warn(
            LABEL,
            "none found — start WoW (borderless/windowed) before capture-test so the picker target is known",
        ),
        [w] if w.fullscreen => CaptureDoctorLine::warn(
            LABEL,
            format!(
                "\"{}\" is fullscreen — switch WoW to borderless/windowed so ScreenCast and the overlay can share the output",
                w.title
            ),
        ),
        [w] => CaptureDoctorLine::ok(
            LABEL,
            format!(
                "\"{}\" {} {} at {},{} ({}x{})",
                w.title, w.compositor, w.address, w.x, w.y, w.width, w.height
            ),
        ),
        many => CaptureDoctorLine::warn(
            LABEL,
            format!(
                "{} candidates — pin one with {ENV_CAPTURE_ADDRESS}=<address> (first: {} \"{}\")",
                many.len(),
                many[0].address,
                many[0].title
            ),
        ),
    }
}

fn monitor_line(count: usize) -> CaptureDoctorLine {
    if count > 1 {
        CaptureDoctorLine::warn(
            "monitors",
            format!(
                "{count} outputs — pick the WoW window (or the monitor it is on) in the picker; a monitor without WoW is rejected"
            ),
        )
    } else {
        CaptureDoctorLine::ok("monitors", format!("{count} output"))
    }
}

fn monitor_count() -> Option<usize> {
    if let Some(out) = command_stdout("hyprctl", &["monitors"]) {
        return Some(out.lines().filter(|l| l.starts_with("Monitor ")).count());
    }
    command_stdout("swaymsg", &["-t", "get_outputs", "-r"])
        .map(|out| out.matches("\"type\": \"output\"").count())
}

fn command_stdout(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn resolve_portal_frontend_binary() -> Option<String> {
    find_executable("xdg-desktop-portal")
}

fn find_executable(cmd: &str) -> Option<String> {
    which(cmd).or_else(|| {
        ["/usr/lib", "/usr/libexec", "/usr/lib/xdg-desktop-portal"]
            .iter()
            .map(|d| std::path::Path::new(d).join(cmd))
            .find(|p| p.is_file())
            .map(|p| p.display().to_string())
    })
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

    const HYPRLAND_PORTAL: &str = "[portal]\nDBusName=org.freedesktop.impl.portal.desktop.hyprland\nInterfaces=org.freedesktop.impl.portal.Screenshot;org.freedesktop.impl.portal.ScreenCast;\nUseIn=wlroots;Hyprland;sway;\n";
    const WLR_PORTAL: &str = "[portal]\nInterfaces=org.freedesktop.impl.portal.ScreenCast;\nUseIn=wlroots;sway;Hyprland;\n";
    const GTK_PORTAL: &str =
        "[portal]\nInterfaces=org.freedesktop.impl.portal.FileChooser;\nUseIn=gnome\n";

    fn desktops(d: &str) -> Vec<String> {
        vec![d.to_owned()]
    }

    fn window(fullscreen: bool) -> DesktopWindow {
        DesktopWindow {
            compositor: "hyprland".into(),
            address: "0xabc".into(),
            title: "World of Warcraft".into(),
            class: "wow.exe".into(),
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            fullscreen,
        }
    }

    #[test]
    fn parses_portal_descriptor() {
        let b = parse_portal_file("hyprland", HYPRLAND_PORTAL);
        assert!(b.screencast);
        assert!(b.use_in.iter().any(|u| u == "Hyprland"));
        assert!(!parse_portal_file("gtk", GTK_PORTAL).screencast);
    }

    #[test]
    fn no_screencast_backend_fails() {
        let line = screencast_backend_line(
            &[parse_portal_file("gtk", GTK_PORTAL)],
            &desktops("Hyprland"),
        );
        assert_eq!(line.level, CaptureDoctorLevel::Fail);
    }

    #[test]
    fn backend_for_other_desktop_fails() {
        let line = screencast_backend_line(
            &[parse_portal_file("hyprland", HYPRLAND_PORTAL)],
            &desktops("KDE"),
        );
        assert_eq!(line.level, CaptureDoctorLevel::Fail);
        assert!(line.detail.contains("KDE"));
    }

    #[test]
    fn wlr_only_on_hyprland_warns_monitor_only() {
        let line = screencast_backend_line(
            &[parse_portal_file("wlr", WLR_PORTAL)],
            &desktops("hyprland"),
        );
        assert_eq!(line.level, CaptureDoctorLevel::Warn);
        assert!(line.detail.contains("monitor-only"));
    }

    #[test]
    fn hyprland_backend_is_ok() {
        let backends = [
            parse_portal_file("hyprland", HYPRLAND_PORTAL),
            parse_portal_file("wlr", WLR_PORTAL),
        ];
        let line = screencast_backend_line(&backends, &desktops("Hyprland"));
        assert_eq!(line.level, CaptureDoctorLevel::Ok);
    }

    #[test]
    fn wow_window_states() {
        assert_eq!(wow_window_line(&[]).level, CaptureDoctorLevel::Warn);
        assert_eq!(
            wow_window_line(&[window(false)]).level,
            CaptureDoctorLevel::Ok
        );
        assert_eq!(
            wow_window_line(&[window(true)]).level,
            CaptureDoctorLevel::Warn
        );
        let many = wow_window_line(&[window(false), window(false)]);
        assert!(many.detail.contains(ENV_CAPTURE_ADDRESS));
    }

    #[test]
    fn multi_monitor_warns() {
        assert_eq!(monitor_line(1).level, CaptureDoctorLevel::Ok);
        assert_eq!(monitor_line(2).level, CaptureDoctorLevel::Warn);
    }
}
