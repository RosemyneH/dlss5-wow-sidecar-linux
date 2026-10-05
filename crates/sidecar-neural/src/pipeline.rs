use sidecar_config::Config;

use crate::chain::NeuralChain;
use crate::frame::{FrameLayout, ProcessError};
use crate::FrameProcessor;

pub fn run_pipeline(
    width: u32,
    height: u32,
    input: &[u8],
    stages: &mut [Box<dyn FrameProcessor>],
) -> Result<Vec<u8>, ProcessError> {
    let layout = FrameLayout::new(width, height)?;
    layout.check_io(input, input)?;
    let mut cur = input.to_vec();
    let mut scratch = vec![0u8; layout.byte_len()];
    for stage in stages {
        stage.process(&layout, &cur, &mut scratch)?;
        cur.copy_from_slice(&scratch);
    }
    Ok(cur)
}

pub fn process_frame(
    config: &Config,
    layout: &FrameLayout,
    input: &[u8],
    output: &mut [u8],
) -> Result<(), ProcessError> {
    layout.check_io(input, output)?;
    NeuralChain::from_config(config)?.process(layout, input, output)
}
