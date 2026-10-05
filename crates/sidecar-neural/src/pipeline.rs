use sidecar_config::Config;

use crate::config_bridge::{build_processor_from_config, neural_pass_count};
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
    let passes = neural_pass_count(config);
    let mut processor = build_processor_from_config(config)?;

    if passes == 1 {
        return processor.process(layout, input, output);
    }

    let nbytes = layout.byte_len();
    let mut scratch_a = vec![0u8; nbytes];
    let mut scratch_b = vec![0u8; nbytes];
    processor.process(layout, input, &mut scratch_a)?;
    let mut flip_a = true;
    for pass in 1..passes {
        if pass == passes - 1 {
            let src = if flip_a {
                &scratch_a[..]
            } else {
                &scratch_b[..]
            };
            processor.process(layout, src, output)?;
            return Ok(());
        }
        if flip_a {
            processor.process(layout, &scratch_a, &mut scratch_b)?;
            flip_a = false;
        } else {
            processor.process(layout, &scratch_b, &mut scratch_a)?;
            flip_a = true;
        }
    }
    Ok(())
}
