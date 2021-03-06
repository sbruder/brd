use std::convert::From;
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::io;
use std::io::prelude::*;
use std::io::Cursor;
use std::num;

use byteorder::{ReadBytesExt, LE};
use derive_more::Deref;
use log::{debug, info, trace, warn};
use thiserror::Error;

use crate::mini_parser::MiniParser;
use crate::utils;

const MEASURE_LENGTH: f32 = 4096.0;

#[derive(Error, Debug)]
pub enum Error {
    #[error("not enough freeze data was found")]
    NotEnoughFreezeData,
    #[error("invalid player count {0} (valid options: 1, 2)")]
    InvalidPlayerCount(u8),
    #[error("invalid difficulty {0} (valid options: 4, 1, 2, 3, 6)")]
    InvalidDifficulty(u8),
    #[error(transparent)]
    IOError(#[from] io::Error),
    #[error(transparent)]
    TryFromIntError(#[from] num::TryFromIntError),
    #[error(transparent)]
    MiniParserError(#[from] crate::mini_parser::Error),
}

/// Convert time offset to beats
/// time offset is the measure times MEASURE_LENGTH
fn measure_to_beats(measure: u32) -> f32 {
    4.0 * measure as f32 / MEASURE_LENGTH
}

#[derive(Debug, Clone, PartialEq, Default)]
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let player_rows = match self {
            Self::Single(player_row) => vec![player_row],
            Self::Double(player_row1, player_row2) => vec![player_row1, player_row2],
        };
        write!(f, "{}", utils::join_display_values(player_rows, " "))
    }
}

impl Row {
    fn new(byte: u8, players: u8) -> Result<Self, Error> {
        match players {
            1 => Ok(Self::Single(PlayerRow::from(byte))),
            2 => Ok(Self::Double(
                PlayerRow::from(byte),
                PlayerRow::from(byte >> 4),
            )),
            _ => Err(Error::InvalidPlayerCount(players)),
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
            // rows with different player count can’t intersect
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
    pub start_ms: u32,
    pub start_beats: f32,
    pub end_beats: f32,
    pub beat_length: f32,
}

#[derive(Clone, Debug, Deref, PartialEq)]
pub struct TempoChanges(pub Vec<TempoChange>);

impl TempoChanges {
    fn parse(ticks_per_second: u32, data: &[u8]) -> Result<Self, Error> {
        let mut cursor = Cursor::new(data);

        let count = cursor.read_u32::<LE>()?.try_into()?;
        let measure = cursor.read_n_i32(count)?;
        let tempo_data = cursor.read_n_i32(count)?;

        let mut entries = Vec::new();

        let mut elapsed_ms = 0;
        let mut elapsed_beats = 0.0;
        for i in 1..count {
            let delta_measure: u32 = (measure[i] - measure[i - 1]).abs().try_into()?;
            let delta_ticks: u32 = (tempo_data[i] - tempo_data[i - 1]).abs().try_into()?;

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
    pub difficulty: Level,
    pub steps: Vec<Step>,
}

impl Chart {
    fn parse(data: &[u8], parameter: u16) -> Result<Self, Error> {
        let difficulty: Level = parameter.try_into()?;

        let mut cursor = Cursor::new(data);

        let count = cursor.read_u32::<LE>()?.try_into()?;
        let measures = cursor.read_n_i32(count)?;
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
            let beats = measure_to_beats(measures[step].try_into()?);

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

                    let row = Row::new(columns, difficulty.players)?;
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
                    row: Row::new(steps[step], difficulty.players)?,
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

/// Metadata about a level.
///
/// It does not store the level visible to the user. However it can – when povided with
/// [`ddr::musicdb::Entry.diff_lv`] – return the user visible level with the [`to_value`] method.
///
/// [`ddr::musicdb::Entry.diff_lv`]: ../musicdb/struct.Entry.html#structfield.diff_lv
/// [`to_value`]: #method.to_value
#[derive(Clone, Debug, PartialEq)]
pub struct Level {
    pub players: u8,
    pub difficulty: u8,
}

impl Level {
    /// Creates a new instance and maps the SSQ difficulty to ordered difficulty.
    ///
    /// # Errors
    ///
    /// This checks if the players and difficulty and valid, otherwise it returns
    /// [`InvalidPlayerCount`] or [`InvalidDifficulty`].
    ///
    /// [`InvalidPlayerCount`]: enum.Error.html#variant.InvalidPlayerCount
    /// [`InvalidDifficulty`]: enum.Error.html#variant.InvalidDifficulty
    pub fn new(players: u8, ssq_difficulty: u8) -> Result<Self, Error> {
        if ![1, 2].contains(&players) {
            return Err(Error::InvalidPlayerCount(players));
        }
        let difficulty = Self::ssq_to_ordered(&ssq_difficulty)?;
        Ok(Level {
            players,
            difficulty,
        })
    }

    /// The SSQ file stores the difficulty in a format, where the lower numeric value does not mean
    /// the easier level. This function maps the values from the SSQ file to ordered values (lower
    /// values → easier, higher values → harder)
    ///
    /// # Errors
    ///
    /// This checks if the difficulty is valid, otherwise it returns [`InvalidDifficulty`].
    ///
    /// [`InvalidDifficulty`]: enum.Error.html#variant.InvalidDifficulty
    fn ssq_to_ordered(ssq_difficulty: &u8) -> Result<u8, Error> {
        Ok(match ssq_difficulty {
            4 => 0,
            1 => 1,
            2 => 2,
            3 => 3,
            6 => 4,
            _ => return Err(Error::InvalidDifficulty(*ssq_difficulty)),
        })
    }

    /// Returns the difficulty as `f32` where 0.0 is the easiest and 1.0 is the hardest.
    pub fn relative_difficulty(&self) -> f32 {
        f32::from(self.difficulty) / 4.0
    }

    /// Returns the user visible level value for difficulty from [`ddr::musicdb::Entry.diff_lv`].
    ///
    /// [`ddr::musicdb::Entry.diff_lv`]: ../musicdb/struct.Entry.html#structfield.diff_lv
    pub fn to_value(&self, levels: &[u8]) -> u8 {
        let index: usize = (self.difficulty + (self.players - 1) * 5).into();

        levels[index]
    }
}

impl TryFrom<u16> for Level {
    type Error = Error;

    fn try_from(parameter: u16) -> Result<Self, Error> {
        let players = (parameter & 0xF) as u8 / 4;
        let difficulty = ((parameter & 0xFF00) >> 8) as u8;
        Self::new(players, difficulty)
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let players = match self.players {
            1 => "Single",
            2 => "Double",
            _ => "Unknown Number of Players",
        };
        let difficulty = match self.difficulty {
            0 => "Beginner",
            1 => "Basic",
            2 => "Difficult",
            3 => "Expert",
            4 => "Challenge",
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
            let length: usize = cursor.read_i32::<LE>()?.try_into()?;
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
                    ssq.tempo_changes = TempoChanges::parse(parameter.into(), &data)?;
                }
                3 => {
                    debug!("Parsing step chunk ({})", Level::try_from(parameter)?);
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

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::TestResult;

    #[quickcheck]
    fn test_row_new(columns: u8, players: u8) -> TestResult {
        match (Row::new(columns, players), players) {
            (Ok(Row::Single(..)), 1) => TestResult::passed(),
            (Ok(Row::Double(..)), 2) => TestResult::passed(),
            (Ok(Row::Single(..)), 2) | (Ok(Row::Double(..)), 1) => TestResult::failed(),
            (row, _) => TestResult::from_bool(row.is_err()),
        }
    }

    #[test]
    fn test_row_into_vec() {
        let single: Vec<bool> = Row::new(0b0010, 1).unwrap().into();
        let double: Vec<bool> = Row::new(0b01000000, 2).unwrap().into();
        assert_eq!(single, vec![false, true, false, false]);
        assert_eq!(
            double,
            vec![false, false, false, false, false, false, true, false]
        );
    }

    #[quickcheck]
    fn test_row_intersects_itself(columns: u8, players: bool) -> bool {
        let players = u8::from(players) + 1;
        // only use first 4 bits for single player
        let columns = if players == 1 {
            columns & 0b1111
        } else {
            columns
        };
        let row = Row::new(columns, players).unwrap();
        let intersects = row.clone().intersects(row);
        // Rows don’t intersect when all columns are unset
        if columns == 0 {
            !intersects
        } else {
            intersects
        }
    }

    #[test]
    fn test_row_intersects() {
        let values = [
            (0b0010, 0b0011, 1, true),
            (0b1000, 0b1000, 1, true),
            (0b1111, 0b0100, 1, true),
            (0b0000, 0b1111, 1, false),
            (0b1001, 0b0110, 1, false),
            (0b01010101, 0b11111111, 2, true),
            (0b10000000, 0b10101010, 2, true),
            (0b00100000, 0b00100000, 2, true),
            (0b00000000, 0b11111111, 2, false),
            (0b01100000, 0b10000100, 2, false),
        ];
        for (a, b, players, intersects) in values.iter() {
            let row_a = Row::new(*a, *players).unwrap();
            let row_b = Row::new(*b, *players).unwrap();
            assert_eq!(row_a.intersects(row_b), *intersects);
        }
        assert!(!Row::new(0b1111, 1)
            .unwrap()
            .intersects(Row::new(0b1111, 2).unwrap()));
    }

    #[test]
    fn test_row_count_active() {
        let values = [
            (0b0000, 0, 1),
            (0b0010, 1, 1),
            (0b1010, 2, 1),
            (0b1111, 4, 1),
            (0b00000000, 0, 2),
            (0b00001000, 1, 2),
            (0b00000110, 2, 2),
            (0b11111111, 8, 2),
        ];
        for (data, active, players) in values.iter() {
            assert_eq!(Row::new(*data, *players).unwrap().count_active(), *active);
        }
    }

    #[test]
    fn test_row_display() {
        let values = [
            (0b0000, "    ", 1),
            (0b0010, " ↓  ", 1),
            (0b1100, "  ↑→", 1),
            (0b1111, "←↓↑→", 1),
            (0b00000000, "         ", 2),
            (0b00001000, "   →     ", 2),
            (0b00000110, " ↓↑      ", 2),
            (0b11111111, "←↓↑→ ←↓↑→", 2),
        ];
        for (data, displayed, players) in values.iter() {
            assert_eq!(
                format!("{}", Row::new(*data, *players).unwrap()),
                *displayed
            );
        }
    }

    #[test]
    fn test_player_row_parse() {
        assert_eq!(PlayerRow::from(0b11110000), PlayerRow::from(0b00000000));
        assert_eq!(
            PlayerRow::from(0b0001),
            PlayerRow {
                left: true,
                ..Default::default()
            }
        );
        assert_eq!(
            PlayerRow::from(0b0010),
            PlayerRow {
                down: true,
                ..Default::default()
            }
        );
        assert_eq!(
            PlayerRow::from(0b0100),
            PlayerRow {
                up: true,
                ..Default::default()
            }
        );
        assert_eq!(
            PlayerRow::from(0b1000),
            PlayerRow {
                right: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_measure_to_beats() {
        assert_eq!(measure_to_beats(184832), 180.5);
        assert_eq!(measure_to_beats(512), 0.5);
    }

    #[test]
    fn test_difficulty_ssq_to_ordered() {
        let ssq_difficulties = vec![4, 1, 2, 3, 6];
        let difficulties = ssq_difficulties
            .iter()
            .map(Level::ssq_to_ordered)
            .map(|d| d.unwrap())
            .collect::<Vec<u8>>();
        // check if ordered ascending
        for window in difficulties.windows(2) {
            assert!(window[0] < window[1]);
        }
    }

    #[test]
    fn test_difficuly_new() {
        assert_eq!(Level::new(2, 4).unwrap().difficulty, 0);
        assert!(Level::new(0, 0).is_err());
        assert!(Level::new(4, 0).is_err());
        assert!(Level::new(1, 5).is_err());
        assert!(Level::new(1, 8).is_err());
    }

    #[test]
    fn test_difficulty_relative() {
        let mut difficulty = Level {
            players: 1,
            difficulty: 0,
        };
        assert_eq!(difficulty.relative_difficulty(), 0.0);
        difficulty.difficulty = 2;
        assert_eq!(difficulty.relative_difficulty(), 0.5);
        difficulty.difficulty = 4;
        assert_eq!(difficulty.relative_difficulty(), 1.0);
    }

    #[test]
    fn test_difficuly_from_u16() {
        let values = [(0b0000010000001000, 0, 2), (0b0000011000000100, 4, 1)];
        for (data, difficulty, players) in values.iter() {
            let diff = Level::try_from(*data).unwrap();
            assert_eq!(diff.players, *players);
            assert_eq!(diff.difficulty, *difficulty);
        }
    }

    #[test]
    fn test_difficulty_display() {
        let values = [
            ("Single Beginner", 0, 1),
            ("Double Basic", 1, 2),
            ("Single Difficult", 2, 1),
            ("Double Expert", 3, 2),
            ("Double Challenge", 4, 2),
            ("Unknown Number of Players Unknown Difficulty", 5, 3),
            ("Unknown Number of Players Unknown Difficulty", 7, 0),
        ];
        for (displayed, difficulty, players) in values.iter() {
            assert_eq!(
                format!(
                    "{}",
                    Level {
                        players: *players,
                        difficulty: *difficulty,
                    }
                ),
                *displayed
            );
        }
    }

    #[test]
    fn test_difficulty_to_value() {
        let levels: Vec<u8> = (1..=10).collect();
        let mut last_level = 0;
        for players in [1, 2].iter() {
            for difficulty in [4, 1, 2, 3, 6].iter() {
                let difficulty = Level::new(*players, *difficulty).unwrap();
                let level = difficulty.to_value(&levels);
                assert!(last_level < level);
                last_level = level;
            }
        }
    }
}
