use crate::frame::{FrameLayout, ProcessError};
use crate::FrameProcessor;

#[derive(Clone, Copy, Debug, Default)]
pub struct Passthrough;

impl FrameProcessor for Passthrough {
    fn id(&self) -> &'static str {
        "passthrough"
    }

    fn process(
        &mut self,
        layout: &FrameLayout,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(), ProcessError> {
        layout.check_io(input, output)?;
        output.copy_from_slice(input);
        Ok(())
    }
}
