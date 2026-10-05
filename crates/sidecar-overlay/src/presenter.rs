use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Duration;

use crate::geometry::{poll_tracked_wow_geometry, GeometrySync};
use crate::pump::OverlayPumpOutcome;

use sidecar_core::DesktopWindow;
use softbuffer::{Context, Surface};
use thiserror::Error;
use tracing::warn;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::dpi::PhysicalSize;
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
}

struct PendingFrame {
    pixels: Vec<u8>,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

struct OverlayApp {
    context: Context<OwnedDisplayHandle>,
    desktop: DesktopWindow,
    window: Option<Rc<Window>>,
    surface: Option<Surface<OwnedDisplayHandle, Rc<Window>>>,
    backing: Vec<u32>,
    pending: Option<PendingFrame>,
    dirty: bool,
    visible: bool,
}

impl OverlayApp {
    fn new(context: Context<OwnedDisplayHandle>, desktop: DesktopWindow) -> Self {
        let len = desktop.width as usize * desktop.height as usize;
        Self {
            context,
            desktop,
            window: None,
            surface: None,
            backing: vec![0; len],
            pending: None,
            dirty: false,
            visible: true,
        }
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
        validate_frame_blit(self.desktop.width, self.desktop.height, rgba, x, y, w, h)?;
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
        let size_changed =
            self.desktop.width != desktop.width || self.desktop.height != desktop.height;
        self.desktop = desktop;
        if size_changed {
            let len = self.desktop.width as usize * self.desktop.height as usize;
            self.backing = vec![0; len];
            self.dirty = true;
        }
        if let Some(window) = &self.window {
            apply_window_chrome(window, &self.desktop);
            window.request_redraw();
        }
    }

    fn apply_pending(&mut self) {
        let Some(frame) = self.pending.take() else {
            return;
        };
        let stride = self.desktop.width as usize;
        for row in 0..frame.h as usize {
            let dst_y = frame.y as usize + row;
            let src_off = row * frame.w as usize * 4;
            for col in 0..frame.w as usize {
                let dst_x = frame.x as usize + col;
                let i = src_off + col * 4;
                let r = u32::from(frame.pixels[i]);
                let g = u32::from(frame.pixels[i + 1]);
                let b = u32::from(frame.pixels[i + 2]);
                let a = u32::from(frame.pixels[i + 3]);
                let dst = dst_y * stride + dst_x;
                self.backing[dst] = b | (g << 8) | (r << 16) | (a << 24);
            }
        }
    }

    fn present(&mut self) -> Result<(), OverlayError> {
        if self.dirty {
            self.apply_pending();
        }
        let surface = self.surface.as_mut().ok_or(OverlayError::NotReady)?;
        let size = self.window.as_ref().expect("window").inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return Ok(());
        };
        surface.resize(w, h).map_err(OverlayError::Softbuffer)?;
        let mut buffer = surface.buffer_mut().map_err(OverlayError::Softbuffer)?;
        let len = buffer.len().min(self.backing.len());
        buffer[..len].copy_from_slice(&self.backing[..len]);
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
            .with_inner_size(PhysicalSize::new(self.desktop.width, self.desktop.height))
            .with_position(PhysicalPosition::new(self.desktop.x, self.desktop.y));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Rc::new(w),
            Err(e) => {
                warn!("overlay window create failed: {e}");
                return;
            }
        };
        apply_window_chrome(&window, &self.desktop);
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
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.present() {
                    warn!("overlay present: {e}");
                }
            }
            _ => {}
        }
    }
}

/// Transparent overlay sized to a [`DesktopWindow`] rect; blits RGBA via winit + softbuffer.
pub struct OverlayPresenter {
    event_loop: EventLoop<()>,
    app: OverlayApp,
}

impl OverlayPresenter {
    pub fn for_desktop_window(desktop: &DesktopWindow) -> Result<Self, OverlayError> {
        let event_loop = EventLoop::new()?;
        let context =
            Context::new(event_loop.owned_display_handle()).map_err(OverlayError::Softbuffer)?;
        Ok(Self {
            event_loop,
            app: OverlayApp::new(context, desktop.clone()),
        })
    }

    /// Queue an RGBA patch at `(x,y)` with size `(w,h)` in overlay-local coordinates (origin = desktop window top-left).
    pub fn show_frame(
        &mut self,
        rgba: &[u8],
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    ) -> Result<(), OverlayError> {
        self.app.queue_frame(rgba, x, y, w, h)
    }

    /// Reposition/resize the overlay from a fresh `list_wow_windows()` poll (same `address` as at creation).
    pub fn refresh_geometry(&mut self) -> GeometrySync {
        let (sync, updated) = poll_tracked_wow_geometry(&self.app.desktop);
        if let Some(desktop) = updated {
            self.app.apply_desktop_geometry(desktop);
        }
        sync
    }

    /// Drive the winit loop once: poll WoW geometry, then pump events.
    pub fn pump(&mut self, timeout: Option<Duration>) -> OverlayPumpOutcome {
        self.refresh_geometry();
        OverlayPumpOutcome::from_pump_status(
            self.event_loop.pump_app_events(timeout, &mut self.app),
        )
    }

    /// Lower-level pump without compositor geometry polling.
    pub fn pump_events_only(&mut self, timeout: Option<Duration>) -> PumpStatus {
        self.event_loop.pump_app_events(timeout, &mut self.app)
    }

    pub fn desktop_geometry(&self) -> (i32, i32, u32, u32) {
        (
            self.app.desktop.x,
            self.app.desktop.y,
            self.app.desktop.width,
            self.app.desktop.height,
        )
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.app.set_visible(visible);
    }
}

fn apply_window_chrome(window: &Window, desktop: &DesktopWindow) {
    window.set_window_level(WindowLevel::AlwaysOnTop);
    let size = PhysicalSize::new(desktop.width, desktop.height);
    let _ = window.request_inner_size(size);
    window.set_outer_position(PhysicalPosition::new(desktop.x, desktop.y));
    if desktop.fullscreen {
        window.set_fullscreen(Some(Fullscreen::Borderless(None)));
    } else {
        window.set_fullscreen(None);
    }
}

fn validate_frame_blit(
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
