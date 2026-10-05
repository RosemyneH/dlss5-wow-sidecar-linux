use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use sidecar_capture::{mock_capture_enabled, start_capture, wow_window_hint, CaptureFrame, FrameStream};
use sidecar_config::{default_config_path, load_config, Config};
use sidecar_core::DesktopWindow;
use sidecar_neural::{
    build_processor_from_config, process_frame, processor_id_for_config, FrameLayout,
};
use sidecar_overlay::OverlayPresenter;
use tracing::{info, warn};

use crate::fps::FpsCounter;
use crate::protocol::SidecarStatus;

pub struct Pipeline {
    overlay_visible: Arc<Mutex<bool>>,
    status: Arc<Mutex<SidecarStatus>>,
    stop_requested: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
    force_mock_capture: bool,
}

impl Pipeline {
    pub fn new(
        overlay_visible: Arc<Mutex<bool>>,
        status: Arc<Mutex<SidecarStatus>>,
        stop_requested: Arc<AtomicBool>,
    ) -> Self {
        Self {
            overlay_visible,
            status,
            stop_requested,
            running: Arc::new(AtomicBool::new(false)),
            thread: Mutex::new(None),
            force_mock_capture: false,
        }
    }

    pub fn with_mock_capture(mut self) -> Self {
        self.force_mock_capture = true;
        self
    }

    pub fn running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    pub fn start(&mut self) -> anyhow::Result<()> {
        if self.running.load(Ordering::Acquire) {
            return Ok(());
        }
        let mut guard = self.thread.lock().unwrap();
        if guard.is_some() {
            return Ok(());
        }

        let (config, warnings) = load_config(&default_config_path());
        for w in warnings {
            warn!("config: {w}");
        }

        self.stop_requested.store(false, Ordering::Release);
        self.running.store(true, Ordering::Release);
        {
            let mut st = self.status.lock().unwrap();
            st.pass_name = processor_id_for_config(&config).to_string();
            st.runtime_variant = "linux-pipeline".into();
            st.last_error.clear();
        }

        let overlay_visible = self.overlay_visible.clone();
        let status = self.status.clone();
        let running = self.running.clone();
        let stop_requested = self.stop_requested.clone();
        let force_mock = self.force_mock_capture;

        *guard = Some(thread::spawn(move || {
            if let Err(e) = run_pipeline_loop(
                config,
                overlay_visible,
                status.clone(),
                stop_requested,
                force_mock,
            ) {
                warn!("pipeline exited: {e:#}");
                status.lock().unwrap().last_error = e.to_string();
            }
            running.store(false, Ordering::Release);
        }));

        Ok(())
    }

    pub fn stop(&mut self) {
        self.stop_requested.store(true, Ordering::Release);
        let mut guard = self.thread.lock().unwrap();
        if let Some(handle) = guard.take() {
            let _ = handle.join();
        }
        self.running.store(false, Ordering::Release);
    }
}

fn mock_desktop_window() -> DesktopWindow {
    DesktopWindow {
        compositor: "mock".into(),
        address: "mock:0".into(),
        title: "mock-wow".into(),
        class: "mock".into(),
        x: 0,
        y: 0,
        width: 64,
        height: 64,
        fullscreen: false,
    }
}

fn desktop_for_pipeline(hint: Option<sidecar_capture::WindowHint>, mock: bool) -> DesktopWindow {
    if mock {
        return mock_desktop_window();
    }
    if let Some(h) = hint {
        return DesktopWindow {
            compositor: h.compositor,
            address: h.address,
            title: h.title,
            class: h.class,
            x: h.x,
            y: h.y,
            width: h.width,
            height: h.height,
            fullscreen: false,
        };
    }
    mock_desktop_window()
}

enum CaptureBackend {
    Live(FrameStream),
    Mock(mpsc::Receiver<CaptureFrame>),
}

impl CaptureBackend {
    fn next_frame(&self, timeout: Duration) -> Result<CaptureFrame, sidecar_capture::CaptureError> {
        match self {
            CaptureBackend::Live(stream) => stream.next_frame_timeout(timeout),
            CaptureBackend::Mock(rx) => match rx.recv_timeout(timeout) {
                Ok(frame) => Ok(frame),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    Err(sidecar_capture::CaptureError::Unavailable(
                        "timed out waiting for frame".into(),
                    ))
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    Err(sidecar_capture::CaptureError::StreamClosed)
                }
            },
        }
    }
}

fn open_capture(mock: bool) -> anyhow::Result<(CaptureBackend, DesktopWindow)> {
    if mock {
        let desktop = mock_desktop_window();
        let (tx, rx) = mpsc::channel();
        let w = desktop.width;
        let h = desktop.height;
        thread::spawn(move || {
            let len = (w * h * 4) as usize;
            let mut rgba = vec![0u8; len];
            for i in (0..len).step_by(4) {
                rgba[i] = 80;
                rgba[i + 1] = 120;
                rgba[i + 2] = 200;
                rgba[i + 3] = 255;
            }
            for _ in 0..120 {
                if tx.send(CaptureFrame::new(rgba.clone(), w, h)).is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(16));
            }
        });
        return Ok((CaptureBackend::Mock(rx), desktop));
    }

    let hint = wow_window_hint();
    let desktop = desktop_for_pipeline(hint.clone(), false);
    let stream = start_capture(hint)?;
    Ok((CaptureBackend::Live(stream), desktop))
}

fn capture_environment_available() -> bool {
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    if session.eq_ignore_ascii_case("wayland") {
        return true;
    }
    std::env::var("WAYLAND_DISPLAY").is_ok()
}

fn run_pipeline_loop(
    config: Config,
    overlay_visible: Arc<Mutex<bool>>,
    status: Arc<Mutex<SidecarStatus>>,
    stop_requested: Arc<AtomicBool>,
    force_mock: bool,
) -> anyhow::Result<()> {
    let use_mock = force_mock || mock_capture_enabled() || !capture_environment_available();
    let (capture, desktop) = open_capture(use_mock).map_err(|e| anyhow::anyhow!("{e}"))?;
    info!(
        "pipeline capture: {} ({}x{})",
        if use_mock { "mock" } else { "live" },
        desktop.width,
        desktop.height
    );

    let initial_visible = *overlay_visible.lock().unwrap();
    let mut presenter = if use_mock {
        None
    } else {
        let mut p = OverlayPresenter::for_desktop_window(&desktop).map_err(|e| anyhow::anyhow!("{e}"))?;
        p.set_visible(initial_visible && config.show_overlay);
        Some(p)
    };

    build_processor_from_config(&config)?;
    let pass_name = processor_id_for_config(&config).to_string();

    let mut capture_fps = FpsCounter::new(Duration::from_secs(1));
    let mut overlay_fps = FpsCounter::new(Duration::from_secs(1));
    let mut last_overlay_visible = initial_visible;
    let mut frames: u64 = 0;
    let mut drops: u64 = 0;

    while !stop_requested.load(Ordering::Acquire) {
        if let Some(p) = presenter.as_mut() {
            if p.pump(Some(Duration::from_millis(0))).exit_code().is_some() {
                break;
            }
        }

        let visible = *overlay_visible.lock().unwrap();
        if visible != last_overlay_visible {
            if let Some(p) = presenter.as_mut() {
                p.set_visible(visible && config.show_overlay);
            }
            last_overlay_visible = visible;
        }

        match capture.next_frame(Duration::from_millis(32)) {
            Ok(frame) if frame.validate() => {
                capture_fps.tick_frame();

                let layout = FrameLayout::new(frame.width, frame.height)?;
                let mut work = vec![0u8; layout.byte_len()];
                process_frame(&config, &layout, &frame.rgba, &mut work)?;

                frames += 1;
                let mut presented = false;
                if let Some(p) = presenter.as_mut() {
                    if visible && config.show_overlay {
                        if p.show_frame(&work, 0, 0, frame.width, frame.height).is_ok() {
                            overlay_fps.tick_frame();
                            presented = true;
                            if p.pump(Some(Duration::from_millis(0))).exit_code().is_some() {
                                break;
                            }
                        } else {
                            drops += 1;
                        }
                    }
                } else if visible && config.show_overlay {
                    overlay_fps.tick_frame();
                    presented = true;
                }

                let mut st = status.lock().unwrap();
                st.capture_fps = capture_fps.fps();
                if presented {
                    st.fps = overlay_fps.fps();
                }
                st.frames = frames;
                st.drops = drops;
                st.width = frame.width;
                st.height = frame.height;
                st.pass_name = pass_name.clone();
                st.overlay_visible = u32::from(visible);
                st.sequence = st.sequence.wrapping_add(1);
            }
            Ok(_) => {
                drops += 1;
                warn!("dropped invalid capture frame");
            }
            Err(sidecar_capture::CaptureError::Unavailable(_)) => continue,
            Err(sidecar_capture::CaptureError::StreamClosed) => break,
            Err(e) => {
                warn!("capture error: {e}");
                break;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::SidecarStatus;

    #[test]
    fn mock_pipeline_runs_headless() {
        let overlay_visible = Arc::new(Mutex::new(true));
        let status = Arc::new(Mutex::new(SidecarStatus::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let mut pipeline =
            Pipeline::new(overlay_visible.clone(), status.clone(), stop).with_mock_capture();
        pipeline.start().expect("start");
        for _ in 0..80 {
            if status.lock().unwrap().frames >= 3 {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        for _ in 0..40 {
            if status.lock().unwrap().capture_fps > 0.0 {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        pipeline.stop();
        assert!(status.lock().unwrap().frames >= 3);
        assert!(status.lock().unwrap().capture_fps > 0.0);
    }
}
