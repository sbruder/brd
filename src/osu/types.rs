use derive_builder::Builder;
use derive_more::{Deref, Display, From};
use num_derive::ToPrimitive;

/// The representation of one screen pixel when osu! is running in 640x480 resolution.
///
/// osupixels are one of the main coordinate systems used in osu!, and apply to hit circle
/// placement and storyboard screen coordinates (these pixels are scaled over a 4:3 ratio to fit
/// your screen).
///
/// ([osu! knowledge base: Glossary: osupixel](https://osu.ppy.sh/help/wiki/Glossary#osupixel))
#[derive(Clone, Debug, Deref, Display, From, PartialEq)]
pub struct OsuPixel(i16);

impl OsuPixel {
    /// Converts osu!mania column to x position
    pub fn from_mania_column(column: u8, columns: u8) -> Self {
        Self((512 * i16::from(column) + 256) / i16::from(columns))
    }
}

/// Special case of [`OsuPixel`] for sliders as they require additional precision.
///
/// [`OsuPixel`]: type.OsuPixel.html
pub type DecimalOsuPixel = f32;

/// Stores time in milliseconds
pub type Time = u32;
/// Special case of [`Time`] for [`General::preview_time`] which has a magic default value of `-1`.
///
/// [`Time`]: type.Time.html
/// [`General::preview_time`]: struct.General.html#structfield.preview_time
pub type SignedTime = i32;

#[derive(ToPrimitive, Clone, Debug, PartialEq)]
pub enum Countdown {
    No = 0,
    Normal = 1,
    Half = 2,
    Double = 3,
}

impl Default for Countdown {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(ToPrimitive, Clone, Debug, PartialEq)]
pub enum Mode {
    Normal = 0,
    Taiko = 1,
    Catch = 2,
    Mania = 3,
}

impl Default for Mode {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(ToPrimitive, Debug, Clone, PartialEq)]
pub enum SampleSet {
    BeatmapDefault = 0,
    Normal = 1,
    Soft = 2,
    Drum = 3,
}

impl Default for SampleSet {
    fn default() -> Self {
        Self::BeatmapDefault
    }
}

#[derive(Clone, Debug, Deref, Display, PartialEq)]
pub struct RangeSetting(f32);

impl From<f32> for RangeSetting {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl RangeSetting {
    pub const MIN: f32 = 0.0;
    pub const MAX: f32 = 10.0;

    pub fn validate(&self) -> bool {
        self.0 >= Self::MIN && self.0 <= Self::MAX
    }
}

/// The sounds played when the object is hit
///
/// By default, no sound is set to `true`, which [uses the normal hitsound](
/// https://osu.ppy.sh/help/wiki/osu!_File_Formats/Osu_(file_format)#hitsounds)
#[derive(Builder, Clone, Debug, Default, PartialEq)]
#[builder(default)]
pub struct HitSound {
    #[builder(default)]
    pub normal: bool,
    #[builder(default)]
    pub whistle: bool,
    #[builder(default)]
    pub finish: bool,
    #[builder(default)]
    pub clap: bool,
}

#[derive(Builder, Clone, Debug, Default, PartialEq)]
#[builder(default)]
pub struct HitSample {
    pub normal_set: SampleSet,
    pub addition_set: SampleSet,
    pub index: u32,
    pub volume: u8,
    #[builder(setter(into))]
    pub filename: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CurveType {
    /// Bézier
    B,
    /// Centripetal catmull-rom
    C,
    /// Linear
    L,
    /// Perfect circle
    P,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        assert_eq!(Countdown::default(), Countdown::Normal);
        assert_eq!(Mode::default(), Mode::Normal);
        assert_eq!(SampleSet::default(), SampleSet::BeatmapDefault);
        assert_eq!(
            HitSound::default(),
            HitSound {
                normal: false,
                whistle: false,
                finish: false,
                clap: false,
            }
        )
    }

    #[test]
    fn range_setting_from_f32() {
        assert_eq!(RangeSetting::from(5.0), RangeSetting(5.0));
    }

    #[test]
    fn range_setting_validate() {
        assert_eq!(RangeSetting(-0.1).validate(), false);
        assert_eq!(RangeSetting(0.0).validate(), true);
        assert_eq!(RangeSetting(5.0).validate(), true);
        assert_eq!(RangeSetting(10.0).validate(), true);
        assert_eq!(RangeSetting(10.1).validate(), false);
    }

    #[test]
    fn osu_pixel_from_mania_column() {
        assert_eq!(OsuPixel::from_mania_column(0, 4), OsuPixel(64));
        assert_eq!(OsuPixel::from_mania_column(3, 4), OsuPixel(448));
        assert_eq!(OsuPixel::from_mania_column(0, 8), OsuPixel(32));
        assert_eq!(OsuPixel::from_mania_column(5, 8), OsuPixel(352));
        assert_eq!(OsuPixel::from_mania_column(7, 8), OsuPixel(480));
    }
}
