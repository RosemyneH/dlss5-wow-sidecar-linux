mod error;
mod frame;
mod hint;
mod portal;
mod pw_record;
mod stream;

pub use error::CaptureError;
pub use frame::CaptureFrame;
pub use hint::WindowHint;
pub use stream::FrameStream;

use sidecar_core::{list_wow_windows, DesktopWindow};

pub fn start_capture(window_hint: Option<WindowHint>) -> Result<FrameStream, CaptureError> {
    stream::start_capture(window_hint)
}

pub fn wow_window_hint() -> Option<WindowHint> {
    list_wow_windows().first().map(WindowHint::from)
}

pub fn hint_from_window(window: &DesktopWindow) -> WindowHint {
    WindowHint::from(window)
}
