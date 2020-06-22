use std::convert::From;
use std::fmt;

use anyhow::{anyhow, Result};
use log::{debug, info, trace, warn};
use nom::bytes::complete::take;
use nom::multi::many0;
use nom::number::complete::{le_i16, le_i32, le_u16};
use nom::IResult;

use crate::utils;
use crate::utils::exec_nom_parser;

const MEASURE_LENGTH: i32 = 4096;
const FREEZE: bool = false;

// Convert time offset to beats
// time offset is the measure times MEASURE_LENGTH
fn measure_to_beats(metric: i32) -> f32 {
    4.0 * metric as f32 / MEASURE_LENGTH as f32
}

fn parse_n_i32(n: usize, input: &[u8]) -> IResult<&[u8], Vec<i32>> {
    let (input, bytes) = take(n as usize * 4)(input)?;
    let (unprocessed_input, values) = many0(le_i32)(bytes)?;
    assert_eq!(unprocessed_input.len(), 0);
    Ok((input, values))
}

fn parse_usize(input: &[u8]) -> IResult<&[u8], usize> {
    let (input, value) = le_i32(input)?;
    Ok((input, value as usize))
}

#[derive(Clone, Debug, PartialEq)]
pub struct TempoChange {
    pub start_ms: i32,
    pub start_beats: f32,
    pub end_beats: f32,
    pub beat_length: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TempoChanges(pub Vec<TempoChange>);

impl TempoChanges {
    fn parse(ticks_per_second: i32, input: &[u8]) -> IResult<&[u8], Self> {
        let (input, count) = parse_usize(input)?;
        let (input, measure) = parse_n_i32(count, input)?;
        let (input, tempo_data) = parse_n_i32(count, input)?;

        let mut entries = Vec::new();

        let mut elapsed_ms = 0;
        let mut elapsed_beats = 0.0;
        for i in 1..count {
            let delta_measure = measure[i] - measure[i - 1];
            let delta_ticks = tempo_data[i] - tempo_data[i - 1];

            let length_ms = 1000 * delta_ticks / ticks_per_second;
            let length_beats = measure_to_beats(delta_measure);

            let beat_length = length_ms as f32 / length_beats;

            let entry = TempoChange {
                start_ms: elapsed_ms,
                start_beats: elapsed_beats,
                end_beats: elapsed_beats + length_beats,
                beat_length,
            };

            entries.push(entry);

            elapsed_ms += length_ms;
            elapsed_beats += length_beats;
        }

        Ok((input, Self(entries)))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Step { beats: f32, row: Row },
    Freeze { start: f32, end: f32, row: Row },
    Shock { beats: f32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Steps(pub Vec<Step>);

impl Steps {
    fn parse(input: &[u8], players: u8) -> IResult<&[u8], Self> {
        let (input, count) = parse_usize(input)?;
        let (input, measure) = parse_n_i32(count, input)?;
        let (input, steps) = take(count)(input)?;

        // freeze data can be padded with zeroes
        let (input, freeze_data) = take(input.len())(input)?;
        let mut freeze = freeze_data.iter().skip_while(|x| **x == 0).copied();

        let mut parsed_steps = Vec::new();

        for i in 0..count {
            let beats = measure_to_beats(measure[i]);

            // check if either all eight bits are set (shock for double) or the first four (shock for
            // single)
            if steps[i] == 0xff || steps[i] == 0xf {
                // shock
                trace!("Shock arrow at {}", beats);

                parsed_steps.push(Step::Shock { beats });
            } else if steps[i] == 0x00 {
                // extra data
                let columns = freeze.next().unwrap();
                let extra_type = freeze.next().unwrap();

                if extra_type == 1 {
                    // freeze end (start is the last normal step in that column)
                    trace!("Freeze arrow at {}", beats);

                    let row = Row::new(columns, players);
                    if row.count_active() != 1 {
                        warn!("Found freeze with not exactly one column, which is not implemented, skipping");
                        continue;
                    }

                    let last_step = match Self::find_last(Self(parsed_steps.clone()), &row) {
                        Ok(last_step) => last_step,
                        Err(err) => {
                            warn!("Could not add freeze arrow: {}; adding normal step", err);
                            parsed_steps.push(Step::Step { beats, row });
                            continue;
                        }
                    };

                    if FREEZE {
                        parsed_steps.push(Step::Freeze {
                            start: if let Step::Step { beats, .. } = parsed_steps[last_step] {
                                beats
                            } else {
                                unreachable!()
                            },
                            end: beats,
                            row,
                        });

                        parsed_steps.remove(last_step);
                    } else {
                        trace!("Freeze disabled, adding normal step");
                        parsed_steps.push(Step::Step { beats, row });
                    }
                } else {
                    debug!(
                        "Encountered unknown extra step with type {}, ignoring",
                        extra_type
                    );
                }
            } else {
                // normal step
                trace!("Normal step at {}", beats);

                parsed_steps.push(Step::Step {
                    beats,
                    row: Row::new(steps[i], players),
                });
            }
        }

        debug!("Parsed {} steps", parsed_steps.len());

        Ok((input, Self(parsed_steps)))
    }

    fn find_last(steps: Self, row: &Row) -> Result<usize> {
        for i in (0..steps.0.len()).rev() {
            if let Step::Step { row: step_row, .. } = &steps.0[i] {
                if step_row.clone().intersects(row.clone()) {
                    return Ok(i);
                }
            }
        }

        Err(anyhow!("No previous step found on that column"))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Difficulty {
    pub players: u8,
    difficulty: u8,
}

impl From<u16> for Difficulty {
    fn from(parameter: u16) -> Self {
        Self {
            difficulty: ((parameter & 0xFF00) >> 8) as u8,
            players: (parameter & 0xF) as u8 / 4,
        }
    }
}

impl Into<f32> for Difficulty {
    fn into(self) -> f32 {
        match self.difficulty {
            1 => 0.25,
            2 => 0.5,
            3 => 1.0,
            4 => 0.0,
            6 => 0.75,
            _ => 1.0,
        }
    }
}

impl fmt::Display for Difficulty {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let players = match self.players {
            1 => "Single",
            2 => "Double",
            _ => "Unknown Number of Players",
        };
        let difficulty = match self.difficulty {
            1 => "Basic",
            2 => "Difficult",
            3 => "Challenge",
            4 => "Beginner",
            6 => "Expert",
            _ => "Unknown Difficulty",
        };
        write!(f, "{} {}", players, difficulty)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chart {
    pub difficulty: Difficulty,
    pub steps: Steps,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SSQ {
    pub tempo_changes: TempoChanges,
    pub charts: Vec<Chart>,
}

impl From<Chunks> for SSQ {
    fn from(chunks: Chunks) -> Self {
        let mut ssq = Self {
            tempo_changes: TempoChanges(Vec::new()),
            charts: Vec::new(),
        };
        for chunk in chunks.0 {
            match chunk {
                Chunk::TempoChanges(mut tempo_changes) => {
                    ssq.tempo_changes.0.append(&mut tempo_changes.0)
                }
                Chunk::Chart(chart) => ssq.charts.push(chart),
                Chunk::Extra(..) => {}
            }
        }
        info!("Parsed {} charts", ssq.charts.len());
        ssq
    }
}

impl SSQ {
    pub fn parse(data: &[u8]) -> Result<Self> {
        debug!(
            "Configuration: measure length: {}, use freezes: {}",
            MEASURE_LENGTH, FREEZE
        );
        let chunks = exec_nom_parser(Chunks::parse, data)?;

        Ok(Self::from(chunks))
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Chunk {
    Chart(Chart),
    TempoChanges(TempoChanges),
    Extra(Vec<u8>),
}

impl Chunk {
    fn parse(input: &[u8]) -> IResult<&[u8], Self> {
        let (input, length) = le_i32(input)?;

        let (input, chunk_type) = le_i16(input)?;
        let (input, parameter) = le_u16(input)?;

        // length without i32 and 2 × i16
        let (input, data) = take(length as usize - 8)(input)?;

        let chunk = match chunk_type {
            1 => {
                debug!("Parsing tempo changes (ticks/s: {})", parameter);
                let (_, TempoChanges(tempo_changes)) = TempoChanges::parse(parameter as i32, data)?;
                Self::TempoChanges(TempoChanges(tempo_changes))
            }
            3 => {
                let difficulty = Difficulty::from(parameter);
                debug!("Parsing step chunk ({})", difficulty);
                let (_, steps) = Steps::parse(data, difficulty.players)?;
                Self::Chart(Chart { difficulty, steps })
            }
            _ => {
                debug!("Found extra chunk (length {})", data.len());
                Self::Extra(data.to_vec())
            }
        };
        Ok((input, chunk))
    }
}

pub struct Chunks(Vec<Chunk>);

impl Chunks {
    fn parse(input: &[u8]) -> IResult<&[u8], Self> {
        let (input, chunks) = many0(Chunk::parse)(input)?;
        Ok((input, Self(chunks)))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerRow {
    pub left: bool,
    pub down: bool,
    pub up: bool,
    pub right: bool,
}

impl From<u8> for PlayerRow {
    fn from(byte: u8) -> Self {
        let columns = utils::byte_to_bitarray(byte);
        PlayerRow {
            left: columns[0],
            down: columns[1],
            up: columns[2],
            right: columns[3],
        }
    }
}

impl Into<Vec<bool>> for PlayerRow {
    fn into(self) -> Vec<bool> {
        vec![self.left, self.down, self.up, self.right]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    Single(PlayerRow),
    Double(PlayerRow, PlayerRow),
}

impl Into<Vec<bool>> for Row {
    fn into(self) -> Vec<bool> {
        match self {
            Self::Single(row) => row.into(),
            Self::Double(row1, row2) => {
                let mut row: Vec<bool> = Vec::new();
                row.append(&mut row1.into());
                row.append(&mut row2.into());
                row
            }
        }
    }
}

impl Row {
    fn new(byte: u8, players: u8) -> Self {
        match players {
            1 => Self::Single(PlayerRow::from(byte)),
            2 => Self::Double(PlayerRow::from(byte), PlayerRow::from(byte >> 4)),
            _ => unreachable!(),
        }
    }

    fn count_active(&self) -> u8 {
        let mut rows = Vec::<bool>::new();

        match self {
            Self::Single(row) => {
                rows.append(&mut row.clone().into());
            }
            Self::Double(player1, player2) => {
                rows.append(&mut player1.clone().into());
                rows.append(&mut player2.clone().into());
            }
        }

        rows.iter().map(|x| *x as u8).sum()
    }

    fn intersects(self, other: Self) -> bool {
        let rows: Vec<(Vec<bool>, Vec<bool>)> = match (self, other) {
            (Self::Single(self_row), Self::Single(other_row)) => {
                vec![(self_row.into(), other_row.into())]
            }
            (Self::Double(self_row1, self_row2), Self::Double(other_row1, other_row2)) => vec![
                (self_row1.into(), other_row1.into()),
                (self_row2.into(), other_row2.into()),
            ],
            _ => vec![],
        };

        for (self_row, other_row) in rows {
            for (self_col, other_col) in self_row.iter().zip(other_row.iter()) {
                if *self_col && self_col == other_col {
                    return true;
                }
            }
        }

        false
    }
}
