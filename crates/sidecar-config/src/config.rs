use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeuralSettings {
    pub enable_hooks: i32,
    pub intensity: f32,
    pub color_strength: f32,
    pub transfer_strength: f32,
    pub paper_white_scale: f32,
    pub preset: i32,
    pub style: i32,
    pub upscaling: bool,
    pub local_structure: f32,
    pub local_tone: f32,
    pub skin_structure: f32,
}

impl Default for NeuralSettings {
    fn default() -> Self {
        Self {
            enable_hooks: 2,
            intensity: 1.0,
            color_strength: 1.0,
            transfer_strength: 1.0,
            paper_white_scale: 1.0,
            preset: 0,
            style: 0,
            upscaling: false,
            local_structure: -1.0,
            local_tone: -1.0,
            skin_structure: -1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hotkeys {
    pub toggle_overlay: String,
    pub toggle_hud: String,
    pub start_stop: String,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            toggle_overlay: "Ctrl+Alt+D".into(),
            toggle_hud: "Ctrl+Alt+H".into(),
            start_stop: "Ctrl+Alt+S".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub language: String,
    pub theme: String,
    pub advanced_mode: bool,
    pub ui_scale: f32,
    pub show_hud: bool,
    pub show_overlay: bool,
    pub flow_grid_size: u32,
    pub wow_dir: String,
    pub neural_pass: String,
    pub dlss_preset: String,
    pub synthetic_depth: f32,
    pub neural_passes: u32,
    pub ui_mask_rects: Vec<UiRect>,
    pub ui_mask_feather: u32,
    pub neural: NeuralSettings,
    pub hotkeys: Hotkeys,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: "en".into(),
            theme: "stormwind".into(),
            advanced_mode: false,
            ui_scale: 0.0,
            show_hud: true,
            show_overlay: true,
            flow_grid_size: 4,
            wow_dir: String::new(),
            neural_pass: "reshade".into(),
            dlss_preset: "cnn-f".into(),
            synthetic_depth: 0.5,
            neural_passes: 1,
            ui_mask_rects: Vec::new(),
            ui_mask_feather: 0,
            neural: NeuralSettings::default(),
            hotkeys: Hotkeys::default(),
        }
    }
}

pub fn reset_rendering_settings(config: &mut Config) {
    let defaults = Config::default();
    config.show_hud = defaults.show_hud;
    config.show_overlay = defaults.show_overlay;
    config.flow_grid_size = defaults.flow_grid_size;
    config.neural_pass = defaults.neural_pass;
    config.dlss_preset = defaults.dlss_preset;
    config.synthetic_depth = defaults.synthetic_depth;
    config.neural_passes = defaults.neural_passes;
    config.neural = defaults.neural;
}
