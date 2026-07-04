//! Preset / patch data model and TOML (de)serialization.
//!
//! A [`Patch`] fully describes the state of the organ: the nine drawbars, the
//! percussion section, the vibrato/chorus scanner, the Leslie rotary speaker
//! and the overdrive. A [`Preset`] is just a named [`Patch`] that can be stored
//! on disk as human-editable TOML.

use serde::{Deserialize, Serialize};

/// Number of drawbars on a single manual.
pub const DRAWBAR_COUNT: usize = 9;

/// Leslie rotary-speaker speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LeslieSpeed {
    /// Rotors stopped (with slow inertial glide to a halt).
    Brake,
    /// Chorale — slow rotation.
    #[default]
    Slow,
    /// Tremolo — fast rotation.
    Fast,
}

/// Vibrato/Chorus scanner selector, matching the classic C-1..V-3 knob.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VibratoMode {
    #[default]
    Off,
    V1,
    V2,
    V3,
    C1,
    C2,
    C3,
}

impl VibratoMode {
    /// Modulation depth (in samples of a ~5 ms line) and dry mix for this mode.
    /// Vibrato modes are 100% wet; chorus modes blend the dry signal back in.
    pub fn depth_and_dry(self) -> (f32, f32) {
        match self {
            VibratoMode::Off => (0.0, 1.0),
            VibratoMode::V1 => (0.30, 0.0),
            VibratoMode::V2 => (0.55, 0.0),
            VibratoMode::V3 => (0.90, 0.0),
            VibratoMode::C1 => (0.30, 0.5),
            VibratoMode::C2 => (0.55, 0.5),
            VibratoMode::C3 => (0.90, 0.5),
        }
    }

    pub const ALL: [VibratoMode; 7] = [
        VibratoMode::Off,
        VibratoMode::V1,
        VibratoMode::V2,
        VibratoMode::V3,
        VibratoMode::C1,
        VibratoMode::C2,
        VibratoMode::C3,
    ];

    pub fn label(self) -> &'static str {
        match self {
            VibratoMode::Off => "OFF",
            VibratoMode::V1 => "V-1",
            VibratoMode::V2 => "V-2",
            VibratoMode::V3 => "V-3",
            VibratoMode::C1 => "C-1",
            VibratoMode::C2 => "C-2",
            VibratoMode::C3 => "C-3",
        }
    }
}

/// Percussion section (the single-triggered harmonic "ping").
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Percussion {
    pub on: bool,
    /// `false` = Normal level, `true` = Soft.
    pub soft: bool,
    /// `false` = Slow decay, `true` = Fast decay.
    pub fast: bool,
    /// `false` = 2nd harmonic, `true` = 3rd harmonic.
    pub third: bool,
}

impl Default for Percussion {
    fn default() -> Self {
        Self {
            on: true,
            soft: false,
            fast: true,
            third: true,
        }
    }
}

/// A complete organ patch. This is what gets serialized to TOML.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Patch {
    /// Drawbar registration, order `[16', 5⅓', 8', 4', 2⅔', 2', 1⅗', 1⅓', 1']`,
    /// each value in `0..=8`.
    pub drawbars: [u8; DRAWBAR_COUNT],
    pub percussion: Percussion,
    pub vibrato: VibratoMode,
    pub leslie: LeslieSpeed,
    /// Tube overdrive amount, `0.0..=1.0`.
    pub overdrive: f32,
    /// Master output volume, `0.0..=1.0`.
    pub volume: f32,
}

impl Default for Patch {
    fn default() -> Self {
        Self {
            // Classic "88 8000 000" full jazz registration.
            drawbars: [8, 8, 8, 0, 0, 0, 0, 0, 0],
            percussion: Percussion::default(),
            vibrato: VibratoMode::C3,
            leslie: LeslieSpeed::Slow,
            overdrive: 0.15,
            volume: 0.8,
        }
    }
}

impl Patch {
    /// Normalized gain (`0.0..=1.0`) of drawbar `i`.
    #[inline]
    pub fn drawbar_gain(&self, i: usize) -> f32 {
        (self.drawbars[i].min(8) as f32) / 8.0
    }
}

/// A named patch, ready to be written to / read from a `.toml` file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    #[serde(flatten)]
    pub patch: Patch,
}

impl Preset {
    pub fn new(name: impl Into<String>, patch: Patch) -> Self {
        Self {
            name: name.into(),
            patch,
        }
    }

    /// Serialize this preset to a pretty TOML string.
    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    /// Parse a preset from a TOML string.
    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }
}

/// Built-in factory presets shipped with the organ.
pub fn factory_presets() -> Vec<Preset> {
    vec![
        Preset::new(
            "Full Jazz",
            Patch {
                drawbars: [8, 8, 8, 0, 0, 0, 0, 0, 0],
                percussion: Percussion {
                    on: true,
                    soft: false,
                    fast: true,
                    third: true,
                },
                vibrato: VibratoMode::C3,
                leslie: LeslieSpeed::Slow,
                overdrive: 0.1,
                volume: 0.8,
            },
        ),
        Preset::new(
            "Rock Screamer",
            Patch {
                drawbars: [8, 8, 8, 8, 0, 0, 0, 0, 8],
                percussion: Percussion {
                    on: false,
                    soft: false,
                    fast: true,
                    third: true,
                },
                vibrato: VibratoMode::Off,
                leslie: LeslieSpeed::Fast,
                overdrive: 0.7,
                volume: 0.85,
            },
        ),
        Preset::new(
            "Gospel Shout",
            Patch {
                drawbars: [8, 8, 8, 8, 8, 8, 8, 8, 8],
                percussion: Percussion {
                    on: true,
                    soft: false,
                    fast: false,
                    third: false,
                },
                vibrato: VibratoMode::C3,
                leslie: LeslieSpeed::Fast,
                overdrive: 0.4,
                volume: 0.8,
            },
        ),
        Preset::new(
            "Smooth Ballad",
            Patch {
                drawbars: [8, 6, 8, 4, 0, 0, 0, 0, 0],
                percussion: Percussion {
                    on: true,
                    soft: true,
                    fast: false,
                    third: true,
                },
                vibrato: VibratoMode::C2,
                leslie: LeslieSpeed::Slow,
                overdrive: 0.05,
                volume: 0.75,
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_toml_round_trip() {
        let preset = Preset::new("Test", Patch::default());
        let toml = preset.to_toml().expect("serialize");
        let back = Preset::from_toml(&toml).expect("deserialize");
        assert_eq!(preset, back);
    }

    #[test]
    fn all_factory_presets_round_trip() {
        for preset in factory_presets() {
            let toml = preset.to_toml().expect("serialize");
            let back = Preset::from_toml(&toml).expect("deserialize");
            assert_eq!(preset, back, "preset {} did not round-trip", preset.name);
        }
    }

    #[test]
    fn drawbar_gain_is_normalized() {
        let p = Patch {
            drawbars: [0, 8, 4, 0, 0, 0, 0, 0, 0],
            ..Patch::default()
        };
        assert_eq!(p.drawbar_gain(0), 0.0);
        assert_eq!(p.drawbar_gain(1), 1.0);
        assert_eq!(p.drawbar_gain(2), 0.5);
    }

    #[test]
    fn vibrato_labels_are_stable() {
        assert_eq!(VibratoMode::C3.label(), "C-3");
        assert_eq!(VibratoMode::Off.label(), "OFF");
    }
}
