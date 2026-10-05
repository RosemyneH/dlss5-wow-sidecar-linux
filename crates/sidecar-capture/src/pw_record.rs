use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};
use tracing::{debug, warn};

use crate::error::CaptureError;
use crate::frame::CaptureFrame;
use crate::hint::WindowHint;

pub struct PwRecordHandle {
    child: Child,
    reader: JoinHandle<()>,
}

impl PwRecordHandle {
    pub fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = self.reader.join();
    }
}

pub fn spawn_pw_record_stream(
    hint: Option<&WindowHint>,
    frame_tx: Sender<Result<CaptureFrame, CaptureError>>,
) -> Result<PwRecordHandle, CaptureError> {
    let width = hint.map(|h| h.width).filter(|w| *w > 0).unwrap_or(1920);
    let height = hint.map(|h| h.height).filter(|h| *h > 0).unwrap_or(1080);
    let frame_bytes = (width as usize) * (height as usize) * 4;

    let mut cmd = Command::new("pw-record");
    cmd.args([
        "--media-type=Video",
        "--media-category=Capture",
        "--format",
        "rgba",
        "-a",
        "-",
    ]);
    if let Some(h) = hint {
        if !h.address.is_empty() {
            debug!(
                compositor = %h.compositor,
                address = %h.address,
                "pw-record fallback: portal unavailable; target left auto (pick matching window in portal when possible)"
            );
        }
    }

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| CaptureError::PwRecord(format!("failed to spawn pw-record: {e}")))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| CaptureError::PwRecord("pw-record stdout pipe missing".into()))?;

    let reader = thread::spawn(move || {
        let mut reader = stdout;
        let mut buf = vec![0u8; frame_bytes];
        while reader.read_exact(&mut buf).is_ok() {
            let frame = CaptureFrame::new(buf.clone(), width, height);
            if frame_tx.send(Ok(frame)).is_err() {
                break;
            }
            buf = vec![0u8; frame_bytes];
        }
        if frame_tx.send(Err(CaptureError::StreamClosed)).is_ok() {
            warn!("pw-record stream ended");
        }
    });

    Ok(PwRecordHandle { child, reader })
}
