use toml::Table;
use toml::Value;

use crate::hotkey::normalize_hotkey_string;
use crate::language::parse_language_tag;
use crate::config::{Config, NeuralSettings, UiRect};

const KNOWN_KEYS: &[&str] = &[
    "show_hud",
    "show_overlay",
    "flow_grid_size",
    "neural_pass",
    "dlss_preset",
    "synthetic_depth",
    "ui_mask",
    "ui_mask_feather",
    "neural",
    "wow_dir",
    "language",
    "ui_scale",
    "neural_passes",
    "hotkeys",
    "theme",
    "advanced_mode",
];

fn is_known(key: &str) -> bool {
    KNOWN_KEYS.contains(&key)
}

fn read_bool(root: &Table, key: &str, target: &mut bool, warnings: &mut Vec<String>) {
    let Some(node) = root.get(key) else {
        return;
    };
    if let Some(value) = node.as_bool() {
        *target = value;
    } else {
        warnings.push(format!("{key}: expected a boolean; keeping the default"));
    }
}

fn read_float(
    root: &Table,
    key: &str,
    target: &mut f32,
    low: f32,
    high: f32,
    warnings: &mut Vec<String>,
) {
    let Some(node) = root.get(key) else {
        return;
    };
    let Some(value) = node.as_float() else {
        warnings.push(format!("{key}: expected a number; keeping the default"));
        return;
    };
    let value = value as f32;
    let clamped = value.clamp(low, high);
    if clamped != value {
        warnings.push(format!("{key}: out of range; clamped"));
    }
    *target = clamped;
}

fn read_int(
    root: &Table,
    key: &str,
    target: &mut i32,
    low: i32,
    high: i32,
    warnings: &mut Vec<String>,
) {
    let Some(node) = root.get(key) else {
        return;
    };
    let Some(value) = node.as_integer() else {
        warnings.push(format!("{key}: expected an integer; keeping the default"));
        return;
    };
    let value = value as i32;
    let clamped = value.clamp(low, high);
    if clamped != value {
        warnings.push(format!("{key}: out of range; clamped"));
    }
    *target = clamped;
}

fn read_optional_strength(
    root: &Table,
    key: &str,
    target: &mut f32,
    warnings: &mut Vec<String>,
) {
    let Some(node) = root.get(key) else {
        return;
    };
    let Some(value) = node.as_float() else {
        warnings.push(format!("{key}: expected a number; keeping the default"));
        return;
    };
    let value = value as f32;
    *target = if value < 0.0 { -1.0 } else { value.clamp(0.0, 4.0) };
}

fn read_hotkey(
    table: &Table,
    key: &str,
    target: &mut String,
    warnings: &mut Vec<String>,
) {
    let Some(entry) = table.get(key) else {
        return;
    };
    let Some(value) = entry.as_str() else {
        warnings.push(format!(
            "hotkeys.{key}: expected a string; keeping {target}"
        ));
        return;
    };
    if value.is_empty() {
        target.clear();
    } else if let Some(formatted) = normalize_hotkey_string(value) {
        *target = formatted;
    } else {
        warnings.push(format!(
            "hotkeys.{key}: \"{value}\" is not a key combination; keeping {target}"
        ));
    }
}

pub fn parse_config(text: &str) -> (Config, Vec<String>) {
    let mut config = Config::default();
    let mut warnings = Vec::new();

    let root: Table = match text.parse() {
        Ok(t) => t,
        Err(error) => {
            warnings.push(format!("could not parse config: {error}"));
            return (config, warnings);
        }
    };

    read_bool(&root, "show_hud", &mut config.show_hud, &mut warnings);
    read_bool(&root, "advanced_mode", &mut config.advanced_mode, &mut warnings);
    read_bool(&root, "show_overlay", &mut config.show_overlay, &mut warnings);

    if let Some(node) = root.get("flow_grid_size") {
        if let Some(value) = node.as_integer() {
            if value == 1 || value == 2 || value == 4 {
                config.flow_grid_size = value as u32;
            } else {
                warnings.push("flow_grid_size: must be 1, 2 or 4; using 4".to_string());
            }
        } else {
            warnings.push("flow_grid_size: expected an integer; using 4".to_string());
        }
    }

    if let Some(node) = root.get("neural_pass") {
        if let Some(value) = node.as_str() {
            config.neural_pass = value.to_string();
        } else {
            warnings.push("neural_pass: expected a string; using \"passthrough\"".to_string());
        }
    }

    if let Some(node) = root.get("dlss_preset") {
        if let Some(value) = node.as_str() {
            config.dlss_preset = value.to_string();
        } else {
            warnings.push("dlss_preset: expected a string; using \"cnn-f\"".to_string());
        }
    }

    if let Some(node) = root.get("language") {
        if let Some(value) = node.as_str() {
            if parse_language_tag(value) {
                config.language = value.to_string();
            } else {
                warnings.push(format!(
                    "language: unknown tag \"{value}\"; falling back to English"
                ));
            }
        } else {
            warnings.push("language: expected a string; using English".to_string());
        }
    }

    if let Some(node) = root.get("theme") {
        if let Some(value) = node.as_str() {
            config.theme = value.to_string();
        } else {
            warnings.push("theme: expected a string; using \"stormwind\"".to_string());
        }
    }

    if let Some(node) = root.get("wow_dir") {
        if let Some(value) = node.as_str() {
            config.wow_dir = value.to_string();
        } else {
            warnings.push("wow_dir: expected a string; leaving it unset".to_string());
        }
    }

    if let Some(node) = root.get("ui_scale") {
        if let Some(value) = node.as_float() {
            let wanted = value as f32;
            config.ui_scale = if wanted <= 0.0 {
                0.0
            } else {
                wanted.clamp(0.75, 3.0)
            };
            if wanted > 0.0 && config.ui_scale != wanted {
                warnings.push("ui_scale: out of range; clamped".to_string());
            }
        } else {
            warnings.push("ui_scale: expected a number; deciding automatically".to_string());
        }
    }

    if let Some(Value::Table(table)) = root.get("hotkeys") {
        read_hotkey(table, "toggle_overlay", &mut config.hotkeys.toggle_overlay, &mut warnings);
        read_hotkey(table, "toggle_hud", &mut config.hotkeys.toggle_hud, &mut warnings);
        read_hotkey(table, "start_stop", &mut config.hotkeys.start_stop, &mut warnings);
    } else if root.contains_key("hotkeys") {
        warnings.push("hotkeys: expected a table; keeping the defaults".to_string());
    }

    read_float(
        &root,
        "synthetic_depth",
        &mut config.synthetic_depth,
        0.0,
        1.0,
        &mut warnings,
    );

    if root.contains_key("neural_passes") {
        let mut passes = config.neural_passes as i32;
        read_int(&root, "neural_passes", &mut passes, 1, 4, &mut warnings);
        config.neural_passes = passes as u32;
    }

    if root.contains_key("ui_mask_feather") {
        let mut feather = config.ui_mask_feather as i32;
        read_int(&root, "ui_mask_feather", &mut feather, 0, 256, &mut warnings);
        config.ui_mask_feather = feather as u32;
    }

    if let Some(Value::Array(array)) = root.get("ui_mask") {
        for element in array {
            let Some(entry) = element.as_table() else {
                warnings.push("ui_mask: expected a table of edges; entry ignored".to_string());
                continue;
            };
            let rect = UiRect {
                left: entry.get("left").and_then(|v| v.as_integer()).unwrap_or(0) as i32,
                top: entry.get("top").and_then(|v| v.as_integer()).unwrap_or(0) as i32,
                right: entry.get("right").and_then(|v| v.as_integer()).unwrap_or(0) as i32,
                bottom: entry.get("bottom").and_then(|v| v.as_integer()).unwrap_or(0) as i32,
            };
            config.ui_mask_rects.push(rect);
        }
    } else if root.contains_key("ui_mask") {
        warnings.push("ui_mask: expected an array of tables; ignored".to_string());
    }

    if let Some(Value::Table(table)) = root.get("neural") {
        let n = &mut config.neural;
        read_neural(table, n, &mut warnings);
    } else if root.contains_key("neural") {
        warnings.push("neural: expected a table; ignored".to_string());
    }

    for key in root.keys() {
        if !is_known(key) {
            warnings.push(format!("unknown key ignored: {key}"));
        }
    }

    (config, warnings)
}

fn read_neural(table: &Table, n: &mut NeuralSettings, warnings: &mut Vec<String>) {
    read_int(table, "enable_hooks", &mut n.enable_hooks, 0, 2, warnings);
    read_float(table, "intensity", &mut n.intensity, 0.0, 1.0, warnings);
    read_float(table, "color_strength", &mut n.color_strength, 0.0, 1.0, warnings);
    read_float(table, "transfer_strength", &mut n.transfer_strength, 0.0, 1.0, warnings);
    read_float(table, "paper_white_scale", &mut n.paper_white_scale, 0.0, 10.0, warnings);
    read_int(table, "preset", &mut n.preset, 0, 3, warnings);
    read_int(table, "style", &mut n.style, 0, 3, warnings);
    read_bool(table, "upscaling", &mut n.upscaling, warnings);
    read_optional_strength(table, "local_structure", &mut n.local_structure, warnings);
    read_optional_strength(table, "local_tone", &mut n.local_tone, warnings);
    read_optional_strength(table, "skin_structure", &mut n.skin_structure, warnings);
}
