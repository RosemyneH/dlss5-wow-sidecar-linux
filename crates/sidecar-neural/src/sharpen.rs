use crate::frame::{FrameLayout, ProcessError};
use crate::FrameProcessor;

#[derive(Clone, Copy, Debug)]
pub struct SimpleSharpen {
    amount: f32,
}

impl SimpleSharpen {
    pub fn new(amount: f32) -> Result<Self, ProcessError> {
        if !(0.0..=2.0).contains(&amount) {
            return Err(ProcessError::InvalidSharpenAmount(amount));
        }
        Ok(Self { amount })
    }
}

impl FrameProcessor for SimpleSharpen {
    fn id(&self) -> &'static str {
        "simple_sharpen"
    }

    fn process(
        &mut self,
        layout: &FrameLayout,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(), ProcessError> {
        layout.check_io(input, output)?;
        let w = layout.width;
        let h = layout.height;

        for y in 0..h {
            for x in 0..w {
                let o = layout.pixel_offset(x, y);
                for c in 0..3 {
                    let blur = box3(input, layout, x, y, c);
                    let center = input[o + c] as f32;
                    let sharp = center + self.amount * (center - blur);
                    output[o + c] = sharp.clamp(0.0, 255.0) as u8;
                }
                output[o + 3] = input[o + 3];
            }
        }
        Ok(())
    }
}

fn box3(input: &[u8], layout: &FrameLayout, x: u32, y: u32, channel: usize) -> f32 {
    let w = layout.width as i32;
    let h = layout.height as i32;
    let cx = x as i32;
    let cy = y as i32;
    let mut sum = 0u32;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let nx = (cx + dx).clamp(0, w - 1) as u32;
            let ny = (cy + dy).clamp(0, h - 1) as u32;
            let i = layout.pixel_offset(nx, ny) + channel;
            sum += input[i] as u32;
        }
    }
    sum as f32 / 9.0
}
