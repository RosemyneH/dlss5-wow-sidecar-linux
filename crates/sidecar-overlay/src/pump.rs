use winit::platform::pump_events::PumpStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayPumpOutcome {
    Continue,
    Exit(i32),
}

impl OverlayPumpOutcome {
    pub fn from_pump_status(status: PumpStatus) -> Self {
        match status {
            PumpStatus::Continue => Self::Continue,
            PumpStatus::Exit(code) => Self::Exit(code),
        }
    }

    pub fn is_continue(self) -> bool {
        matches!(self, Self::Continue)
    }

    pub fn exit_code(self) -> Option<i32> {
        match self {
            Self::Continue => None,
            Self::Exit(code) => Some(code),
        }
    }
}
