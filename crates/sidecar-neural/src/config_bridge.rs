use std::path::PathBuf;

use sidecar_config::Config;

use crate::{build_processor, FrameProcessor, NeuralBackend, ProcessError};

pub fn neural_pass_count(config: &Config) -> u32 {
    config.neural_passes.clamp(1, 3)
}

pub fn neural_backend_from_config(config: &Config) -> NeuralBackend {
    match config.neural_pass.as_str() {
        "passthrough" => NeuralBackend::Passthrough,
        "onnx" => NeuralBackend::Onnx {
            model_path: onnx_model_path(config),
        },
        _ => NeuralBackend::SimpleSharpen {
            amount: sharpen_amount_from_config(config),
        },
    }
}

pub fn build_processor_from_config(
    config: &Config,
) -> Result<Box<dyn FrameProcessor>, ProcessError> {
    build_processor(neural_backend_from_config(config))
}

pub fn processor_id_for_config(config: &Config) -> &'static str {
    match neural_backend_from_config(config) {
        NeuralBackend::Passthrough => "passthrough",
        NeuralBackend::SimpleSharpen { .. } => "simple_sharpen",
        NeuralBackend::Onnx { .. } => "onnx",
    }
}

fn sharpen_amount_from_config(config: &Config) -> f32 {
    let intensity = config.neural.intensity.clamp(0.0, 2.0);
    match config.dlss_preset.as_str() {
        "cnn-e" => (intensity * 0.92).clamp(0.0, 2.0),
        "cnn-f" => intensity,
        _ => intensity,
    }
}

fn onnx_model_path(config: &Config) -> PathBuf {
    if let Ok(path) = std::env::var("WOWSIDECAR_NEURAL_ONNX") {
        return PathBuf::from(path);
    }
    if !config.wow_dir.is_empty() {
        return PathBuf::from(&config.wow_dir).join("neural.onnx");
    }
    PathBuf::from("neural.onnx")
}
