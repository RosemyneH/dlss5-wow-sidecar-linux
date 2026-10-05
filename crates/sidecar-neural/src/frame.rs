use thiserror::Error;

pub const RGBA_BYTES_PER_PIXEL: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameLayout {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Error, PartialEq)]
pub enum ProcessError {
    #[error("invalid frame layout: {0}")]
    InvalidLayout(&'static str),
    #[error("buffer length {actual} does not match {expected} for {width}x{height} RGBA")]
    BufferSize {
        expected: usize,
        actual: usize,
        width: u32,
        height: u32,
    },
    #[error("sharpen amount must be in [0.0, 2.0], got {0}")]
    InvalidSharpenAmount(f32),
    #[error("ONNX inference is not enabled in this build; use Passthrough or SimpleSharpen")]
    OnnxUnavailable,
    #[error("ONNX model not found: {0}")]
    OnnxModelMissing(String),
}

impl FrameLayout {
    pub fn new(width: u32, height: u32) -> Result<Self, ProcessError> {
        if width == 0 || height == 0 {
            return Err(ProcessError::InvalidLayout("width and height must be non-zero"));
        }
        Ok(Self { width, height })
    }

    pub fn pixel_count(&self) -> usize {
        self.width as usize * self.height as usize
    }

    pub fn byte_len(&self) -> usize {
        self.pixel_count() * RGBA_BYTES_PER_PIXEL
    }

    pub fn pixel_offset(&self, x: u32, y: u32) -> usize {
        ((y * self.width + x) as usize) * RGBA_BYTES_PER_PIXEL
    }

    pub fn check_io(&self, input: &[u8], output: &[u8]) -> Result<(), ProcessError> {
        let expected = self.byte_len();
        if input.len() != expected {
            return Err(ProcessError::BufferSize {
                expected,
                actual: input.len(),
                width: self.width,
                height: self.height,
            });
        }
        if output.len() != expected {
            return Err(ProcessError::BufferSize {
                expected,
                actual: output.len(),
                width: self.width,
                height: self.height,
            });
        }
        Ok(())
    }
}

pub struct RgbaFrame<'a> {
    pub layout: FrameLayout,
    pub pixels: &'a [u8],
}

impl<'a> RgbaFrame<'a> {
    pub fn new(layout: FrameLayout, pixels: &'a [u8]) -> Result<Self, ProcessError> {
        layout.check_io(pixels, pixels)?;
        Ok(Self { layout, pixels })
    }
}
