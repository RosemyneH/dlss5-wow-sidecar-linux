use std::convert::TryInto;
use std::time::{Duration, Instant};

use crate::geometry::{poll_tracked_wow_geometry, GeometrySync};
use crate::presenter::validate_frame_blit;
use crate::presenter::OverlayError;
use crate::pump::OverlayPumpOutcome;

use sidecar_core::DesktopWindow;
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
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
use wayland_client::globals::{registry_queue_init, GlobalList};
use wayland_client::protocol::wl_shm::Format;
use wayland_client::protocol::{wl_output, wl_surface};
use wayland_client::{Connection, DispatchError, EventQueue, QueueHandle};
use winit::platform::pump_events::PumpStatus;

struct PendingFrame {
    pixels: Vec<u8>,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

struct LayerShellApp {
    registry_state: RegistryState,
    output_state: OutputState,
    shm: Shm,
    pool: SlotPool,
    layer: LayerSurface,
    desktop: DesktopWindow,
    backing: Vec<u32>,
    pending: Option<PendingFrame>,
    dirty: bool,
    visible: bool,
    configured: bool,
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
        apply_layer_geometry(&layer, &desktop);

        let width = desktop.width.max(1);
        let height = desktop.height.max(1);
        let pool = SlotPool::new((width * height * 4) as usize, &shm).map_err(layer_shell_err)?;

        let len = width as usize * height as usize;
        let mut app = Self {
            registry_state: RegistryState::new(globals),
            output_state: OutputState::new(globals, qh),
            shm,
            pool,
            layer,
            desktop,
            backing: vec![0; len],
            pending: None,
            dirty: false,
            visible: true,
            configured: false,
            exit: false,
            width,
            height,
        };
        app.layer.commit();
        Ok(app)
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        self.dirty = true;
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
        Ok(())
    }

    fn apply_desktop_geometry(&mut self, desktop: DesktopWindow) {
        let size_changed =
            self.desktop.width != desktop.width || self.desktop.height != desktop.height;
        self.desktop = desktop;
        if size_changed {
            let len = self.desktop.width as usize * self.desktop.height as usize;
            self.backing = vec![0; len.max(1)];
            self.width = self.desktop.width.max(1);
            self.height = self.desktop.height.max(1);
            self.dirty = true;
            let bytes = self.width.saturating_mul(self.height).saturating_mul(4);
            match SlotPool::new(bytes as usize, &self.shm) {
                Ok(pool) => self.pool = pool,
                Err(e) => warn!("layer-shell shm pool resize: {e}"),
            }
        }
        apply_layer_geometry(&self.layer, &self.desktop);
        self.layer.commit();
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
                if dst < self.backing.len() {
                    self.backing[dst] = b | (g << 8) | (r << 16) | (a << 24);
                }
            }
        }
    }

    fn draw(&mut self, qh: &QueueHandle<Self>) -> Result<(), OverlayError> {
        if !self.visible {
            self.layer.wl_surface().attach(None, 0, 0);
            self.layer.commit();
            self.dirty = false;
            return Ok(());
        }
        if self.dirty {
            self.apply_pending();
        }
        let width = self.width;
        let height = self.height;
        let stride = width as i32 * 4;

        let (buffer, canvas) = self
            .pool
            .create_buffer(width as i32, height as i32, stride, Format::Argb8888)
            .map_err(layer_shell_err)?;

        for (i, chunk) in canvas.chunks_exact_mut(4).enumerate() {
            let pixel = self.backing.get(i).copied().unwrap_or(0);
            let array: &mut [u8; 4] = chunk.try_into().expect("pixel");
            *array = pixel.to_le_bytes();
        }

        self.layer
            .wl_surface()
            .damage_buffer(0, 0, width as i32, height as i32);
        buffer
            .attach_to(self.layer.wl_surface())
            .map_err(layer_shell_err)?;
        self.layer
            .wl_surface()
            .frame(qh, self.layer.wl_surface().clone());
        self.layer.commit();
        self.dirty = false;
        Ok(())
    }
}

fn apply_layer_geometry(layer: &LayerSurface, desktop: &DesktopWindow) {
    layer.set_anchor(Anchor::TOP | Anchor::LEFT);
    layer.set_margin(desktop.y, 0, 0, desktop.x);
    layer.set_size(desktop.width.max(1), desktop.height.max(1));
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
        if let Err(e) = self.draw(qh) {
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
        if configure.new_size.0 > 0 && configure.new_size.1 > 0 {
            self.width = configure.new_size.0;
            self.height = configure.new_size.1;
        }
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
    conn: Connection,
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
        queue.roundtrip(&mut app).map_err(map_dispatch_err)?;

        Ok(Self { conn, queue, app })
    }

    pub fn show_frame(
        &mut self,
        rgba: &[u8],
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    ) -> Result<(), OverlayError> {
        self.app.queue_frame(rgba, x, y, w, h)?;
        if self.app.configured {
            let qh = self.queue.handle();
            self.app.draw(&qh)?;
            self.conn.flush().map_err(layer_shell_err)?;
        }
        Ok(())
    }

    pub fn refresh_geometry(&mut self) -> GeometrySync {
        let (sync, updated) = poll_tracked_wow_geometry(&self.app.desktop);
        if let Some(desktop) = updated {
            self.app.apply_desktop_geometry(desktop);
            let _ = self.conn.flush();
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
        if self.app.configured {
            let qh = self.queue.handle();
            if let Err(e) = self.app.draw(&qh) {
                warn!("layer-shell set_visible: {e}");
            }
            let _ = self.conn.flush();
        }
    }

    fn pump_events_outcome(&mut self, timeout: Option<Duration>) -> OverlayPumpOutcome {
        if let Err(e) = dispatch_events(&self.conn, &mut self.queue, &mut self.app, timeout) {
            warn!("layer-shell pump: {e}");
        }
        if self.app.exit {
            OverlayPumpOutcome::Exit(0)
        } else {
            OverlayPumpOutcome::Continue
        }
    }
}

fn dispatch_events(
    conn: &Connection,
    queue: &mut EventQueue<LayerShellApp>,
    app: &mut LayerShellApp,
    timeout: Option<Duration>,
) -> Result<(), OverlayError> {
    queue.flush().map_err(map_wayland_err)?;
    let _ = queue.dispatch_pending(app).map_err(map_dispatch_err)?;

    match timeout {
        Some(Duration::ZERO) => return Ok(()),
        None => {
            queue.blocking_dispatch(app).map_err(map_dispatch_err)?;
            return Ok(());
        }
        Some(duration) => {
            let deadline = Instant::now() + duration;
            while Instant::now() < deadline {
                if queue.dispatch_pending(app).map_err(map_dispatch_err)? > 0 {
                    return Ok(());
                }
                queue.flush().map_err(map_wayland_err)?;
                if queue.prepare_read().is_some() {
                    break;
                }
            }
            Ok(())
        }
    }
}

fn map_wayland_err<E: std::fmt::Display>(err: E) -> OverlayError {
    OverlayError::LayerShell(err.to_string())
}

fn map_dispatch_err(err: DispatchError) -> OverlayError {
    OverlayError::LayerShell(err.to_string())
}
