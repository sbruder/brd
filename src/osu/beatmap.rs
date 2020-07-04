use std::fmt;

use derive_more::{Deref, DerefMut};
use num_derive::ToPrimitive;
use num_traits::ToPrimitive;

use crate::utils;

// Generic Type Aliases
pub type OsuPixel = i16;
pub type DecimalOsuPixel = f32;

pub type SampleIndex = u16;

pub type Time = u32;

// Helper functions
fn bitflags(flags: [bool; 8]) -> u8 {
    let mut value = 0u8;
    for (i, flag) in flags.iter().enumerate() {
        value += ((0b1 as u8) << i) * (*flag as u8) as u8;
    }
    value
}

fn assemble_hit_object_type(hit_object_type: u8, new_combo: bool, skip_combo_colours: U3) -> u8 {
    let hit_object_type = 1u8 << hit_object_type;
    let new_combo = if new_combo { 0b0000_0010_u8 } else { 0u8 };
    let skip_combo_colours = (skip_combo_colours & 0b_0000_0111u8) << 1;
    hit_object_type + new_combo + skip_combo_colours
}

pub fn column_to_x(column: u8, columns: u8) -> OsuPixel {
    (512 * OsuPixel::from(column) + 256) / OsuPixel::from(columns)
}

#[derive(ToPrimitive, Clone)]
pub enum Countdown {
    No = 0,
    Normal = 1,
    Half = 2,
    Double = 3,
}

#[derive(ToPrimitive, Clone)]
pub enum Mode {
    Normal = 0,
    Taiko = 1,
    Catch = 2,
    Mania = 3,
}

#[derive(ToPrimitive, Debug, Clone)]
pub enum SampleSet {
    BeatmapDefault = 0,
    Normal = 1,
    Soft = 2,
    Drum = 3,
}

#[derive(Clone)]
pub struct General {
    pub audio_filename: String,
    pub audio_lead_in: Time,
    pub preview_time: Time,
    pub countdown: Countdown,
    pub sample_set: SampleSet,
    pub mode: Mode,
}

impl fmt::Display for General {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

#[derive(Clone)]
pub struct Editor {/* stub */}

impl fmt::Display for Editor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "[Editor]")
    }
}

#[derive(Clone)]
pub struct Metadata {
    pub title: String,
    pub artist: String,
    pub creator: String,
    pub version: String,
    pub source: String,
    pub tags: Vec<String>,
}

impl fmt::Display for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

#[derive(Clone)]
pub struct Difficulty {
    pub hp_drain_rate: f32,
    /// Also is the number of keys in mania
    pub circle_size: f32,
    pub overall_difficulty: f32,
    pub approach_rate: f32,
    pub slider_multiplier: f32,
    pub slider_tick_rate: f32,
}

impl fmt::Display for Difficulty {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

#[derive(Clone, Deref)]
pub struct Events(pub Vec<Event>);

impl fmt::Display for Events {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

#[derive(Clone)]
pub enum Event {
    Background {
        filename: String,
        x_offset: OsuPixel,
        y_offset: OsuPixel,
    },
    Video {
        start_time: Time,
        filename: String,
        x_offset: OsuPixel,
        y_offset: OsuPixel,
    },
    Break {
        start_time: Time,
        end_time: Time,
    },
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

#[derive(Clone, Deref, DerefMut)]
pub struct TimingPoints(pub Vec<TimingPoint>);

impl fmt::Display for TimingPoints {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

#[derive(Clone)]
pub struct TimingPointEffects {
    pub kiai_time: bool,
    pub omit_first_barline: bool,
}

impl fmt::Display for TimingPointEffects {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            bitflags([
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

#[derive(Clone)]
pub struct TimingPoint {
    pub time: Time,
    pub beat_length: f32,
    pub meter: u8,
    pub sample_set: SampleSet,
    pub sample_index: SampleIndex,
    pub volume: u8,
    pub uninherited: bool,
    pub effects: TimingPointEffects,
}

impl fmt::Display for TimingPoint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

#[derive(Clone, Deref)]
pub struct Colours(pub Vec<Colour>);

impl fmt::Display for Colours {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ColourScope::Combo(i) => write!(f, "Combo{}", i),
            _ => write!(f, "{:?}", self),
        }
    }
}

#[derive(Clone)]
pub struct Colour {
    pub scope: ColourScope,
    pub colour: [u8; 3],
}

impl fmt::Display for Colour {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} : {}",
            self.scope,
            utils::join_display_values(self.colour.to_vec(), ",")
        )
    }
}

#[derive(Clone)]
pub struct HitSound {
    pub normal: bool,
    pub whistle: bool,
    pub finish: bool,
    pub clap: bool,
}

impl fmt::Display for HitSound {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            bitflags([
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

#[derive(Clone)]
pub struct HitSample {
    pub normal_set: SampleIndex,
    pub addition_set: SampleIndex,
    pub index: SampleIndex,
    pub volume: u8,
    pub filename: String,
}

impl fmt::Display for HitSample {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}:{}",
            self.normal_set, self.addition_set, self.index, self.volume, self.filename
        )
    }
}

// Three bit integer
pub type U3 = u8;

#[derive(Clone)]
pub enum HitObject {
    HitCircle {
        x: OsuPixel,
        y: OsuPixel,
        time: Time,
        hit_sound: HitSound,
        new_combo: bool,
        skip_combo_colours: U3,
        hit_sample: HitSample,
    },
    Slider {
        x: OsuPixel,
        y: OsuPixel,
        time: Time,
        hit_sound: HitSound,
        new_combo: bool,
        skip_combo_colours: U3,
        curve_type: char,
        curve_points: Vec<(DecimalOsuPixel, DecimalOsuPixel)>,
        slides: u8,
        length: DecimalOsuPixel,
        edge_sounds: Vec<SampleIndex>,
        edge_sets: Vec<(SampleSet, SampleSet)>,
        hit_sample: HitSample,
    },
    Spinner {
        time: Time,
        hit_sound: HitSound,
        new_combo: bool,
        skip_combo_colours: U3,
        end_time: Time,
        hit_sample: HitSample,
    },
    Hold {
        column: u8,
        columns: u8,
        time: Time,
        hit_sound: HitSound,
        new_combo: bool,
        skip_combo_colours: U3,
        end_time: Time,
        hit_sample: HitSample,
    },
}

impl fmt::Display for HitObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            HitObject::HitCircle {
                x,
                y,
                time,
                hit_sound,
                new_combo,
                skip_combo_colours,
                hit_sample,
            } => write!(
                f,
                "{},{},{},{},{},{}",
                x,
                y,
                time,
                assemble_hit_object_type(0, *new_combo, *skip_combo_colours),
                hit_sound,
                hit_sample
            ),
            HitObject::Slider {
                x,
                y,
                time,
                hit_sound,
                new_combo,
                skip_combo_colours,
                curve_type,
                curve_points,
                slides,
                length,
                edge_sounds,
                edge_sets,
                hit_sample,
            } => write!(
                f,
                "{},{},{},{},{},{}|{},{},{},{},{},{}",
                x,
                y,
                time,
                assemble_hit_object_type(1, *new_combo, *skip_combo_colours),
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
            HitObject::Spinner {
                time,
                hit_sound,
                new_combo,
                skip_combo_colours,
                end_time,
                hit_sample,
            } => write!(
                f,
                "256,192,{},{},{},{},{}",
                time,
                assemble_hit_object_type(3, *new_combo, *skip_combo_colours),
                hit_sound,
                end_time,
                hit_sample
            ),
            HitObject::Hold {
                column,
                columns,
                time,
                hit_sound,
                new_combo,
                skip_combo_colours,
                end_time,
                hit_sample,
            } => write!(
                f,
                "{},192,{},{},{},{}:{}",
                column_to_x(*column, *columns),
                time,
                assemble_hit_object_type(7, *new_combo, *skip_combo_colours),
                hit_sound,
                end_time,
                hit_sample
            ),
        }
    }
}

#[derive(Clone, Deref, DerefMut)]
pub struct HitObjects(pub Vec<HitObject>);

impl fmt::Display for HitObjects {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

pub struct Beatmap {
    pub version: u8,
    pub general: General,
    pub editor: Editor,
    pub metadata: Metadata,
    pub difficulty: Difficulty,
    pub events: Events,
    pub timing_points: TimingPoints,
    pub colours: Colours,
    pub hit_objects: HitObjects,
}

impl fmt::Display for Beatmap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "osu file format v{}\n\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
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
