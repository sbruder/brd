use std::collections::HashMap;
use std::convert::TryInto;

use anyhow::{anyhow, Result};
use log::{debug, info, warn};
use nom::bytes::complete::tag;
use nom::error::ParseError;
use nom::multi::count;
use nom::number::complete::{le_i32, le_u32};
use nom::{take_str, IResult};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use crate::utils;
use crate::utils::exec_nom_parser;
use crate::xact3::adpcm;

#[derive(Clone, FromPrimitive, Debug, PartialEq)]
enum FormatTag {
    PCM = 0,
    XMA = 1,
    ADPCM = 2,
    WMA = 3,
}

#[derive(Clone, Debug)]
struct Format {
    tag: FormatTag,
    channels: u16,
    sample_rate: u32,
    alignment: u8,
}

impl From<u32> for Format {
    fn from(format: u32) -> Self {
        Self {
            tag: FormatTag::from_u32(format & ((1 << 2) - 1)).unwrap(),
            channels: ((format >> 2) & ((1 << 3) - 1)) as u16,
            sample_rate: (format >> 5) & ((1 << 18) - 1),
            alignment: ((format >> 23) & ((1 << 8) - 1)) as u8,
        }
    }
}

impl TryInto<adpcm::WaveFormat> for Format {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<adpcm::WaveFormat> {
        if self.tag != FormatTag::ADPCM {
            return Err(anyhow!("Format is not ADPCM"));
        }

        let n_block_align = (self.alignment as u16 + 22) * self.channels;
        let n_samples_per_block =
            (((n_block_align - (7 * self.channels)) * 8) / (4 * self.channels)) + 2;
        let n_avg_bytes_per_sec =
            (self.sample_rate / n_samples_per_block as u32) * n_block_align as u32;

        Ok(adpcm::WaveFormat {
            n_channels: self.channels,
            n_samples_per_sec: self.sample_rate,
            n_avg_bytes_per_sec,
            n_block_align,
            n_samples_per_block,
        })
    }
}

#[derive(Debug)]
struct SegmentPosition {
    start: usize,
    end: usize,
}

impl SegmentPosition {
    fn get_from<'a>(&self, data: &'a [u8]) -> &'a [u8] {
        &data[self.start..self.end]
    }

    fn parse<'a, E: ParseError<&'a [u8]>>(input: &'a [u8]) -> IResult<&[u8], Self, E> {
        let (input, offset) = le_u32(input)?;
        let (input, length) = le_u32(input)?;

        let (start, end) = utils::offset_length_to_start_end(offset as usize, length as usize);
        Ok((input, Self { start, end }))
    }
}

struct Header {
    segment_positions: Vec<SegmentPosition>,
}

impl Header {
    fn parse(input: &[u8]) -> IResult<&[u8], Self> {
        let (input, _magic) = tag("WBND")(input)?;
        let (input, version) = le_u32(input)?;
        debug!("Recognised file (version {})", version);
        if version != 43 {
            warn!("The provided file has an unsupported version ({})", version);
        }
        let (input, _header_version) = le_u32(input)?;
        let (_input, segment_positions) = count(SegmentPosition::parse, 5)(input)?;
        Ok((
            // difference between first segment and parsed bytes of header
            &input[8 * 5 + 12..segment_positions[0].start],
            Self { segment_positions },
        ))
    }
}

#[derive(Debug)]
struct Info {
    entry_count: usize,
    name: String,
}

impl Info {
    fn parse(input: &[u8]) -> IResult<&[u8], Self> {
        let (input, _flags) = le_u32(input)?;
        let (input, entry_count) = le_u32(input)?;
        let (input, name) = take_str64(input)?;
        let (input, _entry_meta_data_element_size) = le_u32(input)?;
        let (input, _entry_name_element_size) = le_u32(input)?;
        let (input, _alignment) = le_u32(input)?;
        let (input, _compact_format) = le_i32(input)?;
        let (input, _build_time) = le_u32(input)?;

        Ok((
            input,
            Self {
                entry_count: entry_count as usize,
                name,
            },
        ))
    }
}

#[derive(Debug)]
struct Entry {
    name: String,
    format: Format,
    data_offset: usize,
    data_length: usize,
}

impl Entry {
    fn parse(input: &[u8]) -> IResult<&[u8], Self> {
        let (input, _flags_and_duration) = le_u32(input)?;
        let (input, format) = le_u32(input)?;
        let (input, data_offset) = le_u32(input)?;
        let (input, data_length) = le_u32(input)?;
        let (input, _loop_start) = le_u32(input)?;
        let (input, _loop_length) = le_u32(input)?;

        Ok((
            input,
            Self {
                name: "".to_string(),
                format: format.into(),
                data_offset: data_offset as usize,
                data_length: data_length as usize,
            },
        ))
    }
}

fn take_str(input: &[u8], len: usize) -> IResult<&[u8], String> {
    let (input, parsed) = take_str!(input, len)?;
    Ok((input, parsed.replace("\0", "")))
}

fn take_str64(input: &[u8]) -> IResult<&[u8], String> {
    take_str(input, 64)
}

#[derive(Debug, Clone)]
pub struct WaveBank<'a> {
    pub name: String,
    pub sounds: HashMap<String, Sound<'a>>,
}

impl WaveBank<'_> {
    pub fn parse(data: &'_ [u8]) -> Result<WaveBank> {
        debug!("Parsing header");
        let header = exec_nom_parser(Header::parse, data)?;

        debug!("Getting segments from file");
        let segments: Vec<&'_ [u8]> = header
            .segment_positions
            .iter()
            .map(|x| x.get_from(data))
            .collect();

        debug!("Parsing info (length {})", segments[0].len());
        let info = exec_nom_parser(Info::parse, segments[0])?;
        debug!("Parsing entries (length {})", segments[1].len());
        let entries = exec_nom_parser(count(Entry::parse, info.entry_count as usize), segments[1])?;
        debug!("Parsing entry names (length {})", segments[3].len());
        let entry_names =
            exec_nom_parser(count(take_str64, info.entry_count as usize), segments[3])?;

        let mut wave_bank = WaveBank {
            name: info.name,
            sounds: HashMap::new(),
        };

        for (entry, name) in entries.iter().zip(entry_names.iter()) {
            let (start, end) =
                utils::offset_length_to_start_end(entry.data_offset, entry.data_length);
            wave_bank.sounds.insert(
                name.replace("\0", "").to_string(),
                Sound {
                    format: entry.format.clone(),
                    data: &segments[4][start..end],
                },
            );
        }

        info!("Parsed WaveBank with {} sounds", wave_bank.sounds.len());

        Ok(wave_bank)
    }
}

#[derive(Clone, Debug)]
pub struct Sound<'a> {
    format: Format,
    data: &'a [u8],
}

impl Sound<'_> {
    pub fn to_wav(&self) -> Result<Vec<u8>> {
        adpcm::build_wav(self.format.clone().try_into()?, self.data)
    }
}
