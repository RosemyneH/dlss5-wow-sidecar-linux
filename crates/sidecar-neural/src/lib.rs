mod chain;
mod config_bridge;
mod frame;
mod onnx;
mod passthrough;
mod pipeline;
mod sharpen;

pub use chain::{NeuralChain, NeuralChainSpec};
pub use config_bridge::{
    build_processor_from_config, neural_backend_from_config, neural_pass_count,
    processor_id_for_config, MAX_NEURAL_PASSES,
};
pub use frame::{FrameLayout, ProcessError, RgbaFrame};
pub use onnx::{build_onnx_processor, OnnxProcessorConfig};
pub use passthrough::Passthrough;
pub use pipeline::{process_frame, run_pipeline};
pub use sharpen::SimpleSharpen;

use std::path::PathBuf;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum NeuralBackend {
    #[default]
    Passthrough,
    SimpleSharpen {
        amount: f32,
    },
    Onnx {
        model_path: PathBuf,
    },
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
mod tests;
