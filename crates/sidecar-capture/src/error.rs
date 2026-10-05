use thiserror::Error;

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("capture requires a Wayland session (XDG_SESSION_TYPE={0})")]
    NotWayland(String),
    #[error("portal screen cast failed: {0}")]
    Portal(String),
    #[error("pipewire stream failed: {0}")]
    PipeWire(String),
    #[error("pw-record fallback failed: {0}")]
    PwRecord(String),
    #[error("capture backend unavailable: {0}")]
    Unavailable(String),
    #[error("frame stream closed")]
    StreamClosed,
}
