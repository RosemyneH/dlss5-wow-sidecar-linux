use std::io;
use std::time::Duration;

use crate::geometry::{poll_tracked_wow_geometry, GeometrySync};
use crate::presenter::validate_frame_blit;
use crate::presenter::OverlayError;
use crate::pump::OverlayPumpOutcome;

use rustix::event::{poll, PollFd, PollFlags, Timespec};
use rustix::io::Errno;
use sidecar_core::DesktopWindow;
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState, Region};
use smithay_client_toolkit::delegate_compositor;
use smithay_client_toolkit::delegate_layer;
use smithay_client_toolkit::delegate_output;
use smithay_client_toolkit::delegate_registry;
use smithay_client_toolkit::delegate_shm;
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::registry_handlers;
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
    LayerSurfaceConfigure,
};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shm::{slot::SlotPool, Shm, ShmHandler};
use tracing::warn;
use wayland_client::backend::WaylandError;
use wayland_client::globals::{registry_queue_init, GlobalList};
use wayland_client::protocol::wl_shm::Format;
use wayland_client::protocol::{wl_output, wl_surface};
use wayland_client::{Connection, EventQueue, QueueHandle};
use winit::platform::pump_events::PumpStatus;

struct LayerShellApp {
    registry_state: RegistryState,
    output_state: OutputState,
    shm: Shm,
    pool: SlotPool,
    layer: LayerSurface,
    desktop: DesktopWindow,
    backing: Vec<u32>,
    dirty: bool,
    visible: bool,
    configured: bool,
    frame_pending: bool,
    exit: bool,
    width: u32,
    height: u32,
}

impl LayerShellApp {
    fn new(
        globals: &GlobalList,
        qh: &QueueHandle<Self>,
        compositor: &CompositorState,
        layer_shell: &LayerShell,
        shm: Shm,
        desktop: DesktopWindow,
    ) -> Result<Self, OverlayError> {
        let surface = compositor.create_surface(qh);
        let layer = layer_shell.create_layer_surface(
            qh,
            surface,
            Layer::Overlay,
            Some("wow-sidecar-overlay"),
            None,
        );
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.set_exclusive_zone(-1);
        let input = Region::new(compositor).map_err(layer_shell_err)?;
        layer.set_input_region(Some(input.wl_region()));
        apply_layer_geometry(&layer, &desktop);

        let width = desktop.width.max(1);
        let height = desktop.height.max(1);
        let pool = SlotPool::new(frame_bytes(width, height), &shm).map_err(layer_shell_err)?;

        layer.commit();
        Ok(Self {
            registry_state: RegistryState::new(globals),
            output_state: OutputState::new(globals, qh),
            shm,
            pool,
            layer,
            backing: vec![0; pixel_count(&desktop)],
            desktop,
            dirty: false,
            visible: true,
            configured: false,
            frame_pending: false,
            exit: false,
            width,
            height,
        })
    }

    fn set_visible(&mut self, visible: bool) {
        if self.visible == visible {
            return;
        }
        self.visible = visible;
        self.frame_pending = false;
        if visible {
            // ʕ •ᴥ•ʔ✿ an unmapped layer surface must recommit without a buffer and await configure ✿ ʕ •ᴥ•ʔ
            apply_layer_geometry(&self.layer, &self.desktop);
            self.dirty = true;
        } else {
            self.layer.wl_surface().attach(None, 0, 0);
            self.configured = false;
        }
        self.layer.commit();
    }

    fn blit(&mut self, rgba: &[u8], x: u32, y: u32, w: u32, h: u32) -> Result<(), OverlayError> {
        validate_frame_blit(self.desktop.width, self.desktop.height, rgba, x, y, w, h)?;
        if w == 0 || h == 0 {
            return Ok(());
        }
        let stride = self.desktop.width as usize;
        let (x, w) = (x as usize, w as usize);
        for (row, src) in rgba.chunks_exact(w * 4).enumerate() {
            let start = (y as usize + row) * stride + x;
            let (src, _) = src.as_chunks::<4>();
            for (dst, &[r, g, b, a]) in self.backing[start..start + w].iter_mut().zip(src) {
                *dst = premultiplied_argb(r, g, b, a);
            }
        }
        self.dirty = true;
        Ok(())
    }

    fn apply_desktop_geometry(&mut self, desktop: DesktopWindow) {
        let size_changed =
            self.desktop.width != desktop.width || self.desktop.height != desktop.height;
        self.desktop = desktop;
        if size_changed {
            self.backing = vec![0; pixel_count(&self.desktop)];
            self.dirty = true;
        }
        apply_layer_geometry(&self.layer, &self.desktop);
        self.layer.commit();
    }

    fn request_draw(&mut self, qh: &QueueHandle<Self>) -> Result<(), OverlayError> {
        if self.dirty && !self.frame_pending {
            self.draw(qh)?;
        }
        Ok(())
    }

    fn draw(&mut self, qh: &QueueHandle<Self>) -> Result<(), OverlayError> {
        if !self.visible || !self.configured {
            return Ok(());
        }
        let width = self.width as usize;
        let height = self.height as usize;
        let stride = width * 4;

        let (buffer, canvas) = self
            .pool
            .create_buffer(
                self.width as i32,
                self.height as i32,
                stride as i32,
                Format::Argb8888,
            )
            .map_err(layer_shell_err)?;

        canvas.fill(0);
        let src_stride = self.desktop.width as usize;
        let copy_w = width.min(src_stride);
        let rows = canvas
            .chunks_exact_mut(stride)
            .zip(self.backing.chunks_exact(src_stride.max(1)))
            .take(height);
        for (dst, src) in rows {
            let (dst, _) = dst.as_chunks_mut::<4>();
            for (px, &argb) in dst[..copy_w].iter_mut().zip(&src[..copy_w]) {
                *px = argb.to_le_bytes();
            }
        }

        let surface = self.layer.wl_surface();
        surface.damage_buffer(0, 0, self.width as i32, self.height as i32);
        surface.frame(qh, surface.clone());
        buffer.attach_to(surface).map_err(layer_shell_err)?;
        self.layer.commit();
        self.frame_pending = true;
        self.dirty = false;
        Ok(())
    }
}

fn apply_layer_geometry(layer: &LayerSurface, desktop: &DesktopWindow) {
    layer.set_anchor(Anchor::TOP | Anchor::LEFT);
    layer.set_margin(desktop.y, 0, 0, desktop.x);
    layer.set_size(desktop.width.max(1), desktop.height.max(1));
}

fn pixel_count(desktop: &DesktopWindow) -> usize {
    (desktop.width as usize * desktop.height as usize).max(1)
}

fn frame_bytes(width: u32, height: u32) -> usize {
    width as usize * height as usize * 4
}

fn premultiplied_argb(r: u8, g: u8, b: u8, a: u8) -> u32 {
    let a32 = u32::from(a);
    let mul = |c: u8| (u32::from(c) * a32 + 127) / 255;
    mul(b) | (mul(g) << 8) | (mul(r) << 16) | (a32 << 24)
}

fn layer_shell_err<E: std::fmt::Display>(err: E) -> OverlayError {
    OverlayError::LayerShell(err.to_string())
}

impl CompositorHandler for LayerShellApp {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        if self.layer.wl_surface() != surface {
            return;
        }
        self.frame_pending = false;
        if let Err(e) = self.request_draw(qh) {
            warn!("layer-shell present: {e}");
        }
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for LayerShellApp {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for LayerShellApp {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let (w, h) = configure.new_size;
        self.width = if w > 0 { w } else { self.desktop.width.max(1) };
        self.height = if h > 0 { h } else { self.desktop.height.max(1) };
        self.configured = true;
        self.dirty = true;
        if let Err(e) = self.draw(qh) {
            warn!("layer-shell configure draw: {e}");
        }
    }
}

impl ShmHandler for LayerShellApp {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_compositor!(LayerShellApp);
delegate_output!(LayerShellApp);
delegate_shm!(LayerShellApp);
delegate_layer!(LayerShellApp);
delegate_registry!(LayerShellApp);

impl ProvidesRegistryState for LayerShellApp {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

pub(crate) struct LayerShellOverlayPresenter {
    queue: EventQueue<LayerShellApp>,
    app: LayerShellApp,
}

impl LayerShellOverlayPresenter {
    pub fn for_desktop_window(desktop: &DesktopWindow) -> Result<Self, OverlayError> {
        let conn = Connection::connect_to_env().map_err(layer_shell_err)?;
        let (globals, mut queue) = registry_queue_init(&conn).map_err(layer_shell_err)?;
        let qh = queue.handle();

        let compositor = CompositorState::bind(&globals, &qh).map_err(layer_shell_err)?;
        let layer_shell = LayerShell::bind(&globals, &qh).map_err(layer_shell_err)?;
        let shm = Shm::bind(&globals, &qh).map_err(layer_shell_err)?;

        let mut app = LayerShellApp::new(
            &globals,
            &qh,
            &compositor,
            &layer_shell,
            shm,
            desktop.clone(),
        )?;
        queue.roundtrip(&mut app).map_err(layer_shell_err)?;

        Ok(Self { queue, app })
    }

    pub fn show_frame(
        &mut self,
        rgba: &[u8],
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    ) -> Result<(), OverlayError> {
        self.app.blit(rgba, x, y, w, h)?;
        let qh = self.queue.handle();
        self.app.request_draw(&qh)?;
        self.queue.flush().map_err(layer_shell_err)
    }

    pub fn refresh_geometry(&mut self) -> GeometrySync {
        let (sync, updated) = poll_tracked_wow_geometry(&self.app.desktop);
        if let Some(desktop) = updated {
            self.app.apply_desktop_geometry(desktop);
            if let Err(e) = self.queue.flush() {
                warn!("layer-shell geometry flush: {e}");
            }
        }
        sync
    }

    pub fn pump(&mut self, timeout: Option<Duration>) -> OverlayPumpOutcome {
        self.refresh_geometry();
        self.pump_events_outcome(timeout)
    }

    pub fn pump_events_only(&mut self, timeout: Option<Duration>) -> PumpStatus {
        match self.pump_events_outcome(timeout) {
            OverlayPumpOutcome::Continue => PumpStatus::Continue,
            OverlayPumpOutcome::Exit(code) => PumpStatus::Exit(code),
        }
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
        if let Err(e) = self.queue.flush() {
            warn!("layer-shell set_visible: {e}");
        }
    }

    fn pump_events_outcome(&mut self, timeout: Option<Duration>) -> OverlayPumpOutcome {
        if let Err(e) = dispatch_events(&mut self.queue, &mut self.app, timeout) {
            warn!("layer-shell connection lost: {e}");
            return OverlayPumpOutcome::Exit(1);
        }
        if self.app.exit {
            OverlayPumpOutcome::Exit(0)
        } else {
            OverlayPumpOutcome::Continue
        }
    }
}

fn dispatch_events(
    queue: &mut EventQueue<LayerShellApp>,
    app: &mut LayerShellApp,
    timeout: Option<Duration>,
) -> Result<(), OverlayError> {
    let dispatched = queue.dispatch_pending(app).map_err(layer_shell_err)?;
    queue.flush().map_err(layer_shell_err)?;

    if let Some(guard) = queue.prepare_read() {
        let wait = if dispatched > 0 {
            Some(Duration::ZERO)
        } else {
            timeout
        };
        if socket_readable(&guard.connection_fd(), wait)? {
            match guard.read() {
                Ok(_) => {}
                Err(WaylandError::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(layer_shell_err(e)),
            }
        }
    }

    queue.dispatch_pending(app).map_err(layer_shell_err)?;
    Ok(())
}

fn socket_readable(
    fd: &impl std::os::fd::AsFd,
    timeout: Option<Duration>,
) -> Result<bool, OverlayError> {
    let timeout = timeout.and_then(|d| Timespec::try_from(d).ok());
    let mut fds = [PollFd::new(fd, PollFlags::IN | PollFlags::ERR)];
    match poll(&mut fds, timeout.as_ref()) {
        Ok(n) => Ok(n > 0),
        Err(Errno::INTR) => Ok(false),
        Err(e) => Err(layer_shell_err(e)),
    }
}
