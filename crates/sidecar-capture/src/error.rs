use thiserror::Error;

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("capture requires a Wayland session (XDG_SESSION_TYPE={0})")]
    NotWayland(String),
    #[error("portal screen cast failed: {0}")]
    Portal(String),
    #[error("pipewire stream failed: {0}")]
    PipeWire(String),
    #[error("pw-record fallback failed: {0}")]
    PwRecord(String),
    #[error("capture backend unavailable: {0}")]
    Unavailable(String),
    #[error("frame stream closed")]
    StreamClosed,
}

pub fn enrich_portal_error(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("cancel") || lower.contains("dismiss") {
        return format!(
            "{raw} — ScreenCast dialog was closed without sharing; run capture-test again and pick a window"
        );
    }
    if lower.contains("not allowed") || lower.contains("permission") {
        return format!(
            "{raw} — portal denied ScreenCast; check Flatpak/portal permissions or pick a different source"
        );
    }
    if lower.contains("dbus") || lower.contains("connection refused") {
        return format!(
            "{raw} — session D-Bus unavailable; log in via a full desktop session (`wowsidecar-linux doctor` capture section)"
        );
    }
    if lower.contains("timeout") || lower.contains("timed out") {
        return format!(
            "{raw} — no response from xdg-desktop-portal; ensure portal services are running (Hyprland: xdg-desktop-portal-hyprland)"
        );
    }
    if lower.contains("no stream") {
        return format!(
            "{raw} — approve the portal picker and choose a Window (Hyprland: install xdg-desktop-portal-hyprland)"
        );
    }
    format!("{raw} — see docs/CAPTURE.md and `wowsidecar-linux doctor` (capture section)")
}

pub fn capture_error_remediation(err: &CaptureError) -> &'static str {
    match err {
        CaptureError::NotWayland(_) => {
            "Switch to a Wayland session or set WAYLAND_DISPLAY for a Wayland compositor."
        }
        CaptureError::Portal(_) => {
            "Run `wowsidecar-linux capture-test --frames 3` in an interactive session and approve ScreenCast."
        }
        CaptureError::PipeWire(_) => {
            "Confirm PipeWire is running; after a portal session, note the node id for WOWSIDECAR_CAPTURE_NODE."
        }
        CaptureError::PwRecord(_) => {
            "Prefer the portal path; many pw-record builds are audio-only — install portal + PipeWire video capture."
        }
        CaptureError::Unavailable(_) => {
            "No frames arrived in time; the portal may still be waiting for you to pick a source."
        }
        CaptureError::StreamClosed => {
            "Capture backend exited; retry capture-test or unset WOWSIDECAR_CAPTURE_NODE."
        }
    }
}

#[cfg(test)]
mod enrich_tests {
    use super::*;

    #[test]
    fn enrich_portal_cancelled() {
        let msg = enrich_portal_error("Portal request was cancelled");
        assert!(msg.contains("without sharing"));
    }

    #[test]
    fn enrich_portal_no_stream() {
        let msg = enrich_portal_error("no stream selected in portal dialog");
        assert!(msg.contains("xdg-desktop-portal-hyprland"));
    }
}
