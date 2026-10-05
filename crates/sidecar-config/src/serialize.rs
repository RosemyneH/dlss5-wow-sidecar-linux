use std::fmt::Write;

use crate::config::{Config, UiRect};

fn boolean(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

fn number(value: f32) -> String {
    format!("{value:.2}")
}

fn toml_string(value: &str) -> String {
    let mut out = String::from('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn serialize_config(config: &Config) -> String {
    let mut out = String::from(
        "# DLSS 5 sidecar. Written by the manager; hand edits are read back on\n\
# the next launch and overwritten on the next save.\n\n",
    );

    let _ = writeln!(out, "language = {}", toml_string(&config.language));
    let _ = writeln!(out, "theme = {}", toml_string(&config.theme));
    let _ = writeln!(out, "advanced_mode = {}", boolean(config.advanced_mode));
    let _ = writeln!(out, "neural_pass = {}", toml_string(&config.neural_pass));
    let _ = writeln!(out, "dlss_preset = {}", toml_string(&config.dlss_preset));
    let _ = writeln!(out, "show_hud = {}", boolean(config.show_hud));
    let _ = writeln!(out, "show_overlay = {}", boolean(config.show_overlay));
    let _ = writeln!(out, "flow_grid_size = {}", config.flow_grid_size);
    let _ = writeln!(out, "synthetic_depth = {}", number(config.synthetic_depth));
    let _ = writeln!(out, "neural_passes = {}", config.neural_passes);
    if config.ui_scale > 0.0 {
        let _ = writeln!(out, "ui_scale = {}", number(config.ui_scale));
    }
    let _ = writeln!(out, "ui_mask_feather = {}", config.ui_mask_feather);
    if !config.wow_dir.is_empty() {
        let _ = writeln!(out, "wow_dir = {}", toml_string(&config.wow_dir));
    }

    out.push_str(
        "\n# Global hotkeys. An empty string switches one off. Ctrl+Alt+Backspace\n\
# is the panic switch and is not configurable.\n",
    );
    out.push_str("[hotkeys]\n");
    let _ = writeln!(
        out,
        "toggle_overlay = {}",
        toml_string(&config.hotkeys.toggle_overlay)
    );
    let _ = writeln!(
        out,
        "toggle_hud = {}",
        toml_string(&config.hotkeys.toggle_hud)
    );
    let _ = writeln!(
        out,
        "start_stop = {}",
        toml_string(&config.hotkeys.start_stop)
    );

    out.push_str(
        "\n# Passed through to the RenoDX DLSS 5 add-on's [RenoDX.DLSS5]\n\
# section in ReShade.ini. A negative strength means \"leave the\n\
# add-on's own default alone\".\n",
    );
    out.push_str("[neural]\n");
    let n = &config.neural;
    let _ = writeln!(out, "enable_hooks = {}", n.enable_hooks);
    let _ = writeln!(out, "intensity = {}", number(n.intensity));
    let _ = writeln!(out, "color_strength = {}", number(n.color_strength));
    let _ = writeln!(out, "transfer_strength = {}", number(n.transfer_strength));
    let _ = writeln!(out, "paper_white_scale = {}", number(n.paper_white_scale));
    let _ = writeln!(out, "preset = {}", n.preset);
    let _ = writeln!(out, "style = {}", n.style);
    let _ = writeln!(out, "upscaling = {}", boolean(n.upscaling));
    let _ = writeln!(out, "local_structure = {}", number(n.local_structure));
    let _ = writeln!(out, "local_tone = {}", number(n.local_tone));
    let _ = writeln!(out, "skin_structure = {}", number(n.skin_structure));

    for rect in &config.ui_mask_rects {
        write_ui_rect(&mut out, rect);
    }

    out
}

fn write_ui_rect(out: &mut String, rect: &UiRect) {
    let _ = writeln!(
        out,
        "\n[[ui_mask]]\nleft = {}\ntop = {}\nright = {}\nbottom = {}",
        rect.left, rect.top, rect.right, rect.bottom
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_config;

    #[test]
    fn round_trip_wow_dir() {
        let mut cfg = Config::default();
        cfg.wow_dir = "/games/World of Warcraft/_retail_".into();
        let text = serialize_config(&cfg);
        let (again, warnings) = parse_config(&text);
        assert!(warnings.is_empty());
        assert_eq!(again.wow_dir, cfg.wow_dir);
    }
}
