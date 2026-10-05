#![allow(clippy::field_reassign_with_default)]

use tempfile::NamedTempFile;

use crate::{
    apply_preset, load_config, matching_preset, parse_config, reset_rendering_settings,
    save_config, serialize_config, set_neural_strength, Config, NeuralStrength, PRESETS,
};

#[test]
fn defaults_apply_to_empty_document() {
    let (cfg, warnings) = parse_config("");
    assert!(cfg.show_hud);
    assert!(cfg.show_overlay);
    assert_eq!(cfg.flow_grid_size, 4);
    assert_eq!(cfg.neural_pass, "reshade");
    assert_eq!(cfg.dlss_preset, "cnn-f");
    assert!(cfg.ui_mask_rects.is_empty());
    assert!(warnings.is_empty());
}

#[test]
fn values_read_from_document() {
    let (cfg, warnings) = parse_config(
        r#"
    show_hud = false
    flow_grid_size = 2
    neural_pass = "reshade"
  "#,
    );
    assert!(!cfg.show_hud);
    assert_eq!(cfg.flow_grid_size, 2);
    assert_eq!(cfg.neural_pass, "reshade");
    assert!(warnings.is_empty());
}

#[test]
fn ui_mask_round_trip() {
    let (cfg, warnings) = parse_config(
        r"
    [[ui_mask]]
    left = 0
    top = 900
    right = 1920
    bottom = 1080

    [[ui_mask]]
    left = 1600
    top = 0
    right = 1920
    bottom = 300
  ",
    );
    assert_eq!(cfg.ui_mask_rects.len(), 2);
    assert_eq!(cfg.ui_mask_rects[0].top, 900);
    assert_eq!(cfg.ui_mask_rects[1].left, 1600);
    assert!(warnings.is_empty());
}

#[test]
fn invalid_grid_size_warns() {
    let (cfg, warnings) = parse_config("flow_grid_size = 7");
    assert_eq!(cfg.flow_grid_size, 4);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("flow_grid_size"));
}

#[test]
fn malformed_toml_warns() {
    let (cfg, warnings) = parse_config("this is not = = toml");
    assert!(cfg.show_hud);
    assert!(!warnings.is_empty());
}

#[test]
fn unknown_keys_warn() {
    let (cfg, warnings) = parse_config(
        r"
    show_hud = false
    nonexistent_key = 42
  ",
    );
    assert!(!cfg.show_hud);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("nonexistent_key"));
}

#[test]
fn neural_table_read() {
    let (cfg, warnings) = parse_config(
        r"
    [neural]
    enable_hooks = 1
    intensity = 0.5
    preset = 2
    upscaling = true
  ",
    );
    assert_eq!(cfg.neural.enable_hooks, 1);
    assert_eq!(cfg.neural.intensity, 0.5);
    assert_eq!(cfg.neural.preset, 2);
    assert!(cfg.neural.upscaling);
    assert!(warnings.is_empty());
}

#[test]
fn neural_intensity_clamped() {
    let (cfg, warnings) = parse_config(
        r"
    [neural]
    intensity = 9.0
  ",
    );
    assert_eq!(cfg.neural.intensity, 1.0);
    assert_eq!(warnings.len(), 1);
}

#[test]
fn optional_strength_round_trip() {
    let config = Config::default();
    assert!(config.neural.local_structure < 0.0);
    let (reread, warnings) = parse_config(&serialize_config(&config));
    assert!(reread.neural.local_structure < 0.0);
    assert!(reread.neural.local_tone < 0.0);
    assert!(reread.neural.skin_structure < 0.0);
    assert!(warnings.is_empty());
}

#[test]
fn full_round_trip() {
    let mut config = Config::default();
    config.neural_pass = "reshade".to_string();
    config.dlss_preset = "cnn-e".to_string();
    config.show_hud = false;
    config.flow_grid_size = 2;
    config.synthetic_depth = 0.25;
    config.ui_mask_feather = 8;
    config.neural.intensity = 0.6;
    config.neural.style = 1;
    config.neural.paper_white_scale = 2.5;
    config.neural.skin_structure = 0.75;
    config.ui_mask_rects.push(crate::UiRect {
        left: 0,
        top: 900,
        right: 1920,
        bottom: 1080,
    });

    let (reread, warnings) = parse_config(&serialize_config(&config));
    assert!(warnings.is_empty());
    assert_eq!(reread.neural_pass, config.neural_pass);
    assert_eq!(reread.dlss_preset, config.dlss_preset);
    assert_eq!(reread.show_hud, config.show_hud);
    assert_eq!(reread.flow_grid_size, config.flow_grid_size);
    assert_eq!(reread.synthetic_depth, config.synthetic_depth);
    assert_eq!(reread.ui_mask_feather, config.ui_mask_feather);
    assert_eq!(reread.neural.intensity, config.neural.intensity);
    assert_eq!(reread.neural.style, config.neural.style);
    assert_eq!(
        reread.neural.paper_white_scale,
        config.neural.paper_white_scale
    );
    assert_eq!(reread.neural.skin_structure, config.neural.skin_structure);
    assert_eq!(reread.ui_mask_rects.len(), 1);
    assert_eq!(reread.ui_mask_rects[0].bottom, 1080);
}

#[test]
fn wow_dir_absent_when_empty() {
    let config = Config::default();
    assert!(config.wow_dir.is_empty());
    let text = serialize_config(&config);
    assert!(!text.contains("wow_dir"));
}

#[test]
fn wow_dir_windows_path_round_trip() {
    let mut config = Config::default();
    config.wow_dir = r"D:\World of Warcraft\_retail_".to_string();
    let (reread, warnings) = parse_config(&serialize_config(&config));
    assert!(warnings.is_empty());
    assert_eq!(reread.wow_dir, config.wow_dir);
}

#[test]
fn wow_dir_quotes_round_trip() {
    let mut config = Config::default();
    config.wow_dir = "C:\\Users\\O'Brien\\Games\\\"WoW\"\\_retail_".to_string();
    let (reread, warnings) = parse_config(&serialize_config(&config));
    assert!(warnings.is_empty());
    assert_eq!(reread.wow_dir, config.wow_dir);
}

#[test]
fn wow_dir_non_ascii_round_trip() {
    let mut config = Config::default();
    config.wow_dir = "D:\\Игры\\World of Warcraft".to_string();
    let (reread, warnings) = parse_config(&serialize_config(&config));
    assert!(warnings.is_empty());
    assert_eq!(reread.wow_dir, config.wow_dir);
}

#[test]
fn wow_dir_wrong_type_warns() {
    let (cfg, warnings) = parse_config("wow_dir = 42\n");
    assert!(cfg.wow_dir.is_empty());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("wow_dir"));
    assert!(!warnings[0].contains("unknown key"));
}

#[test]
fn wow_dir_missing_folder_still_loaded() {
    let (cfg, warnings) = parse_config(r#"wow_dir = "Q:\\gone\\_retail_""#);
    assert!(warnings.is_empty());
    assert_eq!(cfg.wow_dir, r"Q:\gone\_retail_");
}

#[test]
fn language_default_english() {
    let (cfg, warnings) = parse_config("");
    assert_eq!(cfg.language, "en");
    assert!(warnings.is_empty());
}

#[test]
fn language_round_trip() {
    let mut config = Config::default();
    config.language = "ru".to_string();
    let (reread, warnings) = parse_config(&serialize_config(&config));
    assert!(warnings.is_empty());
    assert_eq!(reread.language, "ru");
}

#[test]
fn language_unknown_warns() {
    let (cfg, warnings) = parse_config("language = \"xx\"\n");
    assert_eq!(cfg.language, "en");
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("language"));
    assert!(!warnings[0].contains("unknown key"));
}

#[test]
fn language_wrong_type_warns() {
    let (cfg, warnings) = parse_config("language = true\n");
    assert_eq!(cfg.language, "en");
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("language"));
}

#[test]
fn ui_scale_starts_undecided() {
    let (cfg, warnings) = parse_config("");
    assert_eq!(cfg.ui_scale, 0.0);
    assert!(warnings.is_empty());
}

#[test]
fn ui_scale_omitted_when_zero() {
    let config = Config::default();
    assert!(!serialize_config(&config).contains("ui_scale"));
}

#[test]
fn ui_scale_round_trip() {
    let mut config = Config::default();
    config.ui_scale = 1.50;
    let (reread, warnings) = parse_config(&serialize_config(&config));
    assert!(warnings.is_empty());
    assert_eq!(reread.ui_scale, 1.50);
}

#[test]
fn ui_scale_clamped() {
    let (cfg, warnings) = parse_config("ui_scale = 12.0\n");
    assert_eq!(cfg.ui_scale, 3.0);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("ui_scale"));
}

#[test]
fn ui_scale_negative_is_undecided() {
    let (cfg, warnings) = parse_config("ui_scale = -1.0\n");
    assert_eq!(cfg.ui_scale, 0.0);
    assert!(warnings.is_empty());
}

#[test]
fn ui_scale_wrong_type() {
    let (cfg, warnings) = parse_config("ui_scale = \"big\"\n");
    assert_eq!(cfg.ui_scale, 0.0);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("ui_scale"));
}

#[test]
fn neural_passes_default() {
    let (cfg, warnings) = parse_config("");
    assert_eq!(cfg.neural_passes, 1);
    assert!(warnings.is_empty());
}

#[test]
fn neural_passes_round_trip() {
    let mut config = Config::default();
    config.neural_passes = 3;
    let document = serialize_config(&config);
    let (reread, warnings) = parse_config(&document);
    assert!(warnings.is_empty());
    assert_eq!(reread.neural_passes, 3);
}

#[test]
fn neural_passes_clamped() {
    let (many, warnings) = parse_config("neural_passes = 20\n");
    assert_eq!(many.neural_passes, 4);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("neural_passes"));

    let (none, warnings) = parse_config("neural_passes = 0\n");
    assert_eq!(none.neural_passes, 1);
    assert_eq!(warnings.len(), 1);
}

#[test]
fn hotkeys_defaults_and_typo() {
    let (defaults, warnings) = parse_config("");
    assert!(warnings.is_empty());
    assert_eq!(defaults.hotkeys.toggle_overlay, "Ctrl+Alt+D");
    assert_eq!(defaults.hotkeys.toggle_hud, "Ctrl+Alt+H");
    assert_eq!(defaults.hotkeys.start_stop, "Ctrl+Alt+S");

    let (custom, warnings) = parse_config(
        "[hotkeys]\n\
toggle_overlay = \"shift + alt + f9\"\n\
toggle_hud = \"\"\n\
start_stop = \"not a key\"\n",
    );
    assert_eq!(custom.hotkeys.toggle_overlay, "Alt+Shift+F9");
    assert!(custom.hotkeys.toggle_hud.is_empty());
    assert_eq!(custom.hotkeys.start_stop, "Ctrl+Alt+S");
    assert_eq!(warnings.len(), 1);

    let (back, warnings) = parse_config(&serialize_config(&custom));
    assert!(warnings.is_empty());
    assert_eq!(back.hotkeys.toggle_overlay, custom.hotkeys.toggle_overlay);
    assert!(back.hotkeys.toggle_hud.is_empty());
    assert_eq!(back.hotkeys.start_stop, custom.hotkeys.start_stop);
}

#[test]
fn load_and_save_config_file() {
    let mut config = Config::default();
    config.wow_dir = "/opt/wow".to_string();
    config.language = "de".to_string();
    config.ui_scale = 1.25;

    let file = NamedTempFile::new().unwrap();
    let path = file.path();
    save_config(path, &config).unwrap();
    let (loaded, warnings) = load_config(path);
    assert!(warnings.is_empty());
    assert_eq!(loaded.wow_dir, config.wow_dir);
    assert_eq!(loaded.language, config.language);
    assert_eq!(loaded.ui_scale, config.ui_scale);
}

#[test]
fn load_missing_returns_defaults() {
    let (cfg, warnings) = load_config(std::path::Path::new("/nonexistent/sidecar.toml"));
    assert_eq!(cfg, Config::default());
    assert!(warnings.is_empty());
}

#[test]
fn reset_rendering_keeps_install_settings() {
    let mut config = Config::default();
    config.language = "ru".to_string();
    config.wow_dir = "/games/wow".to_string();
    config.ui_scale = 1.5;
    config.hotkeys.toggle_overlay.clear();
    config.show_hud = false;
    config.neural.intensity = 0.2;

    reset_rendering_settings(&mut config);

    assert_eq!(config.language, "ru");
    assert_eq!(config.wow_dir, "/games/wow");
    assert_eq!(config.ui_scale, 1.5);
    assert!(config.hotkeys.toggle_overlay.is_empty());
    assert!(config.show_hud);
    assert_eq!(config.neural.intensity, 1.0);
}

#[test]
fn presets_match_windows_sidecar() {
    for i in 0..PRESETS.len() {
        let mut config = Config::default();
        apply_preset(&mut config, i);
        assert_eq!(matching_preset(&config), Some(i));
    }
}

#[test]
fn neural_strength_sets_pass_count() {
    let mut config = Config::default();
    set_neural_strength(&mut config, NeuralStrength::Stronger);
    assert_eq!(config.neural_passes, 2);
}
