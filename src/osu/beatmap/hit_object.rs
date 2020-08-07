use std::fmt;

use derive_builder::Builder;
use num_traits::ToPrimitive;

use super::super::types::*;
use crate::utils;

/// Represents every hit object type
///
/// The recommended way to construct hit objects is to use the `*Builder` structs of [`HitCircle`],
/// [`Slider`], [`Spinner`] and [`Hold`]. See their respective
/// documentation for examples on how to do that.
/// For constructing osu!mania hit circles, the convenience struct [`ManiaHitCircle`] and its
/// builder is provided.
///
/// [`HitCircle`]: struct.HitCircle.html
/// [`Slider`]: struct.Slider.html
/// [`Spinner`]: struct.Spinner.html
/// [`Hold`]: struct.Hold.html
/// [`ManiaHitCircle`]: struct.ManiaHitCircle.html
#[derive(Clone, Debug, PartialEq)]
pub enum HitObject {
    HitCircle(HitCircle),
    Slider(Slider),
    Spinner(Spinner),
    Hold(Hold),
}

// TODO: deduplicate new_combo and skip_combo_colours
impl HitObject {
    /// Variant independent getter for `new_combo`
    fn new_combo(&self) -> bool {
        match self {
            Self::HitCircle(HitCircle { new_combo, .. })
            | Self::Slider(Slider { new_combo, .. })
            | Self::Spinner(Spinner { new_combo, .. })
            | Self::Hold(Hold { new_combo, .. }) => *new_combo,
        }
    }

    /// Variant independent getter for `skip_combo_colours`
    fn skip_combo_colours(&self) -> u8 {
        match self {
            Self::HitCircle(HitCircle {
                skip_combo_colours, ..
            })
            | Self::Slider(Slider {
                skip_combo_colours, ..
            })
            | Self::Spinner(Spinner {
                skip_combo_colours, ..
            })
            | Self::Hold(Hold {
                skip_combo_colours, ..
            }) => *skip_combo_colours,
        }
    }

    /// Returns the hit object type as `u8` (byte)
    ///
    /// See the [osu! knowledge base] for more information.
    ///
    /// [osu! knowledge base]: https://osu.ppy.sh/help/wiki/osu!_File_Formats/Osu_(file_format)#type
    fn type_byte(&self) -> u8 {
        let type_bit = match self {
            Self::HitCircle { .. } => 0,
            Self::Slider { .. } => 1,
            Self::Spinner { .. } => 3,
            Self::Hold { .. } => 7,
        };
        let hit_object_type = 1u8 << type_bit;

        let new_combo = if self.new_combo() {
            0b0000_0010_u8
        } else {
            0u8
        };

        let skip_combo_colours = (self.skip_combo_colours() & 0b_0000_0111u8) << 3;

        hit_object_type + new_combo + skip_combo_colours
    }
}

impl fmt::Display for HitObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HitObject::HitCircle(HitCircle {
                x,
                y,
                time,
                hit_sound,
                hit_sample,
                ..
            }) => write!(
                f,
                "{},{},{},{},{},{}",
                x,
                y,
                time,
                self.type_byte(),
                hit_sound,
                hit_sample
            ),
            HitObject::Slider(Slider {
                x,
                y,
                time,
                curve_type,
                curve_points,
                slides,
                length,
                edge_sounds,
                edge_sets,
                hit_sound,
                hit_sample,
                ..
            }) => write!(
                f,
                "{},{},{},{},{},{:?}|{},{},{},{},{},{}",
                x,
                y,
                time,
                self.type_byte(),
                hit_sound,
                curve_type,
                curve_points
                    .iter()
                    .map(|point| format!("{}:{}", point.0, point.1))
                    .collect::<Vec<_>>()
                    .join("|"),
                slides,
                length,
                utils::join_display_values(edge_sounds.clone(), "|"),
                edge_sets
                    .iter()
                    .map(|set| format!(
                        "{}:{}",
                        ToPrimitive::to_u16(&set.0).unwrap(),
                        ToPrimitive::to_u16(&set.1).unwrap()
                    ))
                    .collect::<Vec<_>>()
                    .join("|"),
                hit_sample
            ),
            HitObject::Spinner(Spinner {
                time,
                end_time,
                hit_sound,
                hit_sample,
                ..
            }) => write!(
                f,
                "256,192,{},{},{},{},{}",
                time,
                self.type_byte(),
                hit_sound,
                end_time,
                hit_sample
            ),
            HitObject::Hold(Hold {
                column,
                columns,
                time,
                end_time,
                hit_sound,
                hit_sample,
                ..
            }) => write!(
                f,
                "{},192,{},{},{},{}:{}",
                OsuPixel::from_mania_column(*column, *columns),
                time,
                self.type_byte(),
                hit_sound,
                end_time,
                hit_sample
            ),
        }
    }
}

/// Represents a hit circle
///
/// Minimal example:
///
/// ```
/// # use brd::osu::beatmap::hit_object::*;
/// let hit_circle: HitObject = HitCircleBuilder::default()
///     .x(200)
///     .y(400)
///     .time(5000)
///     .build()
///     .unwrap()
///     .into();
/// assert_eq!(format!("{}", hit_circle), "200,400,5000,1,0,0:0:0:0:");
/// ```
#[derive(Builder, Clone, Debug, PartialEq)]
pub struct HitCircle {
    #[builder(setter(into))]
    x: OsuPixel,
    #[builder(setter(into))]
    y: OsuPixel,
    time: Time,
    #[builder(default)]
    hit_sound: HitSound,
    #[builder(default)]
    new_combo: bool,
    #[builder(default)]
    skip_combo_colours: u8,
    #[builder(default)]
    hit_sample: HitSample,
}

impl Into<HitObject> for HitCircle {
    fn into(self) -> HitObject {
        HitObject::HitCircle(self)
    }
}

/// Represents a slider
///
/// Minimal example:
///
/// ```
/// # use brd::osu::{beatmap::hit_object::*, types::*};
/// let slider: HitObject = SliderBuilder::default()
///     .x(200)
///     .y(400)
///     .time(5000)
///     .curve_type(CurveType::B)
///     .curve_points(vec![(20.1, 30.2), (40.3, 50.4)])
///     .length(250.8)
///     .build()
///     .unwrap()
///     .into();
/// assert_eq!(
///     format!("{}", slider),
///     "200,400,5000,2,0,B|20.1:30.2|40.3:50.4,1,250.8,,,0:0:0:0:"
/// );
/// ```
#[derive(Builder, Clone, Debug, PartialEq)]
pub struct Slider {
    #[builder(setter(into))]
    x: OsuPixel,
    #[builder(setter(into))]
    y: OsuPixel,
    time: Time,
    curve_type: CurveType,
    curve_points: Vec<(DecimalOsuPixel, DecimalOsuPixel)>,
    #[builder(default = "1")]
    slides: u8,
    length: DecimalOsuPixel,
    #[builder(default)]
    edge_sounds: Vec<HitSound>,
    #[builder(default)]
    edge_sets: Vec<(SampleSet, SampleSet)>,
    #[builder(default)]
    hit_sound: HitSound,
    #[builder(default)]
    new_combo: bool,
    #[builder(default)]
    skip_combo_colours: u8,
    #[builder(default)]
    hit_sample: HitSample,
}

impl Into<HitObject> for Slider {
    fn into(self) -> HitObject {
        HitObject::Slider(self)
    }
}

/// Represents a spinner
///
/// Minimal example:
///
/// ```
/// # use brd::osu::{beatmap::hit_object::*};
/// let spinner: HitObject = SpinnerBuilder::default()
///     .time(5000)
///     .end_time(10000)
///     .build()
///     .unwrap()
///     .into();
/// assert_eq!(format!("{}", spinner), "256,192,5000,8,0,10000,0:0:0:0:");
/// ```
#[derive(Builder, Clone, Debug, PartialEq)]
pub struct Spinner {
    time: Time,
    end_time: Time,
    #[builder(default)]
    hit_sound: HitSound,
    #[builder(default)]
    new_combo: bool,
    #[builder(default)]
    skip_combo_colours: u8,
    #[builder(default)]
    hit_sample: HitSample,
}

impl Into<HitObject> for Spinner {
    fn into(self) -> HitObject {
        HitObject::Spinner(self)
    }
}

/// Represents a osu!mania hold
///
/// Minimal example:
///
/// ```
/// # use brd::osu::{beatmap::hit_object::*};
/// let hold: HitObject = HoldBuilder::default()
///     .column(2) // columns start at 0 → column 2 is the third column
///     .columns(4)
///     .time(5000)
///     .end_time(10000)
///     .build()
///     .unwrap()
///     .into();
/// assert_eq!(format!("{}", hold), "320,192,5000,128,0,10000:0:0:0:0:");
/// ```
#[derive(Builder, Clone, Debug, PartialEq)]
pub struct Hold {
    column: u8,
    columns: u8,
    time: Time,
    end_time: Time,
    #[builder(default)]
    hit_sound: HitSound,
    #[builder(default)]
    new_combo: bool,
    #[builder(default)]
    skip_combo_colours: u8,
    #[builder(default)]
    hit_sample: HitSample,
}

impl Into<HitObject> for Hold {
    fn into(self) -> HitObject {
        HitObject::Hold(self)
    }
}

/// Helper sturct to build an osu!mania hit circle
///
/// This struct abstracts the creation of osu!mania hit circles, are normal [`HitCircle`]s that use
/// the `x` value to determine the column to display in. `192` is used as the `y` value.
///
/// [`HitCircle`]: struct.HitCircle.html
///
/// Minimal example:
///
/// ```
/// # use brd::osu::{beatmap::hit_object::*};
/// let helper: HitObject = ManiaHitCircleBuilder::default()
///     .column(1)
///     .columns(4)
///     .time(7500)
///     .build()
///     .unwrap()
///     .into();
/// let manual: HitObject = HitCircleBuilder::default()
///     .x(192)
///     .y(192)
///     .time(7500)
///     .build()
///     .unwrap()
///     .into();
/// assert_eq!(helper, manual);
/// ```
#[derive(Builder, Clone, Debug, PartialEq)]
pub struct ManiaHitCircle {
    column: u8,
    columns: u8,
    time: Time,
    #[builder(default)]
    hit_sound: HitSound,
    #[builder(default)]
    new_combo: bool,
    #[builder(default)]
    skip_combo_colours: u8,
    #[builder(default)]
    hit_sample: HitSample,
}

impl Into<HitObject> for ManiaHitCircle {
    fn into(self) -> HitObject {
        HitCircle {
            x: OsuPixel::from_mania_column(self.column, self.columns),
            y: 192.into(),
            time: self.time,
            hit_sound: self.hit_sound,
            new_combo: self.new_combo,
            skip_combo_colours: self.skip_combo_colours,
            hit_sample: self.hit_sample,
        }
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_circle() {
        let object: HitObject = HitCircleBuilder::default()
            .x(200)
            .y(400)
            .time(5732)
            .new_combo(true)
            .skip_combo_colours(5)
            .build()
            .unwrap()
            .into();
        assert_eq!(format!("{}", object), "200,400,5732,43,0,0:0:0:0:");
    }

    #[test]
    fn slider() {
        let object: HitObject = SliderBuilder::default()
            .x(200)
            .y(400)
            .slides(4)
            .time(5732)
            .curve_type(CurveType::B)
            .curve_points(vec![(20.1, 30.2), (40.3, 50.4)])
            .length(250.8)
            .edge_sounds(vec![HitSound::default()])
            .edge_sets(vec![(SampleSet::Normal, SampleSet::Drum)])
            .new_combo(true)
            .skip_combo_colours(5)
            .build()
            .unwrap()
            .into();
        assert_eq!(
            format!("{}", object),
            "200,400,5732,44,0,B|20.1:30.2|40.3:50.4,4,250.8,0,1:3,0:0:0:0:"
        );
    }

    #[test]
    fn spinner() {
        let object: HitObject = SpinnerBuilder::default()
            .time(5000)
            .end_time(10000)
            .new_combo(true)
            .skip_combo_colours(5)
            .build()
            .unwrap()
            .into();
        assert_eq!(format!("{}", object), "256,192,5000,50,0,10000,0:0:0:0:")
    }

    #[test]
    fn hold() {
        let object: HitObject = HoldBuilder::default()
            .column(2)
            .columns(4)
            .time(6000)
            .end_time(9000)
            .new_combo(true)
            .skip_combo_colours(5)
            .build()
            .unwrap()
            .into();
        assert_eq!(format!("{}", object), "320,192,6000,170,0,9000:0:0:0:0:");
    }
}
