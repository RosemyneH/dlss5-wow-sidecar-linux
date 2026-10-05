mod frame;
mod onnx;
mod passthrough;
mod sharpen;

pub use frame::{FrameLayout, ProcessError, RgbaFrame};
pub use onnx::{build_onnx_processor, OnnxProcessorConfig};
pub use passthrough::Passthrough;
pub use sharpen::SimpleSharpen;

use std::path::PathBuf;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum NeuralBackend {
    #[default]
    Passthrough,
    SimpleSharpen { amount: f32 },
    Onnx { model_path: PathBuf },
}

pub trait FrameProcessor: Send {
    fn id(&self) -> &'static str;
    fn process(
        &mut self,
        layout: &FrameLayout,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(), ProcessError>;
}

pub fn build_processor(backend: NeuralBackend) -> Result<Box<dyn FrameProcessor>, ProcessError> {
    match backend {
        NeuralBackend::Passthrough => Ok(Box::new(Passthrough)),
        NeuralBackend::SimpleSharpen { amount } => Ok(Box::new(SimpleSharpen::new(amount)?)),
        NeuralBackend::Onnx { model_path } => build_onnx_processor(&OnnxProcessorConfig {
            model_path,
            execution_provider: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_W: u32 = 64;
    const TEST_H: u32 = 64;

    fn test_layout() -> FrameLayout {
        FrameLayout::new(TEST_W, TEST_H).expect("64x64 layout")
    }

    fn checker_rgba(layout: &FrameLayout) -> Vec<u8> {
        let mut buf = vec![0u8; layout.byte_len()];
        for y in 0..layout.height {
            for x in 0..layout.width {
                let i = layout.pixel_offset(x, y);
                let v = if (x + y) % 2 == 0 { 40 } else { 200 };
                buf[i] = v;
                buf[i + 1] = v / 2;
                buf[i + 2] = 255 - v;
                buf[i + 3] = 255;
            }
        }
        buf
    }

    #[test]
    fn passthrough_64x64_identity() {
        let layout = test_layout();
        let input = checker_rgba(&layout);
        let mut output = vec![0u8; layout.byte_len()];
        let mut proc = Passthrough;
        proc.process(&layout, &input, &mut output).unwrap();
        assert_eq!(input, output);
    }

    #[test]
    fn sharpen_64x64_changes_edges() {
        let layout = test_layout();
        let input = checker_rgba(&layout);
        let mut passthrough_out = vec![0u8; layout.byte_len()];
        let mut sharpen_out = vec![0u8; layout.byte_len()];
        Passthrough
            .process(&layout, &input, &mut passthrough_out)
            .unwrap();
        let mut sharpen = SimpleSharpen::new(0.85).unwrap();
        sharpen.process(&layout, &input, &mut sharpen_out).unwrap();
        assert_ne!(passthrough_out, sharpen_out);
        assert_eq!(sharpen_out.len(), layout.byte_len());
    }

    #[test]
    fn onnx_backend_not_wired_yet() {
        let model = std::env::temp_dir().join("wowsidecar-neural-test.onnx");
        std::fs::write(&model, b"stub").unwrap();
        let result = build_processor(NeuralBackend::Onnx { model_path: model });
        assert!(matches!(result, Err(ProcessError::OnnxUnavailable)));
    }
}
