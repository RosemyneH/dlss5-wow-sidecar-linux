use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::geometry::{poll_tracked_wow_geometry, GeometrySync};
use crate::pump::OverlayPumpOutcome;

use sidecar_core::DesktopWindow;
use softbuffer::{Context, Surface};
use thiserror::Error;
use tracing::warn;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, OwnedDisplayHandle};
use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};
use winit::window::{Fullscreen, Window, WindowId, WindowLevel};

#[derive(Debug, Error)]
pub enum OverlayError {
    #[error("winit event loop: {0}")]
    EventLoop(#[from] winit::error::EventLoopError),
    #[error("softbuffer: {0}")]
    Softbuffer(#[from] softbuffer::SoftBufferError),
    #[error("frame must be {expected} bytes (RGBA {w}x{h}), got {got}")]
    FrameSize {
        expected: usize,
        w: u32,
        h: u32,
        got: usize,
    },
    #[error("blit region {x},{y} {w}x{h} exceeds overlay {ow}x{oh}")]
    OutOfBounds {
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        ow: u32,
        oh: u32,
    },
    #[error("overlay window is not ready yet; call pump() until resumed")]
    NotReady,
    #[cfg(feature = "layer-shell")]
    #[error("wayland layer-shell: {0}")]
    LayerShell(String),
}

struct PendingFrame {
    pixels: Vec<u8>,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OverlayExtent {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
}

// ʕ •ᴥ•ʔ✿ Window probes (hyprctl / swaymsg) report logical compositor units ✿ ʕ •ᴥ•ʔ
fn physical_extent(desktop: &DesktopWindow, scale_factor: f64) -> OverlayExtent {
    OverlayExtent {
        position: LogicalPosition::new(desktop.x, desktop.y).to_physical(scale_factor),
        size: LogicalSize::new(desktop.width, desktop.height).to_physical(scale_factor),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ChromeDelta {
    moved: bool,
    resized: bool,
    fullscreen: bool,
}

impl ChromeDelta {
    const ALL: Self = Self {
        moved: true,
        resized: true,
        fullscreen: true,
    };

    fn between(old: &DesktopWindow, new: &DesktopWindow) -> Self {
        Self {
            moved: old.x != new.x || old.y != new.y,
            resized: old.width != new.width || old.height != new.height,
            fullscreen: old.fullscreen != new.fullscreen,
        }
    }

    fn any(self) -> bool {
        self.moved || self.resized || self.fullscreen
    }
}

const GEOMETRY_POLL_INTERVAL: Duration = Duration::from_millis(50);

struct GeometryPoller {
    interval: Duration,
    last: Option<Instant>,
}

impl GeometryPoller {
    fn new(interval: Duration) -> Self {
        Self {
            interval,
            last: None,
        }
    }

    fn due(&mut self, now: Instant) -> bool {
        if self
            .last
            .is_some_and(|last| now.duration_since(last) < self.interval)
        {
            return false;
        }
        self.last = Some(now);
        true
    }
}

struct Backing {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
}

impl Backing {
    fn new(size: PhysicalSize<u32>) -> Self {
        Self {
            width: size.width,
            height: size.height,
            pixels: vec![0; size.width as usize * size.height as usize],
        }
    }

    fn resize(&mut self, size: PhysicalSize<u32>) -> bool {
        if self.width == size.width && self.height == size.height {
            return false;
        }
        *self = Self::new(size);
        true
    }

    fn blit_rgba(&mut self, frame: &PendingFrame) {
        let cols = frame.w.min(self.width.saturating_sub(frame.x)) as usize;
        let rows = frame.h.min(self.height.saturating_sub(frame.y)) as usize;
        let stride = self.width as usize;
        for row in 0..rows {
            let src = &frame.pixels[row * frame.w as usize * 4..][..cols * 4];
            let dst_off = (frame.y as usize + row) * stride + frame.x as usize;
            let px_rows = src.as_chunks::<4>().0;
            for (dst, px) in self.pixels[dst_off..dst_off + cols]
                .iter_mut()
                .zip(px_rows.iter().take(cols))
            {
                let [r, g, b, a] = [px[0], px[1], px[2], px[3]].map(u32::from);
                *dst = b | (g << 8) | (r << 16) | (a << 24);
            }
        }
    }

    fn copy_into(&self, dst: &mut [u32], dst_width: u32, dst_height: u32) {
        let cols = self.width.min(dst_width) as usize;
        let rows = self.height.min(dst_height) as usize;
        for row in 0..rows {
            let src = &self.pixels[row * self.width as usize..][..cols];
            let out = &mut dst[row * dst_width as usize..][..dst_width as usize];
            out[..cols].copy_from_slice(src);
            out[cols..].fill(0);
        }
        dst[rows * dst_width as usize..].fill(0);
    }
}

struct OverlayApp {
    context: Context<OwnedDisplayHandle>,
    desktop: DesktopWindow,
    scale_factor: f64,
    window: Option<Rc<Window>>,
    surface: Option<Surface<OwnedDisplayHandle, Rc<Window>>>,
    backing: Backing,
    pending: Option<PendingFrame>,
    dirty: bool,
    visible: bool,
}

impl OverlayApp {
    fn new(context: Context<OwnedDisplayHandle>, desktop: DesktopWindow) -> Self {
        let backing = Backing::new(physical_extent(&desktop, 1.0).size);
        Self {
            context,
            desktop,
            scale_factor: 1.0,
            window: None,
            surface: None,
            backing,
            pending: None,
            dirty: false,
            visible: true,
        }
    }

    fn resize_backing(&mut self, size: PhysicalSize<u32>) {
        if self.backing.resize(size) {
            self.dirty = true;
        }
    }

    fn apply_scale_factor(&mut self, scale_factor: f64) {
        self.scale_factor = scale_factor;
        self.resize_backing(physical_extent(&self.desktop, scale_factor).size);
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        if let Some(window) = &self.window {
            window.set_visible(visible);
        }
    }

    fn queue_frame(
        &mut self,
        rgba: &[u8],
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    ) -> Result<(), OverlayError> {
        validate_frame_blit(self.backing.width, self.backing.height, rgba, x, y, w, h)?;
        self.pending = Some(PendingFrame {
            pixels: rgba.to_vec(),
            x,
            y,
            w,
            h,
        });
        self.dirty = true;
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        Ok(())
    }

    fn apply_desktop_geometry(&mut self, desktop: DesktopWindow) {
        let delta = ChromeDelta::between(&self.desktop, &desktop);
        self.desktop = desktop;
        if !delta.any() {
            return;
        }
        if delta.resized {
            self.resize_backing(physical_extent(&self.desktop, self.scale_factor).size);
        }
        if let Some(window) = &self.window {
            apply_window_chrome(window, &self.desktop, delta);
            window.request_redraw();
        }
    }

    fn present(&mut self) -> Result<(), OverlayError> {
        if let Some(frame) = self.pending.take() {
            self.backing.blit_rgba(&frame);
        }
        let surface = self.surface.as_mut().ok_or(OverlayError::NotReady)?;
        let size = self.window.as_ref().expect("window").inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return Ok(());
        };
        surface.resize(w, h).map_err(OverlayError::Softbuffer)?;
        let mut buffer = surface.buffer_mut().map_err(OverlayError::Softbuffer)?;
        self.backing.copy_into(&mut buffer, size.width, size.height);
        buffer.present().map_err(OverlayError::Softbuffer)?;
        self.dirty = false;
        Ok(())
    }
}

impl ApplicationHandler for OverlayApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("wow-sidecar-overlay")
            .with_transparent(true)
            .with_decorations(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_inner_size(LogicalSize::new(self.desktop.width, self.desktop.height))
            .with_position(LogicalPosition::new(self.desktop.x, self.desktop.y));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Rc::new(w),
            Err(e) => {
                warn!("overlay window create failed: {e}");
                return;
            }
        };
        self.apply_scale_factor(window.scale_factor());
        apply_window_chrome(&window, &self.desktop, ChromeDelta::ALL);
        let surface = match Surface::new(&self.context, window.clone()) {
            Ok(s) => s,
            Err(e) => {
                warn!("softbuffer surface failed: {e}");
                return;
            }
        };
        window.set_visible(self.visible);
        self.window = Some(window);
        self.surface = Some(surface);
        if self.dirty {
            self.window.as_ref().unwrap().request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                mut inner_size_writer,
            } => {
                self.apply_scale_factor(scale_factor);
                let _ = inner_size_writer
                    .request_inner_size(PhysicalSize::new(self.backing.width, self.backing.height));
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::Resized(size) => {
                self.resize_backing(size);
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.present() {
                    warn!("overlay present: {e}");
                }
            }
            _ => {}
        }
    }
}

struct WinitOverlayPresenter {
    event_loop: EventLoop<()>,
    app: OverlayApp,
    geometry_poller: GeometryPoller,
}

impl WinitOverlayPresenter {
    fn for_desktop_window(desktop: &DesktopWindow) -> Result<Self, OverlayError> {
        let event_loop = EventLoop::new()?;
        let context =
            Context::new(event_loop.owned_display_handle()).map_err(OverlayError::Softbuffer)?;
        Ok(Self {
            event_loop,
            app: OverlayApp::new(context, desktop.clone()),
            geometry_poller: GeometryPoller::new(GEOMETRY_POLL_INTERVAL),
        })
    }

    fn show_frame(
        &mut self,
        rgba: &[u8],
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    ) -> Result<(), OverlayError> {
        self.app.queue_frame(rgba, x, y, w, h)
    }

    fn refresh_geometry(&mut self) -> GeometrySync {
        let (sync, updated) = poll_tracked_wow_geometry(&self.app.desktop);
        if let Some(desktop) = updated {
            self.app.apply_desktop_geometry(desktop);
        }
        sync
    }

    fn pump(&mut self, timeout: Option<Duration>) -> OverlayPumpOutcome {
        if self.geometry_poller.due(Instant::now()) {
            self.refresh_geometry();
        }
        OverlayPumpOutcome::from_pump_status(
            self.event_loop.pump_app_events(timeout, &mut self.app),
        )
    }

    fn pump_events_only(&mut self, timeout: Option<Duration>) -> PumpStatus {
        self.event_loop.pump_app_events(timeout, &mut self.app)
    }

    fn desktop_geometry(&self) -> (i32, i32, u32, u32) {
        (
            self.app.desktop.x,
            self.app.desktop.y,
            self.app.desktop.width,
            self.app.desktop.height,
        )
    }

    fn set_visible(&mut self, visible: bool) {
        self.app.set_visible(visible);
    }
}

#[cfg(feature = "layer-shell")]
use crate::layer_shell_presenter::LayerShellOverlayPresenter;

enum OverlayBackend {
    Winit(WinitOverlayPresenter),
    #[cfg(feature = "layer-shell")]
    LayerShell(LayerShellOverlayPresenter),
}

/// Transparent overlay sized to a [`DesktopWindow`] rect (winit + softbuffer by default).
pub struct OverlayPresenter {
    backend: OverlayBackend,
}

impl OverlayPresenter {
    pub fn for_desktop_window(desktop: &DesktopWindow) -> Result<Self, OverlayError> {
        #[cfg(feature = "layer-shell")]
        if layer_shell_backend_requested() {
            match LayerShellOverlayPresenter::for_desktop_window(desktop) {
                Ok(p) => {
                    return Ok(Self {
                        backend: OverlayBackend::LayerShell(p),
                    })
                }
                Err(e) => warn!("layer-shell overlay unavailable, using winit: {e}"),
            }
        }
        Ok(Self {
            backend: OverlayBackend::Winit(WinitOverlayPresenter::for_desktop_window(desktop)?),
        })
    }

    pub fn show_frame(
        &mut self,
        rgba: &[u8],
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    ) -> Result<(), OverlayError> {
        match &mut self.backend {
            OverlayBackend::Winit(p) => p.show_frame(rgba, x, y, w, h),
            #[cfg(feature = "layer-shell")]
            OverlayBackend::LayerShell(p) => p.show_frame(rgba, x, y, w, h),
        }
    }

    pub fn refresh_geometry(&mut self) -> GeometrySync {
        match &mut self.backend {
            OverlayBackend::Winit(p) => p.refresh_geometry(),
            #[cfg(feature = "layer-shell")]
            OverlayBackend::LayerShell(p) => p.refresh_geometry(),
        }
    }

    pub fn pump(&mut self, timeout: Option<Duration>) -> OverlayPumpOutcome {
        match &mut self.backend {
            OverlayBackend::Winit(p) => p.pump(timeout),
            #[cfg(feature = "layer-shell")]
            OverlayBackend::LayerShell(p) => p.pump(timeout),
        }
    }

    pub fn pump_events_only(&mut self, timeout: Option<Duration>) -> PumpStatus {
        match &mut self.backend {
            OverlayBackend::Winit(p) => p.pump_events_only(timeout),
            #[cfg(feature = "layer-shell")]
            OverlayBackend::LayerShell(p) => p.pump_events_only(timeout),
        }
    }

    pub fn desktop_geometry(&self) -> (i32, i32, u32, u32) {
        match &self.backend {
            OverlayBackend::Winit(p) => p.desktop_geometry(),
            #[cfg(feature = "layer-shell")]
            OverlayBackend::LayerShell(p) => p.desktop_geometry(),
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        match &mut self.backend {
            OverlayBackend::Winit(p) => p.set_visible(visible),
            #[cfg(feature = "layer-shell")]
            OverlayBackend::LayerShell(p) => p.set_visible(visible),
        }
    }
}

#[cfg(feature = "layer-shell")]
fn layer_shell_backend_requested() -> bool {
    std::env::var("WOW_SIDECAR_OVERLAY_BACKEND")
        .map(|v| {
            let v = v.to_ascii_lowercase();
            v == "layer-shell" || v == "layer_shell"
        })
        .unwrap_or(false)
}

fn apply_window_chrome(window: &Window, desktop: &DesktopWindow, delta: ChromeDelta) {
    window.set_window_level(WindowLevel::AlwaysOnTop);
    let extent = physical_extent(desktop, window.scale_factor());
    if delta.resized {
        let _ = window.request_inner_size(extent.size);
    }
    if delta.moved {
        window.set_outer_position(extent.position);
    }
    if delta.fullscreen {
        if desktop.fullscreen {
            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        } else {
            window.set_fullscreen(None);
        }
    }
}

pub(crate) fn validate_frame_blit(
    ow: u32,
    oh: u32,
    rgba: &[u8],
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> Result<(), OverlayError> {
    let expected = (w as usize) * (h as usize) * 4;
    if rgba.len() != expected {
        return Err(OverlayError::FrameSize {
            expected,
            w,
            h,
            got: rgba.len(),
        });
    }
    if x + w > ow || y + h > oh {
        return Err(OverlayError::OutOfBounds { x, y, w, h, ow, oh });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_frame_validates_byte_len() {
        let err = validate_frame_blit(64, 64, &[0u8; 8], 0, 0, 4, 4).expect_err("wrong size");
        assert!(matches!(err, OverlayError::FrameSize { .. }));
    }

    #[test]
    fn show_frame_rejects_out_of_bounds_blit() {
        let err = validate_frame_blit(64, 64, &[0u8; 64], 62, 62, 4, 4).expect_err("oob");
        assert!(matches!(err, OverlayError::OutOfBounds { .. }));
    }
}
