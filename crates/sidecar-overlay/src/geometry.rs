use sidecar_core::{list_wow_windows, DesktopWindow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesktopRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
}

impl DesktopRect {
    pub fn from_desktop(d: &DesktopWindow) -> Self {
        Self {
            x: d.x,
            y: d.y,
            width: d.width,
            height: d.height,
            fullscreen: d.fullscreen,
        }
    }

    pub fn pixel_area(self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometrySync {
    Unchanged,
    Updated,
    TargetMissing,
}

pub fn find_window_by_address<'a>(
    windows: &'a [DesktopWindow],
    address: &str,
) -> Option<&'a DesktopWindow> {
    windows.iter().find(|w| w.address == address)
}

pub fn desktop_rects_equal(a: &DesktopRect, b: &DesktopRect) -> bool {
    a.x == b.x
        && a.y == b.y
        && a.width == b.width
        && a.height == b.height
        && a.fullscreen == b.fullscreen
}

/// Compare tracked WoW window geometry against a fresh compositor listing.
pub fn sync_desktop_geometry(
    tracked: &DesktopWindow,
    windows: &[DesktopWindow],
) -> (GeometrySync, Option<DesktopWindow>) {
    match find_window_by_address(windows, &tracked.address) {
        None => (GeometrySync::TargetMissing, None),
        Some(fresh) => {
            let a = DesktopRect::from_desktop(tracked);
            let b = DesktopRect::from_desktop(fresh);
            if desktop_rects_equal(&a, &b) {
                (GeometrySync::Unchanged, None)
            } else {
                (GeometrySync::Updated, Some(fresh.clone()))
            }
        }
    }
}

pub fn poll_tracked_wow_geometry(tracked: &DesktopWindow) -> (GeometrySync, Option<DesktopWindow>) {
    sync_desktop_geometry(tracked, &list_wow_windows())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_window(address: &str, x: i32, y: i32, w: u32, h: u32) -> DesktopWindow {
        DesktopWindow {
            compositor: "test".into(),
            address: address.into(),
            title: "World of Warcraft".into(),
            class: "gxwindow".into(),
            x,
            y,
            width: w,
            height: h,
            fullscreen: false,
        }
    }

    #[test]
    fn desktop_rect_area() {
        let r = DesktopRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            fullscreen: false,
        };
        assert_eq!(r.pixel_area(), 1920 * 1080);
    }

    #[test]
    fn find_window_by_address_matches() {
        let a = sample_window("0x1", 0, 0, 100, 100);
        let b = sample_window("0x2", 10, 10, 200, 200);
        let windows = [a.clone(), b];
        let found = find_window_by_address(&windows, "0x1").unwrap();
        assert_eq!(found.address, "0x1");
    }

    #[test]
    fn sync_reports_missing_target() {
        let tracked = sample_window("gone", 0, 0, 800, 600);
        let (sync, updated) = sync_desktop_geometry(&tracked, &[]);
        assert_eq!(sync, GeometrySync::TargetMissing);
        assert!(updated.is_none());
    }

    #[test]
    fn sync_reports_move() {
        let tracked = sample_window("0xabc", 100, 200, 800, 600);
        let fresh = sample_window("0xabc", 120, 200, 800, 600);
        let (sync, updated) = sync_desktop_geometry(&tracked, &[fresh]);
        assert_eq!(sync, GeometrySync::Updated);
        assert_eq!(updated.unwrap().x, 120);
    }

    #[test]
    fn sync_unchanged_when_equal() {
        let tracked = sample_window("0xabc", 0, 0, 640, 480);
        let fresh = sample_window("0xabc", 0, 0, 640, 480);
        let (sync, updated) = sync_desktop_geometry(&tracked, &[fresh]);
        assert_eq!(sync, GeometrySync::Unchanged);
        assert!(updated.is_none());
    }
}
