mod geometry;
mod presenter;
mod pump;

pub use geometry::{
    desktop_rects_equal, find_window_by_address, poll_tracked_wow_geometry, sync_desktop_geometry,
    DesktopRect, GeometrySync,
};
pub use presenter::{OverlayError, OverlayPresenter};
pub use pump::OverlayPumpOutcome;
pub use winit::platform::pump_events::PumpStatus;
