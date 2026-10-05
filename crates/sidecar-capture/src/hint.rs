use sidecar_core::DesktopWindow;

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
