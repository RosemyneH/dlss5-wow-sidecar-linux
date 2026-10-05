use crate::frame::CaptureFrame;

pub const SYNTHETIC_W: u32 = 4;
pub const SYNTHETIC_H: u32 = 4;

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
