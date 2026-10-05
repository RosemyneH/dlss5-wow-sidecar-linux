use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use tracing::debug;

use crate::error::CaptureError;
use crate::frame::CaptureFrame;
use crate::hint::{parse_capture_node_from_env, WindowHint};
use crate::portal::{spawn_direct_pipewire_stream, spawn_portal_stream, PortalHandle};
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
                "timed out waiting for frame".into(),
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

pub fn start_capture(window_hint: Option<WindowHint>) -> Result<FrameStream, CaptureError> {
    ensure_wayland()?;

    let (tx, rx) = mpsc::channel();

    if let Some(node_id) = parse_capture_node_from_env() {
        match spawn_direct_pipewire_stream(node_id, window_hint.clone(), tx.clone()) {
            Ok(handle) => {
                debug!(
                    node_id,
                    "capture backend: PipeWire node from WOWSIDECAR_CAPTURE_NODE"
                );
                return Ok(FrameStream {
                    rx,
                    backend: Some(BackendHandle::Portal(handle)),
                });
            }
            Err(e) => {
                debug!(
                    error = %e,
                    "WOWSIDECAR_CAPTURE_NODE path failed, trying portal ScreenCast"
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
            debug!(error = %portal_err, "portal capture unavailable, trying pw-record");
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
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    if session.eq_ignore_ascii_case("wayland") {
        return Ok(());
    }
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        return Ok(());
    }
    Err(CaptureError::NotWayland(session))
}
