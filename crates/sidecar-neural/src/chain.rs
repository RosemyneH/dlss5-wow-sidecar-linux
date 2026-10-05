use sidecar_config::Config;

use crate::config_bridge::{neural_backend_from_config, neural_pass_count};
use crate::frame::{FrameLayout, ProcessError};
use crate::{build_processor, FrameProcessor, NeuralBackend};

#[derive(Clone, Debug, PartialEq)]
pub struct NeuralChainSpec {
    pub backend: NeuralBackend,
    pub passes: u32,
}

impl NeuralChainSpec {
    pub fn from_config(config: &Config) -> Self {
        Self {
            backend: neural_backend_from_config(config),
            passes: neural_pass_count(config),
        }
    }
}

pub struct NeuralChain {
    spec: NeuralChainSpec,
    processor: Box<dyn FrameProcessor>,
    scratch: Vec<u8>,
}

impl NeuralChain {
    pub fn from_config(config: &Config) -> Result<Self, ProcessError> {
        let spec = NeuralChainSpec::from_config(config);
        let processor = build_processor(spec.backend.clone())?;
        Ok(Self {
            spec,
            processor,
            scratch: Vec::new(),
        })
    }

    pub fn spec(&self) -> &NeuralChainSpec {
        &self.spec
    }

    pub fn processor_id(&self) -> &'static str {
        self.processor.id()
    }

    // ʕ •ᴥ•ʔ✿ Returns whether the spec changed; on error the previous chain stays active ✿ ʕ •ᴥ•ʔ
    pub fn reload(&mut self, config: &Config) -> Result<bool, ProcessError> {
        let spec = NeuralChainSpec::from_config(config);
        if spec == self.spec {
            return Ok(false);
        }
        if spec.backend != self.spec.backend {
            self.processor = build_processor(spec.backend.clone())?;
        }
        self.spec = spec;
        Ok(true)
    }

    pub fn process(
        &mut self,
        layout: &FrameLayout,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(), ProcessError> {
        layout.check_io(input, output)?;
        self.processor.process(layout, input, output)?;
        if self.spec.passes > 1 {
            self.scratch.resize(layout.byte_len(), 0);
            for _ in 1..self.spec.passes {
                self.scratch.copy_from_slice(output);
                self.processor.process(layout, &self.scratch, output)?;
            }
        }
        Ok(())
    }
}
