use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use tracing::{debug, warn};

use crate::error::CaptureError;
use crate::frame::CaptureFrame;
use crate::hint::WindowHint;
use crate::portal::{spawn_direct_pipewire_stream, spawn_portal_stream, PortalHandle};
use crate::pw_node::resolve_capture_node_id;
use crate::pw_record::{spawn_pw_record_stream, PwRecordHandle};

enum BackendHandle {
    Portal(PortalHandle),
    PwRecord(PwRecordHandle),
}

pub struct FrameStream {
    rx: Receiver<Result<CaptureFrame, CaptureError>>,
    backend: Option<BackendHandle>,
}

impl FrameStream {
    pub fn next_frame(&self) -> Result<CaptureFrame, CaptureError> {
        match self.rx.recv() {
            Ok(frame) => frame,
            Err(_) => Err(CaptureError::StreamClosed),
        }
    }

    pub fn next_frame_timeout(&self, timeout: Duration) -> Result<CaptureFrame, CaptureError> {
        match self.rx.recv_timeout(timeout) {
            Ok(frame) => frame,
            Err(mpsc::RecvTimeoutError::Timeout) => Err(CaptureError::Unavailable(
                "timed out waiting for frame — approve the ScreenCast dialog or see docs/CAPTURE.md"
                    .into(),
            )),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(CaptureError::StreamClosed),
        }
    }

    pub fn receiver(&self) -> &Receiver<Result<CaptureFrame, CaptureError>> {
        &self.rx
    }
}

impl Drop for FrameStream {
    fn drop(&mut self) {
        if let Some(backend) = self.backend.take() {
            match backend {
                BackendHandle::Portal(h) => h.stop(),
                BackendHandle::PwRecord(h) => h.stop(),
            }
        }
    }
}

pub fn start_capture_or_mock(window_hint: Option<WindowHint>) -> Result<FrameStream, CaptureError> {
    if crate::mock::mock_capture_enabled() {
        return Ok(start_mock_stream());
    }
    match start_capture(window_hint) {
        Ok(stream) => Ok(stream),
        Err(CaptureError::NotWayland(_)) => Ok(start_mock_stream()),
        Err(e) => Err(e),
    }
}

pub fn start_mock_stream() -> FrameStream {
    let (tx, rx) = mpsc::channel();
    crate::mock::spawn_mock_producer(tx);
    FrameStream { rx, backend: None }
}

pub fn start_capture(window_hint: Option<WindowHint>) -> Result<FrameStream, CaptureError> {
    ensure_wayland()?;

    let (tx, rx) = mpsc::channel();
    let mut direct_err: Option<CaptureError> = None;

    if let Some(node_id) = resolve_capture_node_id(window_hint.as_ref()) {
        match spawn_direct_pipewire_stream(node_id, window_hint.clone(), tx.clone()) {
            Ok(handle) => {
                debug!(
                    node_id,
                    "capture backend: PipeWire node (env override or Hypr/window hint match)"
                );
                return Ok(FrameStream {
                    rx,
                    backend: Some(BackendHandle::Portal(handle)),
                });
            }
            Err(e) => {
                direct_err = Some(e);
                warn!(
                    node_id,
                    "direct PipeWire node capture failed; falling back to portal ScreenCast"
                );
            }
        }
    }

    match spawn_portal_stream(window_hint.clone(), tx.clone()) {
        Ok(handle) => {
            debug!("capture backend: xdg-desktop-portal ScreenCast + PipeWire");
            return Ok(FrameStream {
                rx,
                backend: Some(BackendHandle::Portal(handle)),
            });
        }
        Err(portal_err) => {
            let chain = match &direct_err {
                Some(d) => format!("{d}; portal thread: {portal_err}"),
                None => portal_err.to_string(),
            };
            warn!(error = %chain, "portal capture unavailable, trying pw-record");
        }
    }

    let handle = spawn_pw_record_stream(window_hint.as_ref(), tx.clone())?;
    debug!("capture backend: pw-record CLI fallback");
    Ok(FrameStream {
        rx,
        backend: Some(BackendHandle::PwRecord(handle)),
    })
}

fn ensure_wayland() -> Result<(), CaptureError> {
    if crate::doctor::wayland_ok() {
        return Ok(());
    }
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".into());
    Err(CaptureError::NotWayland(session))
}
