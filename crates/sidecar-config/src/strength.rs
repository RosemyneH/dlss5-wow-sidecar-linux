use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeuralStrength {
    Natural,
    Stronger,
    Strongest,
}

impl NeuralStrength {
    pub const ALL: [NeuralStrength; 3] = [
        NeuralStrength::Natural,
        NeuralStrength::Stronger,
        NeuralStrength::Strongest,
    ];

    pub fn passes(self) -> u32 {
        match self {
            NeuralStrength::Natural => 1,
            NeuralStrength::Stronger => 2,
            NeuralStrength::Strongest => 3,
        }
    }

    pub fn from_passes(passes: u32) -> Option<Self> {
        match passes {
            1 => Some(NeuralStrength::Natural),
            2 => Some(NeuralStrength::Stronger),
            3 => Some(NeuralStrength::Strongest),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            NeuralStrength::Natural => "1x  Natural",
            NeuralStrength::Stronger => "2x  Stronger",
            NeuralStrength::Strongest => "3x  Strongest",
        }
    }

    pub fn note(self) -> &'static str {
        match self {
            NeuralStrength::Natural => "Closest to the game",
            NeuralStrength::Stronger => "About twice the GPU time",
            NeuralStrength::Strongest => "About three times the GPU time",
        }
    }
}

pub fn set_neural_strength(config: &mut Config, strength: NeuralStrength) {
    config.neural_passes = strength.passes();
}

pub fn neural_strength_of(config: &Config) -> Option<NeuralStrength> {
    NeuralStrength::from_passes(config.neural_passes)
}
