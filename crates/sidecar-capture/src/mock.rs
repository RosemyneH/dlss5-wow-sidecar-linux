use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use crate::frame::CaptureFrame;

pub const SYNTHETIC_W: u32 = 4;
pub const SYNTHETIC_H: u32 = 4;

pub const MOCK_STREAM_W: u32 = 64;
pub const MOCK_STREAM_H: u32 = 64;

static MOCK_CAPTURE: AtomicBool = AtomicBool::new(false);

pub fn enable_mock_capture() {
    MOCK_CAPTURE.store(true, Ordering::Relaxed);
}

pub fn mock_capture_enabled() -> bool {
    MOCK_CAPTURE.load(Ordering::Relaxed)
        || std::env::var("WOWSIDECAR_MOCK_CAPTURE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
}

fn mock_rgba(frame_idx: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((MOCK_STREAM_W * MOCK_STREAM_H * 4) as usize);
    for y in 0..MOCK_STREAM_H {
        for x in 0..MOCK_STREAM_W {
            rgba.push(((x + frame_idx) % 256) as u8);
            rgba.push(((y + frame_idx * 2) % 256) as u8);
            rgba.push(128);
            rgba.push(255);
        }
    }
    rgba
}

pub fn mock_frame(frame_idx: u32) -> CaptureFrame {
    CaptureFrame::new(mock_rgba(frame_idx), MOCK_STREAM_W, MOCK_STREAM_H)
}

/// Deterministic RGBA pattern for pipeline tests (no portal / PipeWire).
pub fn synthetic_rgba_4x4() -> Vec<u8> {
    let mut rgba = Vec::with_capacity((SYNTHETIC_W * SYNTHETIC_H * 4) as usize);
    for y in 0..SYNTHETIC_H {
        for x in 0..SYNTHETIC_W {
            rgba.push((x * 60 + 10) as u8);
            rgba.push((y * 40 + 20) as u8);
            rgba.push(128);
            rgba.push(255);
        }
    }
    rgba
}

pub fn synthetic_frame_4x4() -> CaptureFrame {
    CaptureFrame::new(synthetic_rgba_4x4(), SYNTHETIC_W, SYNTHETIC_H)
}

pub(crate) fn spawn_mock_producer(
    tx: std::sync::mpsc::Sender<Result<CaptureFrame, crate::error::CaptureError>>,
) {
    thread::spawn(move || {
        let mut frame_idx = 0u32;
        while tx.send(Ok(mock_frame(frame_idx))).is_ok() {
            frame_idx = frame_idx.wrapping_add(1);
            thread::sleep(Duration::from_millis(16));
        }
    });
}
