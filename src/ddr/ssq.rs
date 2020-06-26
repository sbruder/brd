use std::convert::From;
use std::convert::TryInto;
use std::fmt;
use std::io;
use std::io::prelude::*;
use std::io::Cursor;

use byteorder::{ReadBytesExt, LE};
use log::{debug, info, trace, warn};
use thiserror::Error;

use crate::mini_parser::{MiniParser, MiniParserError};
use crate::utils;

const MEASURE_LENGTH: i32 = 4096;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Not enough freeze data was found")]
    NotEnoughFreezeData,
    #[error(transparent)]
    IOError(#[from] io::Error),
    #[error(transparent)]
    TryFromIntError(#[from] std::num::TryFromIntError),
    #[error(transparent)]
    MiniParserError(#[from] MiniParserError),
}

/// Convert time offset to beats
/// time offset is the measure times MEASURE_LENGTH
fn measure_to_beats(metric: i32) -> f32 {
    4.0 * metric as f32 / MEASURE_LENGTH as f32
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

impl fmt::Display for PlayerRow {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}{}{}{}",
            if self.left { "←" } else { " " },
            if self.down { "↓" } else { " " },
            if self.up { "↑" } else { " " },
            if self.right { "→" } else { " " },
        )
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

impl fmt::Display for Row {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let player_rows = match self {
            Self::Single(player_row) => vec![player_row],
            Self::Double(player_row1, player_row2) => vec![player_row1, player_row2],
        };
        write!(f, "{}", utils::join_display_values(player_rows, " "))
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
    fn parse(ticks_per_second: i32, data: &[u8]) -> Result<Self, Error> {
        let mut cursor = Cursor::new(data);

        let count = cursor.read_u32::<LE>()?.try_into()?;
        let measure = cursor.read_n_u32(count)?;
        let tempo_data = cursor.read_n_u32(count)?;

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

        Ok(Self(entries))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Step { beats: f32, row: Row },
    Freeze { start: f32, end: f32, row: Row },
    Shock { beats: f32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chart {
    pub difficulty: Difficulty,
    pub steps: Vec<Step>,
}

impl Chart {
    fn parse(data: &[u8], parameter: u16) -> Result<Self, Error> {
        let difficulty: Difficulty = parameter.into();

        let mut cursor = Cursor::new(data);

        let count = cursor.read_u32::<LE>()?.try_into()?;
        let measures = cursor.read_n_u32(count)?;
        let mut steps = vec![0; count];
        cursor.read_exact(&mut steps)?;

        let mut freeze_data = Vec::new();
        cursor.read_to_end(&mut freeze_data)?;
        // freeze data can be padded with zeroes
        let mut freeze_data = freeze_data.iter().skip_while(|x| **x == 0).copied();

        let mut parsed_steps = Vec::new();

        // indices of (normal) steps that start a freeze (they are not needed after processing all
        // steps as they are already included in the freezes)
        let mut freeze_steps = Vec::new();

        for step in 0..count {
            let beats = measure_to_beats(measures[step]);

            // check if either all eight bits are set (shock for double) or the first four (shock for
            // single)
            if steps[step] == 0xff || steps[step] == 0xf {
                // shock
                trace!("Shock arrow at {}", beats);

                parsed_steps.push(Step::Shock { beats });
            } else if steps[step] == 0x00 {
                // extra data
                let columns = freeze_data.next().ok_or(Error::NotEnoughFreezeData)?;
                let extra_type = freeze_data.next().ok_or(Error::NotEnoughFreezeData)?;

                if extra_type == 1 {
                    // freeze end (start is the last normal step in that column)
                    trace!("Freeze arrow at {}", beats);

                    let row = Row::new(columns, difficulty.players);
                    if row.count_active() != 1 {
                        warn!("Found freeze with not exactly one column, which is not implemented, skipping");
                        continue;
                    }

                    match Self::find_last(parsed_steps.clone(), &row) {
                        Some(last_step) => {
                            parsed_steps.push(Step::Freeze {
                                start: if let Step::Step { beats, .. } = parsed_steps[last_step] {
                                    beats
                                } else {
                                    unreachable!()
                                },
                                end: beats,
                                row,
                            });

                            freeze_steps.push(last_step);
                        }
                        None => {
                            warn!("Could not find previous step for freeze, adding normal step");
                            parsed_steps.push(Step::Step { beats, row });
                        }
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
                    row: Row::new(steps[step], difficulty.players),
                });
            }
        }

        // remove steps that start a freeze
        freeze_steps.dedup();
        for i in freeze_steps.iter().rev() {
            parsed_steps.remove(*i);
        }

        debug!("Parsed {} steps", parsed_steps.len());

        Ok(Self {
            difficulty,
            steps: parsed_steps,
        })
    }

    fn find_last(steps: Vec<Step>, row: &Row) -> Option<usize> {
        for i in (0..steps.len()).rev() {
            if let Step::Step { row: step_row, .. } = &steps[i] {
                if step_row.clone().intersects(row.clone()) {
                    return Some(i);
                }
            }
        }

        None
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
pub struct SSQ {
    pub tempo_changes: TempoChanges,
    pub charts: Vec<Chart>,
}

impl SSQ {
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        let mut cursor = Cursor::new(data);

        let mut ssq = Self {
            tempo_changes: TempoChanges(Vec::new()),
            charts: Vec::new(),
        };

        loop {
            let length = cursor.read_i32::<LE>()? as usize;
            trace!("Found chunk (length {})", length);
            if length == 0 {
                break;
            }

            let chunk_type = cursor.read_u16::<LE>()?;
            let parameter = cursor.read_u16::<LE>()?;

            // length without i32 and 2 × i16
            let mut data = vec![0; length - 8];
            cursor.read_exact(&mut data)?;

            match chunk_type {
                1 => {
                    debug!("Parsing tempo changes (ticks/s: {})", parameter);
                    ssq.tempo_changes = TempoChanges::parse(parameter as i32, &data)?;
                }
                3 => {
                    debug!("Parsing step chunk ({})", Difficulty::from(parameter));
                    ssq.charts.push(Chart::parse(&data, parameter)?)
                }
                _ => {
                    debug!(
                        "Found extra chunk (type {}, length {})",
                        chunk_type,
                        data.len()
                    );
                }
            };
        }

        info!("Parsed {} charts", ssq.charts.len());

        Ok(ssq)
    }
}
