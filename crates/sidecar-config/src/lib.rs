mod config;
mod hotkey;
mod io;
mod language;
mod parse;
mod presets;
mod serialize;
mod strength;

pub use config::{reset_rendering_settings, Config, Hotkeys, NeuralSettings, UiRect};
pub use hotkey::{format_hotkey, normalize_hotkey_string, parse_hotkey, Hotkey};
pub use io::{default_config_path, load_config, save_config, sidecar_dir};
pub use language::parse_language_tag;
pub use parse::parse_config;
pub use presets::{apply_preset, matching_preset, Preset, PRESETS};
pub use serialize::serialize_config;
pub use strength::{neural_strength_of, set_neural_strength, NeuralStrength};

#[cfg(test)]
mod tests;
