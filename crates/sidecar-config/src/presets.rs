use crate::config::{Config, NeuralSettings};

#[derive(Debug, Clone, Copy)]
pub struct Preset {
    pub name: &'static str,
    pub summary: &'static str,
    pub detail: &'static str,
    apply: fn(&mut Config),
}

pub const PRESETS: [Preset; 4] = [
    Preset {
        name: "Recommended",
        summary: "The tuned default. Start here.",
        detail: concat!(
            "Full neural intensity with the CNN F render preset, which clamps temporal ",
            "history hard -- the right choice when motion vectors are estimated from ",
            "colour rather than rendered by the game."
        ),
        apply: apply_recommended,
    },
    Preset {
        name: "Softer",
        summary: "Half strength. Use if the picture looks over-processed.",
        detail: concat!(
            "The same pipeline with the neural result mixed in at 60%. Cheaper on the ",
            "eyes for interface-heavy scenes, and the first thing to try if faces or ",
            "text look waxy."
        ),
        apply: apply_softer,
    },
    Preset {
        name: "Most stable",
        summary: "For smearing, or flicker on flames and lights.",
        detail: concat!(
            "Switches to CNN E, which clamps temporal history hardest, and eases the ",
            "intensity. This is the preset for when motion looks smeared -- the ",
            "estimated motion vectors are being confidently wrong and this contains ",
            "them."
        ),
        apply: apply_most_stable,
    },
    Preset {
        name: "Off (A/B baseline)",
        summary: "Capture and present, untouched.",
        detail: concat!(
            "No neural work at all, on the same capture and present path. This is the ",
            "honest comparison: whatever you see here is what the overlay costs you ",
            "before any neural rendering happens."
        ),
        apply: apply_off,
    },
];

fn apply_recommended(config: &mut Config) {
    config.neural_pass = "reshade".into();
    config.dlss_preset = "cnn-f".into();
    config.flow_grid_size = 4;
    config.synthetic_depth = 0.5;
    config.neural = NeuralSettings::default();
}

fn apply_softer(config: &mut Config) {
    apply_recommended(config);
    config.neural.intensity = 0.60;
}

fn apply_most_stable(config: &mut Config) {
    config.neural_pass = "reshade".into();
    config.dlss_preset = "cnn-e".into();
    config.flow_grid_size = 2;
    config.synthetic_depth = 0.5;
    config.neural = NeuralSettings::default();
    config.neural.intensity = 0.85;
}

fn apply_off(config: &mut Config) {
    config.neural_pass = "passthrough".into();
}

pub fn apply_preset(config: &mut Config, index: usize) {
    (PRESETS[index].apply)(config);
}

pub fn matching_preset(config: &Config) -> Option<usize> {
    for (i, preset) in PRESETS.iter().enumerate() {
        let mut candidate = Config::default();
        (preset.apply)(&mut candidate);
        if candidate.neural_pass != config.neural_pass {
            continue;
        }
        if candidate.neural_pass == "passthrough" {
            return Some(i);
        }
        if candidate.dlss_preset == config.dlss_preset
            && candidate.flow_grid_size == config.flow_grid_size
            && candidate.synthetic_depth == config.synthetic_depth
            && candidate.neural == config.neural
        {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::parse_config;
    use crate::reset_rendering_settings;
    use crate::strength::{neural_strength_of, set_neural_strength, NeuralStrength};

    #[test]
    fn preset_matching_round_trip() {
        for i in 0..PRESETS.len() {
            let mut config = Config::default();
            apply_preset(&mut config, i);
            assert_eq!(matching_preset(&config), Some(i));
        }
    }

    #[test]
    fn preset_matching_rejects_tweaked_neural_knobs() {
        let mut config = Config::default();
        apply_preset(&mut config, 0);
        config.neural.enable_hooks = 0;
        assert_eq!(matching_preset(&config), None);
    }

    #[test]
    fn neural_strength_maps_pass_counts() {
        let mut config = Config::default();
        set_neural_strength(&mut config, NeuralStrength::Strongest);
        assert_eq!(config.neural_passes, 3);
        assert_eq!(neural_strength_of(&config), Some(NeuralStrength::Strongest));
    }

    #[test]
    fn neural_passes_clamped_in_toml() {
        let (cfg, warnings) = parse_config("neural_passes = 20\n");
        assert_eq!(cfg.neural_passes, 4);
        assert!(warnings.iter().any(|w| w.contains("neural_passes")));

        let (cfg, warnings) = parse_config("neural_passes = 3\n");
        assert_eq!(cfg.neural_passes, 3);
        assert!(warnings.is_empty());
        assert_eq!(neural_strength_of(&cfg), Some(NeuralStrength::Strongest));
    }

    #[test]
    fn reset_rendering_preserves_calibration() {
        let mut config = Config::default();
        config.wow_dir = "/games/wow".into();
        config.neural_passes = 3;
        config.neural.intensity = 0.2;
        reset_rendering_settings(&mut config);
        assert_eq!(config.wow_dir, "/games/wow");
        assert_eq!(config.neural_passes, 1);
        assert_eq!(config.neural.intensity, 1.0);
    }
}
