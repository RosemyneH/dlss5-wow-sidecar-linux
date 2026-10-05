mod doctor;
mod error;
mod frame;
mod hint;
mod mock;
mod portal;
mod portal_restore;
mod pw_node;
mod pw_record;
mod stream;

pub use mock::{
    enable_mock_capture, mock_capture_enabled, mock_frame, synthetic_frame_4x4, synthetic_rgba_4x4,
    MOCK_STREAM_H, MOCK_STREAM_W, SYNTHETIC_H, SYNTHETIC_W,
};

pub use doctor::{
    capture_doctor_report, doctor_level_char, format_doctor_line, resolve_portal_frontend_binary,
    wayland_ok, CaptureDoctorLevel, CaptureDoctorLine,
};
pub use error::{capture_error_remediation, enrich_portal_error, CaptureError};
pub use frame::CaptureFrame;
pub use hint::{
    capture_address_override_from_env, hyprland_addresses_equal, identifier_matches_hint,
    normalize_hyprland_address, parse_capture_hint_from_env, parse_capture_node_from_env,
    parse_capture_node_value, pick_wow_hint, WindowHint, ENV_CAPTURE_ADDRESS, ENV_CAPTURE_HINT,
    ENV_CAPTURE_NODE,
};
pub use pw_node::{
    capture_auto_node_enabled, discover_capture_node_for_hint, hint_match_score,
    is_video_capture_node, pick_capture_node_from_candidates, resolve_capture_node_id,
    PwNodeCandidate, ENV_CAPTURE_AUTO_NODE,
};
pub use stream::{start_capture_or_mock, start_mock_stream, FrameStream};

use sidecar_core::{list_wow_windows, DesktopWindow};

pub fn start_capture(window_hint: Option<WindowHint>) -> Result<FrameStream, CaptureError> {
    stream::start_capture(window_hint)
}

pub fn wow_window_hint() -> Option<WindowHint> {
    pick_wow_hint(&list_wow_windows())
}

pub fn hint_from_window(window: &DesktopWindow) -> WindowHint {
    WindowHint::from(window)
}
