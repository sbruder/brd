//! The description format of an osu! beatmap.
//!
//! The beatmap file format is described in the [osu! knowledge base].
//!
//! Example (building a minimal beatmap):
//!
//! ```
//! # use brd::osu::beatmap;
//! let awesome_beatmap = beatmap::BeatmapBuilder::default()
//!     .general(
//!         beatmap::GeneralBuilder::default()
//!             .audio_filename("audio.mp3")
//!             .build()
//!             .unwrap(),
//!     )
//!     .metadata(
//!         beatmap::MetadataBuilder::default()
//!             .title("My awesome song")
//!             .artist("Awesome artist")
//!             .creator("Me")
//!             .version("Hard")
//!             .source("Awesome songs vol.3")
//!             .build()
//!             .unwrap(),
//!     )
//!     .difficulty(
//!         beatmap::DifficultyBuilder::default()
//!             .hp_drain_rate(4.0)
//!             .circle_size(4.0)
//!             .overall_difficulty(3.0)
//!             .approach_rate(8.0)
//!             .slider_multiplier(0.64)
//!             .slider_tick_rate(1.0)
//!             .build()
//!             .unwrap(),
//!     )
//!     .timing_points(beatmap::TimingPoints(vec![
//!         beatmap::TimingPointBuilder::default()
//!             .time(0)
//!             .beat_length(1000.0 / 3.0)
//!             .build()
//!             .unwrap(),
//!     ]))
//!     .hit_objects(beatmap::HitObjects(vec![
//!         beatmap::hit_object::HitCircleBuilder::default()
//!             .x(256)
//!             .y(192)
//!             .time(8000)
//!             .build()
//!             .unwrap()
//!             .into(),
//!     ]))
//!     .build()
//!     .unwrap();
//!
//! assert_eq!(
//!     format!("{}", awesome_beatmap),
//!     r#"osu file format v14
//!
//! [General]
//! AudioFilename: audio.mp3
//! AudioLeadIn: 0
//! PreviewTime: -1
//! Countdown: 1
//! SampleSet: Normal
//! Mode: 0
//!
//! [Editor]
//!
//! [Metadata]
//! Title:My awesome song
//! Artist:Awesome artist
//! Creator:Me
//! Version:Hard
//! Source:Awesome songs vol.3
//! Tags:
//!
//! [Difficulty]
//! HPDrainRate:4
//! CircleSize:4
//! OverallDifficulty:3
//! ApproachRate:8
//! SliderMultiplier:0.64
//! SliderTickRate:1
//!
//! [Events]
//!
//!
//! [TimingPoints]
//! 0,333.33334,4,0,0,100,1,0
//!
//! [Colours]
//!
//!
//! [HitObjects]
//! 256,192,8000,1,0,0:0:0:0:
//! "#
//! );
//! ```
//!
//! [osu! knowledge base]: https://osu.ppy.sh/help/wiki/osu!_File_Formats/Osu_(file_format)
pub mod hit_object;
pub use hit_object::HitObject;

use std::fmt;

use derive_builder::Builder;
use derive_more::{Deref, DerefMut};
use num_traits::ToPrimitive;

use super::types::*;
use crate::utils;

#[derive(Builder, Clone)]
pub struct General {
    #[builder(setter(into))]
    pub audio_filename: String,
    #[builder(default)]
    pub audio_lead_in: Time,
    #[builder(default = "-1")]
    pub preview_time: SignedTime,
    #[builder(default)]
    pub countdown: Countdown,
    // SampleSet’s normal default (BeatmapDefault) does not make sense here
    #[builder(default = "SampleSet::Normal")]
    pub sample_set: SampleSet,
    #[builder(default)]
    pub mode: Mode,
}

impl fmt::Display for General {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\
            [General]\n\
            AudioFilename: {}\n\
            AudioLeadIn: {}\n\
            PreviewTime: {}\n\
            Countdown: {}\n\
            SampleSet: {:?}\n\
            Mode: {}\n\
            ",
            self.audio_filename,
            self.audio_lead_in,
            self.preview_time,
            ToPrimitive::to_u8(&self.countdown).unwrap(),
            self.sample_set,
            ToPrimitive::to_u8(&self.mode).unwrap()
        )
    }
}

#[derive(Clone, Default)]
pub struct Editor;

impl fmt::Display for Editor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "[Editor]")
    }
}

#[derive(Builder, Clone)]
#[builder(setter(into))]
pub struct Metadata {
    pub title: String,
    pub artist: String,
    #[builder(default = "\"brd::osu\".to_string()")]
    pub creator: String,
    pub version: String,
    #[builder(default)]
    pub source: String,
    #[builder(default)]
    pub tags: Vec<String>,
}

impl fmt::Display for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\
            [Metadata]\n\
            Title:{}\n\
            Artist:{}\n\
            Creator:{}\n\
            Version:{}\n\
            Source:{}\n\
            Tags:{}\n\
            ",
            self.title,
            self.artist,
            self.creator,
            self.version,
            self.source,
            self.tags.join(" ")
        )
    }
}

#[derive(Builder, Clone, Debug)]
#[builder(build_fn(validate = "Self::validate"))]
pub struct Difficulty {
    #[builder(setter(into))]
    pub hp_drain_rate: RangeSetting,
    /// Also is the number of keys in mania
    #[builder(setter(into))]
    pub circle_size: RangeSetting,
    #[builder(setter(into))]
    pub overall_difficulty: RangeSetting,
    #[builder(setter(into))]
    pub approach_rate: RangeSetting,
    pub slider_multiplier: f32,
    pub slider_tick_rate: f32,
}

impl DifficultyBuilder {
    fn validate_option(maybe_value: &Option<RangeSetting>, name: &str) -> Result<(), String> {
        if let Some(value) = maybe_value {
            if !value.validate() {
                return Err(format!(
                    "{} has to be between {} and {}",
                    name,
                    RangeSetting::MIN,
                    RangeSetting::MAX
                ));
            }
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), String> {
        Self::validate_option(&self.hp_drain_rate, "hp_drain_rate")?;
        Self::validate_option(&self.circle_size, "circle_size")?;
        Self::validate_option(&self.overall_difficulty, "overall_difficulty")?;
        Self::validate_option(&self.approach_rate, "approach_rate")?;
        Ok(())
    }
}

impl fmt::Display for Difficulty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\
            [Difficulty]\n\
            HPDrainRate:{}\n\
            CircleSize:{}\n\
            OverallDifficulty:{}\n\
            ApproachRate:{}\n\
            SliderMultiplier:{}\n\
            SliderTickRate:{}\n\
            ",
            self.hp_drain_rate,
            self.circle_size,
            self.overall_difficulty,
            self.approach_rate,
            self.slider_multiplier,
            self.slider_tick_rate
        )
    }
}

#[derive(Clone, Default, Deref, DerefMut)]
pub struct Events(pub Vec<Event>);

impl fmt::Display for Events {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\
            [Events]\n\
            {}\n\
            ",
            utils::join_display_values(self.to_vec(), "\n")
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Background {
        filename: String,
        x_offset: OsuPixel,
        y_offset: OsuPixel,
    },
    Video {
        filename: String,
        start_time: Time,
        x_offset: OsuPixel,
        y_offset: OsuPixel,
    },
    Break {
        start_time: Time,
        end_time: Time,
    },
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::Background {
                filename,
                x_offset,
                y_offset,
            } => write!(f, "0,0,{},{},{}", filename, x_offset, y_offset),
            Event::Video {
                start_time,
                filename,
                x_offset,
                y_offset,
            } => write!(
                f,
                "Video,{},{},{},{}",
                start_time, filename, x_offset, y_offset
            ),
            Event::Break {
                start_time,
                end_time,
            } => write!(f, "Break,{},{}", start_time, end_time),
        }
    }
}

#[derive(Clone, Default, Deref, DerefMut)]
pub struct TimingPoints(pub Vec<TimingPoint>);

impl fmt::Display for TimingPoints {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\
            [TimingPoints]\n\
            {}\n\
            ",
            utils::join_display_values(self.to_vec(), "\n")
        )
    }
}

#[derive(Builder, Clone, Default)]
pub struct TimingPointEffects {
    pub kiai_time: bool,
    pub omit_first_barline: bool,
}

impl fmt::Display for TimingPointEffects {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            utils::bitarray_to_byte([
                self.kiai_time,
                false,
                false,
                self.omit_first_barline,
                false,
                false,
                false,
                false
            ])
        )
    }
}

#[derive(Builder, Clone)]
pub struct TimingPoint {
    pub time: Time,
    pub beat_length: f32,
    #[builder(default = "4")]
    pub meter: u8,
    #[builder(default = "SampleSet::BeatmapDefault")]
    pub sample_set: SampleSet,
    #[builder(default = "0")]
    pub sample_index: u32,
    #[builder(default = "100")]
    pub volume: u8,
    #[builder(default = "true")]
    pub uninherited: bool,
    #[builder(default)]
    pub effects: TimingPointEffects,
}

impl fmt::Display for TimingPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{},{},{},{},{},{},{},{}",
            self.time,
            self.beat_length,
            self.meter,
            ToPrimitive::to_u8(&self.sample_set).unwrap(),
            self.sample_index,
            self.volume,
            self.uninherited as u8,
            self.effects
        )
    }
}

#[derive(Clone, Default, Deref, DerefMut)]
pub struct Colours(pub Vec<Colour>);

impl fmt::Display for Colours {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\
            [Colours]\n\
            {}\n\
            ",
            utils::join_display_values(self.to_vec(), "\n")
        )
    }
}

#[derive(Clone, Debug)]
pub enum ColourScope {
    Combo(u16),
    SliderTrackOverride,
    SliderBorder,
}

impl fmt::Display for ColourScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColourScope::Combo(i) => write!(f, "Combo{}", i),
            _ => write!(f, "{:?}", self),
        }
    }
}

#[derive(Builder, Clone)]
pub struct Colour {
    pub scope: ColourScope,
    pub colour: [u8; 3],
}

impl fmt::Display for Colour {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} : {}",
            self.scope,
            utils::join_display_values(self.colour.to_vec(), ",")
        )
    }
}

impl fmt::Display for HitSound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            utils::bitarray_to_byte([
                self.normal,
                self.whistle,
                self.finish,
                self.clap,
                false,
                false,
                false,
                false
            ])
        )
    }
}

impl fmt::Display for HitSample {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}:{}",
            ToPrimitive::to_u8(&self.normal_set).unwrap(),
            ToPrimitive::to_u8(&self.addition_set).unwrap(),
            self.index,
            self.volume,
            self.filename
        )
    }
}

#[derive(Clone, Default, Deref, DerefMut)]
pub struct HitObjects(pub Vec<HitObject>);

impl fmt::Display for HitObjects {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\
            [HitObjects]\n\
            {}\n\
            ",
            utils::join_display_values(self.to_vec(), "\n")
        )
    }
}

#[derive(Builder)]
pub struct Beatmap {
    #[builder(default = "14")]
    pub version: u8,
    pub general: General,
    #[builder(default)]
    pub editor: Editor,
    pub metadata: Metadata,
    pub difficulty: Difficulty,
    #[builder(default)]
    pub events: Events,
    pub timing_points: TimingPoints,
    #[builder(default)]
    pub colours: Colours,
    pub hit_objects: HitObjects,
}

impl fmt::Display for Beatmap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "osu file format v{}\n\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
            self.version,
            self.general.clone(),
            self.editor.clone(),
            self.metadata.clone(),
            self.difficulty.clone(),
            self.events.clone(),
            self.timing_points.clone(),
            self.colours.clone(),
            self.hit_objects.clone()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general() {
        let general = GeneralBuilder::default()
            .audio_filename("foo.mp3")
            .audio_lead_in(23)
            .preview_time(5000)
            .countdown(Countdown::Double)
            .sample_set(SampleSet::Drum)
            .mode(Mode::Mania)
            .build()
            .unwrap();
        assert_eq!(
            format!("{}", general),
            "[General]\n\
            AudioFilename: foo.mp3\n\
            AudioLeadIn: 23\n\
            PreviewTime: 5000\n\
            Countdown: 3\n\
            SampleSet: Drum\n\
            Mode: 3\n",
        )
    }

    #[test]
    fn editor() {
        assert_eq!(format!("{}", Editor), "[Editor]\n");
    }

    #[test]
    fn metadata() {
        let metadata = MetadataBuilder::default()
            .title("Song Title")
            .artist("Song Artist")
            .creator("mycoolusername42")
            .version("Super Hard")
            .source("Best Hits Vol. 23")
            .tags(vec![
                "some".to_string(),
                "descriptive".to_string(),
                "tags".to_string(),
            ])
            .build()
            .unwrap();
        assert_eq!(
            format!("{}", metadata),
            "[Metadata]\n\
            Title:Song Title\n\
            Artist:Song Artist\n\
            Creator:mycoolusername42\n\
            Version:Super Hard\n\
            Source:Best Hits Vol. 23\n\
            Tags:some descriptive tags\n"
        );
    }

    #[test]
    fn dificulty_builder_error() {
        assert_eq!(
            DifficultyBuilder::default()
                .hp_drain_rate(25.0)
                .circle_size(5.0)
                .overall_difficulty(5.0)
                .approach_rate(5.0)
                .build()
                .unwrap_err(),
            "hp_drain_rate has to be between 0 and 10"
        );
    }

    #[test]
    fn difficulty() {
        let difficulty = DifficultyBuilder::default()
            .hp_drain_rate(4.0)
            .circle_size(5.0)
            .overall_difficulty(6.0)
            .approach_rate(7.0)
            .slider_multiplier(0.64)
            .slider_tick_rate(1.0)
            .build()
            .unwrap();
        assert_eq!(
            format!("{}", difficulty),
            "[Difficulty]\n\
            HPDrainRate:4\n\
            CircleSize:5\n\
            OverallDifficulty:6\n\
            ApproachRate:7\n\
            SliderMultiplier:0.64\n\
            SliderTickRate:1\n"
        )
    }

    #[test]
    fn events() {
        let mut events = Events(Vec::new());
        events.push(Event::Background {
            filename: "foo.jpg".to_string(),
            x_offset: 42.into(),
            y_offset: 23.into(),
        });
        events.push(Event::Video {
            filename: "foo.mp4".to_string(),
            start_time: 500,
            x_offset: 42.into(),
            y_offset: 23.into(),
        });
        events.push(Event::Break {
            start_time: 23000,
            end_time: 42000,
        });
        assert_eq!(
            format!("{}", events),
            "[Events]\n\
            0,0,foo.jpg,42,23\n\
            Video,500,foo.mp4,42,23\n\
            Break,23000,42000\n"
        )
    }

    #[test]
    fn timing_points() {
        let mut timing_points = TimingPoints(Vec::new());
        timing_points.push(
            TimingPointBuilder::default()
                .time(0)
                .beat_length(1000.0 / 3.0)
                .build()
                .unwrap(),
        );
        timing_points.push(
            TimingPointBuilder::default()
                .time(5000)
                .beat_length(500.0)
                .meter(8)
                .sample_set(SampleSet::Drum)
                .sample_index(1)
                .volume(50)
                .uninherited(false)
                .effects(
                    TimingPointEffectsBuilder::default()
                        .kiai_time(true)
                        .omit_first_barline(true)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap(),
        );
        assert_eq!(
            format!("{}", timing_points),
            "[TimingPoints]\n\
            0,333.33334,4,0,0,100,1,0\n\
            5000,500,8,3,1,50,0,9\n"
        );
    }

    #[test]
    fn colours() {
        let mut colours = Colours::default();
        colours.push(
            ColourBuilder::default()
                .scope(ColourScope::Combo(42))
                .colour([0, 127, 255])
                .build()
                .unwrap(),
        );
        colours.push(
            ColourBuilder::default()
                .scope(ColourScope::SliderTrackOverride)
                .colour([127, 255, 0])
                .build()
                .unwrap(),
        );
        colours.push(
            ColourBuilder::default()
                .scope(ColourScope::SliderBorder)
                .colour([255, 0, 127])
                .build()
                .unwrap(),
        );
        assert_eq!(
            format!("{}", colours),
            "[Colours]\n\
            Combo42 : 0,127,255\n\
            SliderTrackOverride : 127,255,0\n\
            SliderBorder : 255,0,127\n"
        )
    }

    #[test]
    fn hit_sound() {
        assert_eq!(format!("{}", HitSound::default()), "0");
        assert_eq!(
            format!(
                "{}",
                HitSoundBuilder::default().normal(true).build().unwrap()
            ),
            "1"
        );
        assert_eq!(
            format!(
                "{}",
                HitSoundBuilder::default().whistle(true).build().unwrap()
            ),
            "2"
        );
        assert_eq!(
            format!(
                "{}",
                HitSoundBuilder::default().finish(true).build().unwrap()
            ),
            "4"
        );
        assert_eq!(
            format!("{}", HitSoundBuilder::default().clap(true).build().unwrap()),
            "8"
        );
    }

    #[test]
    fn hit_sample() {
        assert_eq!(format!("{}", HitSample::default()), "0:0:0:0:");
        assert_eq!(
            format!(
                "{}",
                HitSampleBuilder::default()
                    .normal_set(SampleSet::Drum)
                    .addition_set(SampleSet::Normal)
                    .index(23)
                    .volume(42)
                    .filename("foo.mp3")
                    .build()
                    .unwrap()
            ),
            "3:1:23:42:foo.mp3"
        );
    }

    #[test]
    fn hit_objects() {
        let mut hit_objects: HitObjects = Default::default();
        hit_objects.push(
            hit_object::HitCircleBuilder::default()
                .x(200)
                .y(400)
                .time(5732)
                .build()
                .unwrap()
                .into(),
        );
        hit_objects.push(
            hit_object::HitCircleBuilder::default()
                .x(400)
                .y(500)
                .time(7631)
                .build()
                .unwrap()
                .into(),
        );
        assert_eq!(
            format!("{}", hit_objects),
            "[HitObjects]\n\
            200,400,5732,1,0,0:0:0:0:\n\
            400,500,7631,1,0,0:0:0:0:\n"
        );
    }
}
