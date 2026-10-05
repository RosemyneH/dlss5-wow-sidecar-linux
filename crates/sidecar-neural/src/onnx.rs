use std::path::{Path, PathBuf};

use crate::frame::ProcessError;
use crate::FrameProcessor;

#[derive(Clone, Debug)]
pub struct OnnxProcessorConfig {
    pub model_path: PathBuf,
    pub execution_provider: Option<String>,
}

#[allow(dead_code)]
pub trait OnnxRuntime: Send {
    fn load(config: &OnnxProcessorConfig) -> Result<Self, ProcessError>
    where
        Self: Sized;
    fn infer_rgba(
        &mut self,
        layout: &crate::frame::FrameLayout,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(), ProcessError>;
}

pub fn build_onnx_processor(
    config: &OnnxProcessorConfig,
) -> Result<Box<dyn FrameProcessor>, ProcessError> {
    if !config.model_path.exists() {
        return Err(ProcessError::OnnxModelMissing(
            config.model_path.display().to_string(),
        ));
    }
    let _ = config.execution_provider.as_deref();
    Err(ProcessError::OnnxUnavailable)
}

#[allow(dead_code)]
pub fn register_onnx_model_path(path: &Path) -> OnnxProcessorConfig {
    OnnxProcessorConfig {
        model_path: path.to_path_buf(),
        execution_provider: None,
    }
}
