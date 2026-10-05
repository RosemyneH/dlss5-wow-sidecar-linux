use std::time::{Duration, Instant};

use sidecar_probes::ProbeState;
use sidecar_runtime::SidecarStatus;

pub const STALL_AFTER: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Passthrough,
    Sharpen,
    Onnx,
    Idle,
    Unknown,
}

impl BackendKind {
    pub fn from_id(id: &str) -> Self {
        match id {
            "passthrough" => Self::Passthrough,
            "simple_sharpen" => Self::Sharpen,
            "onnx" => Self::Onnx,
            "idle" | "" => Self::Idle,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Passthrough => "passthrough (unprocessed)",
            Self::Sharpen => "sharpen",
            Self::Onnx => "onnx",
            Self::Idle => "idle (pipeline not started)",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CaptureState {
    Stopped,
    StatusUnavailable,
    Idle,
    Failed(String),
    WaitingFirstFrame,
    Stalled { frames: u64 },
    Partial { frames: u64, drops: u64 },
    Streaming { frames: u64 },
}

impl CaptureState {
    pub fn from_live(daemon_up: bool, status: Option<&SidecarStatus>, stalled: bool) -> Self {
        if !daemon_up {
            return Self::Stopped;
        }
        let Some(s) = status else {
            return Self::StatusUnavailable;
        };
        if !s.last_error.is_empty() {
            return Self::Failed(s.last_error.clone());
        }
        if BackendKind::from_id(&s.pass_name) == BackendKind::Idle {
            return Self::Idle;
        }
        if s.frames == 0 {
            return Self::WaitingFirstFrame;
        }
        if stalled {
            return Self::Stalled { frames: s.frames };
        }
        if s.drops > 0 {
            return Self::Partial {
                frames: s.frames,
                drops: s.drops,
            };
        }
        Self::Streaming { frames: s.frames }
    }

    pub fn level(&self) -> ProbeState {
        match self {
            Self::Streaming { .. } => ProbeState::Ok,
            Self::Failed(_) | Self::Stalled { .. } => ProbeState::Fail,
            _ => ProbeState::Warn,
        }
    }

    pub fn summary(&self) -> String {
        match self {
            Self::Stopped => "overlay stopped".into(),
            Self::StatusUnavailable => "daemon up, status not answering".into(),
            Self::Idle => "daemon idle — pipeline not started".into(),
            Self::Failed(err) => format!("pipeline failed: {err}"),
            Self::WaitingFirstFrame => {
                "waiting for first frame — approve the ScreenCast portal dialog".into()
            }
            Self::Stalled { frames } => format!(
                "stalled after {frames} frame(s) — no new frames for {}s",
                STALL_AFTER.as_secs()
            ),
            Self::Partial { frames, drops } => {
                let total = frames + drops;
                let pct = *drops as f64 * 100.0 / total as f64;
                format!("partial — {drops}/{total} frame(s) dropped ({pct:.1}%)")
            }
            Self::Streaming { frames } => format!("streaming — {frames} frame(s)"),
        }
    }
}

pub struct StallTracker {
    last_sequence: Option<u32>,
    changed_at: Instant,
}

impl StallTracker {
    pub fn new(now: Instant) -> Self {
        Self {
            last_sequence: None,
            changed_at: now,
        }
    }

    pub fn observe(&mut self, sequence: Option<u32>, now: Instant) -> bool {
        if sequence != self.last_sequence {
            self.last_sequence = sequence;
            self.changed_at = now;
        }
        sequence.is_some() && now.duration_since(self.changed_at) >= STALL_AFTER
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(pass: &str, frames: u64, drops: u64) -> SidecarStatus {
        SidecarStatus {
            pass_name: pass.into(),
            frames,
            drops,
            ..SidecarStatus::default()
        }
    }

    #[test]
    fn backend_ids_map_to_kinds() {
        assert_eq!(BackendKind::from_id("simple_sharpen"), BackendKind::Sharpen);
        assert_eq!(
            BackendKind::from_id("passthrough"),
            BackendKind::Passthrough
        );
        assert_eq!(BackendKind::from_id("onnx"), BackendKind::Onnx);
        assert_eq!(BackendKind::from_id("idle"), BackendKind::Idle);
        assert_eq!(BackendKind::from_id("bogus"), BackendKind::Unknown);
    }

    #[test]
    fn capture_state_classification() {
        assert_eq!(
            CaptureState::from_live(false, None, false),
            CaptureState::Stopped
        );
        assert_eq!(
            CaptureState::from_live(true, None, false),
            CaptureState::StatusUnavailable
        );
        assert_eq!(
            CaptureState::from_live(true, Some(&status("idle", 0, 0)), false),
            CaptureState::Idle
        );
        assert_eq!(
            CaptureState::from_live(true, Some(&status("simple_sharpen", 0, 0)), false),
            CaptureState::WaitingFirstFrame
        );
        assert_eq!(
            CaptureState::from_live(true, Some(&status("passthrough", 10, 2)), false),
            CaptureState::Partial {
                frames: 10,
                drops: 2
            }
        );
        assert_eq!(
            CaptureState::from_live(true, Some(&status("passthrough", 10, 0)), true),
            CaptureState::Stalled { frames: 10 }
        );
        assert_eq!(
            CaptureState::from_live(true, Some(&status("simple_sharpen", 10, 0)), false),
            CaptureState::Streaming { frames: 10 }
        );
        let mut failed = status("simple_sharpen", 10, 0);
        failed.last_error = "boom".into();
        assert_eq!(
            CaptureState::from_live(true, Some(&failed), false),
            CaptureState::Failed("boom".into())
        );
    }

    #[test]
    fn partial_summary_reports_drop_ratio() {
        let s = CaptureState::Partial {
            frames: 3,
            drops: 1,
        };
        assert!(s.summary().contains("1/4"));
        assert!(s.summary().contains("25.0%"));
    }

    #[test]
    fn stall_tracker_flags_frozen_sequence() {
        let t0 = Instant::now();
        let mut tracker = StallTracker::new(t0);
        assert!(!tracker.observe(Some(1), t0));
        assert!(!tracker.observe(Some(1), t0 + Duration::from_secs(1)));
        assert!(tracker.observe(Some(1), t0 + STALL_AFTER));
        assert!(!tracker.observe(Some(2), t0 + STALL_AFTER));
        assert!(!tracker.observe(None, t0 + STALL_AFTER * 3));
    }
}
