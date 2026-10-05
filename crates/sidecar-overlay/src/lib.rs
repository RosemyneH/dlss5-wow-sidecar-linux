mod geometry;
#[cfg(feature = "layer-shell")]
mod layer_shell_presenter;
mod presenter;
mod pump;

pub use geometry::{
    desktop_rects_equal, find_window_by_address, overlay_chrome_from_desktop,
    poll_tracked_wow_geometry, resolve_tracked_desktop, sync_desktop_geometry, DesktopRect,
    GeometrySync, OverlayChrome,
};
pub use presenter::{OverlayError, OverlayPresenter};
pub use pump::OverlayPumpOutcome;
pub use winit::platform::pump_events::PumpStatus;
