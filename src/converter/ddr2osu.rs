use std::fmt;
use std::str::FromStr;

use anyhow::{anyhow, Result};
use clap::Clap;
use log::{debug, info, trace, warn};

use crate::ddr::ssq;
use crate::osu::beatmap;

#[derive(Clone, Debug)]
pub struct ConfigRange(f32, f32);

impl ConfigRange {
    /// Map value from 0 to 1 onto the range
    fn map_from(&self, value: f32) -> f32 {
        (value * (self.1 - self.0)) + self.0
    }
}

impl fmt::Display for ConfigRange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.0, self.1)
    }
}

impl FromStr for ConfigRange {
    type Err = anyhow::Error;

    fn from_str(string: &str) -> Result<Self> {
        match string.split(':').collect::<Vec<&str>>()[..] {
            [start, end] => Ok(ConfigRange(start.parse::<f32>()?, end.parse::<f32>()?)),
            _ => Err(anyhow!("Invalid range format (expected start:end)")),
        }
    }
}

#[derive(Debug, Clap, Clone)]
pub struct Config {
    #[clap(skip = "audio.wav")]
    pub audio_filename: String,
    #[clap(
        long = "no-stops",
        about = "Disable stops",
        parse(from_flag = std::ops::Not::not),
        display_order = 3
    )]
    pub stops: bool,
    #[clap(
        arg_enum,
        long,
        default_value = "step",
        about = "What to do with shocks",
        display_order = 3
    )]
    pub shock_action: ShockAction,
    #[clap(
        long = "hp",
        about = "Range of HP drain (beginner:challenge)",
        default_value = "2:4"
    )]
    pub hp_drain: ConfigRange,
    #[clap(
        long = "acc",
        about = "Range of Accuracy (beginner:challenge)",
        default_value = "7:8"
    )]
    pub accuracy: ConfigRange,
    #[clap(flatten)]
    pub metadata: ConfigMetadata,
}

#[derive(Clap, Debug, Clone)]
pub struct ConfigMetadata {
    #[clap(long, about = "Song title to use in beatmap", display_order = 4)]
    pub title: Option<String>,
    #[clap(long, about = "Artist name to use in beatmap", display_order = 4)]
    pub artist: Option<String>,
    #[clap(
        long,
        default_value = "Dance Dance Revolution",
        about = "Source to use in beatmap",
        display_order = 4
    )]
    pub source: String,
    #[clap(skip)]
    pub levels: Option<Vec<u8>>,
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ddr2osu ({}shock→{:?} hp{} acc{})",
            if self.stops { "stops " } else { "" },
            self.shock_action,
            self.hp_drain,
            self.accuracy
        )
    }
}

#[derive(Clap, Clone, Debug)]
pub enum ShockAction {
    Ignore,
    Step,
    //Static(Vec<u8>),
}

struct ShockStepGenerator {
    last: u8,
    columns: u8,
    mode: ShockAction,
}

impl Iterator for ShockStepGenerator {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Vec<u8>> {
        match &self.mode {
            ShockAction::Ignore => None,
            ShockAction::Step => {
                let columns = match self.last {
                    0 | 3 => vec![0, 3],
                    1 | 2 => vec![1, 2],
                    4 | 7 => vec![4, 7],
                    5 | 6 => vec![5, 6],
                    _ => vec![],
                };
                self.last = (self.last + 1) % self.columns;
                Some(columns)
            } //ShockAction::Static(columns) => Some(columns.clone()),
        }
    }
}

impl ShockStepGenerator {
    fn new(columns: u8, mode: ShockAction) -> Self {
        Self {
            last: 0,
            columns,
            mode,
        }
    }
}

fn get_time_from_beats(beats: f32, tempo_changes: &ssq::TempoChanges) -> Option<beatmap::Time> {
    for tempo_change in tempo_changes.to_vec() {
        // For TempoChanges that are infinitely short but exactly cover that beat, use the start
        // time of that TempoChange
        if (beats - tempo_change.start_beats).abs() < 0.001
            && (beats - tempo_change.end_beats).abs() < 0.001
        {
            return Some(tempo_change.start_ms);
        }

        if beats < tempo_change.end_beats {
            return Some(
                tempo_change.start_ms
                    + ((beats - tempo_change.start_beats) * tempo_change.beat_length) as u32,
            );
        }
    }

    None
}

impl From<ssq::TempoChange> for beatmap::TimingPoint {
    fn from(tempo_change: ssq::TempoChange) -> Self {
        beatmap::TimingPoint {
            time: tempo_change.start_ms,
            beat_length: if tempo_change.beat_length == f32::INFINITY {
                10000.0
            } else {
                tempo_change.beat_length
            },
            meter: 4,
            sample_set: beatmap::SampleSet::BeatmapDefault,
            sample_index: 0,
            volume: 100,
            uninherited: true,
            effects: beatmap::TimingPointEffects {
                kiai_time: false,
                omit_first_barline: false,
            },
        }
    }
}

impl ssq::Step {
    fn to_hit_objects(
        &self,
        num_columns: u8,
        tempo_changes: &ssq::TempoChanges,
        shock_step_generator: &mut ShockStepGenerator,
    ) -> Option<Vec<beatmap::HitObject>> {
        let mut hit_objects = Vec::new();

        match self {
            ssq::Step::Step { beats, row } => {
                let time = get_time_from_beats(*beats, tempo_changes);

                match time {
                    Some(time) => {
                        let columns: Vec<bool> = row.clone().into();

                        for (column, active) in columns.iter().enumerate() {
                            if *active {
                                hit_objects.push(beatmap::HitObject::HitCircle {
                                    x: beatmap::column_to_x(column as u8, num_columns),
                                    y: 192,
                                    time,
                                    hit_sound: beatmap::HitSound {
                                        normal: true,
                                        whistle: false,
                                        finish: false,
                                        clap: false,
                                    },
                                    new_combo: false,
                                    skip_combo_colours: 0,
                                    hit_sample: beatmap::HitSample {
                                        normal_set: 0,
                                        addition_set: 0,
                                        index: 0,
                                        volume: 0,
                                        filename: "".to_string(),
                                    },
                                })
                            }
                        }
                    }
                    None => {
                        warn!("Could not get start time of step, skipping");
                        return None;
                    }
                }
            }
            ssq::Step::Freeze { start, end, row } => {
                let time = get_time_from_beats(*start, tempo_changes);
                let end_time = get_time_from_beats(*end, tempo_changes);

                match (time, end_time) {
                    (Some(time), Some(end_time)) => {
                        let columns: Vec<bool> = row.clone().into();

                        for (column, active) in columns.iter().enumerate() {
                            if *active {
                                hit_objects.push(beatmap::HitObject::Hold {
                                    column: column as u8,
                                    columns: num_columns,
                                    time,
                                    end_time,
                                    hit_sound: beatmap::HitSound {
                                        normal: true,
                                        whistle: false,
                                        finish: false,
                                        clap: false,
                                    },
                                    new_combo: false,
                                    skip_combo_colours: 0,
                                    hit_sample: beatmap::HitSample {
                                        normal_set: 0,
                                        addition_set: 0,
                                        index: 0,
                                        volume: 0,
                                        filename: "".to_string(),
                                    },
                                })
                            }
                        }
                    }
                    (None, Some(_)) => {
                        warn!("Could not get start time of freeze, skipping");
                        return None;
                    }
                    (Some(_), None) => {
                        warn!("Could not get end time of freeze, skipping");
                        return None;
                    }
                    (None, None) => {
                        warn!("Could not get start and end time of freeze, skipping");
                        return None;
                    }
                }
            }
            ssq::Step::Shock { beats } => {
                let columns = shock_step_generator.next().unwrap_or_else(Vec::new);

                for column in columns {
                    hit_objects.push(beatmap::HitObject::HitCircle {
                        x: beatmap::column_to_x(column as u8, num_columns),
                        y: 192,
                        time: get_time_from_beats(*beats, tempo_changes)?,
                        hit_sound: beatmap::HitSound {
                            normal: true,
                            whistle: false,
                            finish: false,
                            clap: false,
                        },
                        new_combo: false,
                        skip_combo_colours: 0,
                        hit_sample: beatmap::HitSample {
                            normal_set: 0,
                            addition_set: 0,
                            index: 0,
                            volume: 0,
                            filename: "".to_string(),
                        },
                    })
                }
            }
        }

        Some(hit_objects)
    }
}

struct ConvertedChart {
    difficulty: ssq::Difficulty,
    hit_objects: beatmap::HitObjects,
    timing_points: beatmap::TimingPoints,
}

impl ConvertedChart {
    fn to_beatmap(&self, config: &Config) -> beatmap::Beatmap {
        beatmap::Beatmap {
            version: 14,
            general: beatmap::General {
                audio_filename: config.audio_filename.clone(),
                audio_lead_in: 0,
                preview_time: 0,
                countdown: beatmap::Countdown::No,
                sample_set: beatmap::SampleSet::Soft,
                mode: beatmap::Mode::Mania,
            },
            editor: beatmap::Editor {},
            metadata: beatmap::Metadata {
                title: config
                    .metadata
                    .title
                    .as_ref()
                    .unwrap_or(&"unknown title".to_string())
                    .clone(),
                artist: config
                    .metadata
                    .artist
                    .as_ref()
                    .unwrap_or(&"unknown artist".to_string())
                    .clone(),
                creator: format!("{}", config),
                version: match &config.metadata.levels {
                    Some(levels) => {
                        let level = self.difficulty.to_level(levels);
                        format!("{} (Lv. {})", self.difficulty, level)
                    }
                    None => format!("{}", self.difficulty),
                },
                source: config.metadata.source.clone(),
                tags: vec![],
            },
            difficulty: beatmap::Difficulty {
                hp_drain_rate: config.hp_drain.map_from(self.difficulty.clone().into()),
                circle_size: f32::from(self.difficulty.players) * 4.0,
                overall_difficulty: config.accuracy.map_from(self.difficulty.clone().into()),
                approach_rate: 8.0,
                slider_multiplier: 0.64,
                slider_tick_rate: 1.0,
            },
            events: beatmap::Events(vec![]),
            timing_points: self.timing_points.clone(),
            colours: beatmap::Colours(vec![]),
            hit_objects: self.hit_objects.clone(),
        }
    }
}

impl ssq::SSQ {
    pub fn to_beatmaps(&self, config: &Config) -> Result<Vec<beatmap::Beatmap>> {
        debug!("Configuration: {:?}", config);

        let mut timing_points = beatmap::TimingPoints(Vec::new());

        for entry in self.tempo_changes.to_vec() {
            if config.stops || entry.beat_length != f32::INFINITY {
                trace!("Converting {:?} to to timing point", entry);
                timing_points.push(entry.into());
            }
        }
        debug!(
            "Converted {} tempo changes to timing points",
            self.tempo_changes.len()
        );

        let mut converted_charts = Vec::new();

        for chart in &self.charts {
            debug!("Converting chart {} to beatmap", chart.difficulty);
            let mut hit_objects = beatmap::HitObjects(Vec::new());

            let mut shock_step_generator =
                ShockStepGenerator::new(chart.difficulty.players * 4, config.shock_action.clone());
            for step in &chart.steps {
                trace!("Converting {:?} to hit object", step);
                if let Some(mut step_hit_objects) = step.to_hit_objects(
                    chart.difficulty.players * 4,
                    &self.tempo_changes,
                    &mut shock_step_generator,
                ) {
                    hit_objects.append(&mut step_hit_objects);
                }
            }

            let converted_chart = ConvertedChart {
                difficulty: chart.difficulty.clone(),
                hit_objects,
                timing_points: timing_points.clone(),
            };

            debug!(
                "Converted to beatmap with {} hit objects",
                converted_chart.hit_objects.len(),
            );

            converted_charts.push(converted_chart);
        }

        let mut beatmaps = Vec::new();

        for converted_chart in converted_charts {
            let beatmap = converted_chart.to_beatmap(config);
            beatmaps.push(beatmap);
        }

        info!("Converted {} step charts to beatmaps", beatmaps.len());

        Ok(beatmaps)
    }
}
