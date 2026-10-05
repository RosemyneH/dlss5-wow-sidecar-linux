use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum SidecarCommand {
    None = 0,
    Stop = 1,
    ShowOverlay = 2,
    HideOverlay = 3,
    ShowHud = 4,
    HideHud = 5,
    Panic = 6,
}

impl SidecarCommand {
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::None),
            1 => Some(Self::Stop),
            2 => Some(Self::ShowOverlay),
            3 => Some(Self::HideOverlay),
            4 => Some(Self::ShowHud),
            5 => Some(Self::HideHud),
            6 => Some(Self::Panic),
            _ => None,
        }
    }

    pub fn from_cli_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "stop" => Some(Self::Stop),
            "panic" => Some(Self::Panic),
            "show-overlay" | "overlay-on" => Some(Self::ShowOverlay),
            "hide-overlay" | "overlay-off" => Some(Self::HideOverlay),
            "show-hud" | "hud-on" => Some(Self::ShowHud),
            "hide-hud" | "hud-off" => Some(Self::HideHud),
            "toggle-overlay" => Some(Self::HideOverlay),
            "toggle-hud" => Some(Self::HideHud),
            _ => None,
        }
    }

    pub fn is_toggle(self) -> bool {
        matches!(self, Self::HideOverlay | Self::HideHud)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SidecarStatus {
    pub sequence: u32,
    pub process_id: u32,
    pub overlay_visible: u32,
    pub hud_visible: u32,
    pub width: u32,
    pub height: u32,
    pub p50_ms: f64,
    pub p99_ms: f64,
    pub frames: u64,
    pub drops: u64,
    pub fps: f64,
    pub capture_fps: f64,
    pub idle_ms: f64,
    pub record_ms: f64,
    pub present_wait_ms: f64,
    pub gpu_wait_ms: f64,
    pub vram_used_mb: u32,
    pub vram_budget_mb: u32,
    pub vram_spilled_mb: u32,
    pub pass_name: String,
    pub runtime_variant: String,
    pub last_error: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ControlRequest {
    Ping,
    Command { command: SidecarCommand },
    GetStatus,
    Start,
    Stop,
    Status,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ControlResponse {
    Pong,
    Ack { ok: bool },
    Status { status: SidecarStatus },
    Error { message: String },
}
