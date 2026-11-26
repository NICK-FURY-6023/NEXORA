//! The Nexora nexora filter chain.
//!
//! Nexora ships ten built-in filters; Nexora adds more. Every parameter
//! name and default below is read from the shipped reference:
//! `filterConfigs.kt` for Nexora, `src/typings/playback/filters.types.ts`
//! for the Nexora extras.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One equalizer band.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NexoraEqualizerBand {
    pub band: i64,
    pub gain: f32,
}

/// The `karaoke` filter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NexoraKaraokeFilter {
    #[serde(default = "one")]
    pub level: f32,
    #[serde(default = "one", rename = "monoLevel")]
    pub mono_level: f32,
    #[serde(default = "karaoke_filter_band", rename = "filterBand")]
    pub filter_band: f32,
    #[serde(default = "karaoke_filter_width", rename = "filterWidth")]
    pub filter_width: f32,
}

/// The `timescale` filter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NexoraTimescaleFilter {
    #[serde(default = "one")]
    pub speed: f32,
    #[serde(default = "one")]
    pub pitch: f32,
    #[serde(default = "one")]
    pub rate: f32,
}

/// The `tremolo` filter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NexoraTremoloFilter {
    #[serde(default = "tremolo_frequency")]
    pub frequency: f32,
    #[serde(default = "half")]
    pub depth: f32,
}

/// The `vibrato` filter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NexoraVibratoFilter {
    #[serde(default = "vibrato_frequency")]
    pub frequency: f32,
    #[serde(default = "half")]
    pub depth: f32,
}

/// The `distortion` filter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NexoraDistortionFilter {
    #[serde(default)]
    pub sin_offset: f32,
    #[serde(default = "one")]
    pub sin_scale: f32,
    #[serde(default)]
    pub cos_offset: f32,
    #[serde(default = "one")]
    pub cos_scale: f32,
    #[serde(default)]
    pub tan_offset: f32,
    #[serde(default = "one")]
    pub tan_scale: f32,
    #[serde(default)]
    pub offset: f32,
    #[serde(default = "one")]
    pub scale: f32,
}

/// The `rotation` filter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NexoraRotationFilter {
    #[serde(default, rename = "rotationHz")]
    pub rotation_hz: f32,
}

/// The `channelMix` filter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NexoraChannelMixFilter {
    #[serde(default = "one", rename = "leftToLeft")]
    pub left_to_left: f32,
    #[serde(default, rename = "leftToRight")]
    pub left_to_right: f32,
    #[serde(default, rename = "rightToLeft")]
    pub right_to_left: f32,
    #[serde(default = "one", rename = "rightToRight")]
    pub right_to_right: f32,
}

/// The `lowPass` filter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NexoraLowPassFilter {
    #[serde(default = "low_pass_smoothing")]
    pub smoothing: f32,
}

/// The Nexora `highpass` filter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NexoraHighPassFilter {
    #[serde(default = "low_pass_smoothing")]
    pub smoothing: f32,
}

/// The Nexora `compressor` filter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NexoraCompressorFilter {
    #[serde(default)]
    pub threshold: f32,
    #[serde(default = "one")]
    pub ratio: f32,
    #[serde(default)]
    pub attack: f32,
    #[serde(default)]
    pub release: f32,
    #[serde(default)]
    pub gain: f32,
    #[serde(default, rename = "makeupGain")]
    pub makeup_gain: f32,
}

/// The Nexora `echo` filter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NexoraEchoFilter {
    #[serde(default)]
    pub delay: f32,
    #[serde(default)]
    pub feedback: f32,
    #[serde(default)]
    pub mix: f32,
}

/// The Nexora `phaser` filter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NexoraPhaserFilter {
    #[serde(default)]
    pub rate: f32,
    #[serde(default)]
    pub depth: f32,
    #[serde(default)]
    pub feedback: f32,
    #[serde(default)]
    pub delay: f32,
    #[serde(default)]
    pub mix: f32,
    #[serde(default = "phaser_stages")]
    pub stages: u32,
    #[serde(default = "phaser_min_frequency", rename = "minFrequency")]
    pub min_frequency: f32,
    #[serde(default = "phaser_max_frequency", rename = "maxFrequency")]
    pub max_frequency: f32,
}

/// The whole filter chain of a player.
///
/// The ten Nexora built-ins come first in the reference's declaration order;
/// the four Nexora extras follow. `pluginFilters` carries the reference's
/// plugin-extension map, which NEXORA leaves empty.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NexoraFilters {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equalizer: Option<Vec<NexoraEqualizerBand>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub karaoke: Option<NexoraKaraokeFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timescale: Option<NexoraTimescaleFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tremolo: Option<NexoraTremoloFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vibrato: Option<NexoraVibratoFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distortion: Option<NexoraDistortionFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<NexoraRotationFilter>,
    #[serde(
        default,
        rename = "channelMix",
        alias = "channelmix",
        skip_serializing_if = "Option::is_none"
    )]
    pub channel_mix: Option<NexoraChannelMixFilter>,
    #[serde(
        default,
        rename = "lowPass",
        alias = "lowpass",
        skip_serializing_if = "Option::is_none"
    )]
    pub low_pass: Option<NexoraLowPassFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highpass: Option<NexoraHighPassFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compressor: Option<NexoraCompressorFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub echo: Option<NexoraEchoFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phaser: Option<NexoraPhaserFilter>,
    #[serde(
        default,
        rename = "pluginFilters",
        skip_serializing_if = "Option::is_none"
    )]
    pub plugin_filters: Option<Value>,
}

impl NexoraFilters {
    /// The reference's names of the filters this chain carries.
    pub fn enabled_names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        if self.volume.is_some() {
            names.push("volume");
        }
        if self.equalizer.is_some() {
            names.push("equalizer");
        }
        if self.karaoke.is_some() {
            names.push("karaoke");
        }
        if self.timescale.is_some() {
            names.push("timescale");
        }
        if self.tremolo.is_some() {
            names.push("tremolo");
        }
        if self.vibrato.is_some() {
            names.push("vibrato");
        }
        if self.distortion.is_some() {
            names.push("distortion");
        }
        if self.rotation.is_some() {
            names.push("rotation");
        }
        if self.channel_mix.is_some() {
            names.push("channelMix");
        }
        if self.low_pass.is_some() {
            names.push("lowPass");
        }
        if self.highpass.is_some() {
            names.push("highpass");
        }
        if self.compressor.is_some() {
            names.push("compressor");
        }
        if self.echo.is_some() {
            names.push("echo");
        }
        if self.phaser.is_some() {
            names.push("phaser");
        }
        names
    }
}

fn one() -> f32 {
    1.0
}

fn half() -> f32 {
    0.5
}

fn karaoke_filter_band() -> f32 {
    220.0
}

fn karaoke_filter_width() -> f32 {
    100.0
}

fn tremolo_frequency() -> f32 {
    2.0
}

fn vibrato_frequency() -> f32 {
    2.0
}

fn low_pass_smoothing() -> f32 {
    20.0
}

fn phaser_stages() -> u32 {
    6
}

fn phaser_min_frequency() -> f32 {
    440.0
}

fn phaser_max_frequency() -> f32 {
    0.1
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn filter_json_uses_the_reference_parameter_names() {
        let filters = NexoraFilters {
            karaoke: Some(NexoraKaraokeFilter {
                level: 1.0,
                mono_level: 1.0,
                filter_band: 220.0,
                filter_width: 100.0,
            }),
            channel_mix: Some(NexoraChannelMixFilter {
                left_to_left: 1.0,
                left_to_right: 0.0,
                right_to_left: 0.0,
                right_to_right: 1.0,
            }),
            low_pass: Some(NexoraLowPassFilter { smoothing: 20.0 }),
            ..NexoraFilters::default()
        };
        let value = serde_json::to_value(&filters).unwrap();
        assert_eq!(value["karaoke"]["monoLevel"], 1.0);
        assert_eq!(value["karaoke"]["filterBand"], 220.0);
        assert_eq!(value["karaoke"]["filterWidth"], 100.0);
        assert_eq!(value["channelMix"]["leftToLeft"], 1.0);
        assert_eq!(value["channelMix"]["rightToRight"], 1.0);
        assert_eq!(value["lowPass"]["smoothing"], 20.0);
        assert!(value.get("timescale").is_none());
    }

    #[test]
    fn node_link_lowercase_lowpass_is_accepted() {
        let filters: NexoraFilters =
            serde_json::from_value(json!({ "lowpass": { "smoothing": 30.0 } })).unwrap();
        assert_eq!(filters.low_pass.unwrap().smoothing, 30.0);
    }

    #[test]
    fn missing_parameters_fall_back_to_the_reference_defaults() {
        let filters: NexoraFilters = serde_json::from_value(json!({ "karaoke": {} })).unwrap();
        let karaoke = filters.karaoke.unwrap();
        assert_eq!(karaoke.level, 1.0);
        assert_eq!(karaoke.mono_level, 1.0);
        assert_eq!(karaoke.filter_band, 220.0);
        assert_eq!(karaoke.filter_width, 100.0);
    }

    #[test]
    fn enabled_names_lists_the_chain_in_reference_order() {
        let filters = NexoraFilters {
            volume: Some(0.5),
            equalizer: Some(vec![NexoraEqualizerBand { band: 1, gain: 0.5 }]),
            timescale: Some(NexoraTimescaleFilter {
                speed: 2.0,
                pitch: 1.0,
                rate: 1.0,
            }),
            ..NexoraFilters::default()
        };
        assert_eq!(
            filters.enabled_names(),
            vec!["volume", "equalizer", "timescale"]
        );
    }
}
