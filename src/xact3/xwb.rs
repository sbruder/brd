use std::collections::HashMap;
use std::convert::TryInto;
use std::io;
use std::io::Cursor;
use std::num;

use byteorder::{ReadBytesExt, LE};
use log::{debug, info, trace, warn};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use thiserror::Error;

use crate::mini_parser;
use crate::mini_parser::{MiniParser, MiniParserError};
use crate::xact3::adpcm;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0:?} is not a supported format")]
    UnsupportedFormat(FormatTag),
    #[error("Invalid magic: expected “WBND”, found “{0}”")]
    InvalidMagic(String),
    #[error(transparent)]
    IOError(#[from] io::Error),
    #[error(transparent)]
    MiniParserError(#[from] MiniParserError),
    #[error(transparent)]
    ADPCMError(#[from] adpcm::Error),
    #[error(transparent)]
    TryFromIntError(#[from] num::TryFromIntError),
}

#[derive(Clone, FromPrimitive, Debug, PartialEq)]
pub enum FormatTag {
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
            tag: FormatTag::from_u32(format & ((1 << 2) - 1)).unwrap(), // all 2 bit ints covered
            channels: ((format >> 2) & ((1 << 3) - 1)) as u16,
            sample_rate: (format >> 5) & ((1 << 18) - 1),
            alignment: ((format >> 23) & ((1 << 8) - 1)) as u8,
        }
    }
}

impl TryInto<adpcm::WaveFormat> for Format {
    type Error = Error;

    fn try_into(self) -> Result<adpcm::WaveFormat, Error> {
        if self.tag != FormatTag::ADPCM {
            return Err(Error::UnsupportedFormat(self.tag));
        }

        let block_align = (u16::from(self.alignment) + 22) * self.channels;

        Ok(adpcm::WaveFormat {
            channels: self.channels,
            sample_rate: self.sample_rate,
            block_align,
        })
    }
}

#[derive(Debug)]
struct SegmentPosition {
    offset: usize,
    length: usize,
}

impl SegmentPosition {
    fn get_from<'a>(&self, data: &'a [u8]) -> Result<&'a [u8], MiniParserError> {
        mini_parser::get_slice_range(data, self.offset..self.offset + self.length)
    }
}

struct Header {
    segment_positions: Vec<SegmentPosition>,
}

impl Header {
    fn parse(data: &[u8]) -> Result<Self, Error> {
        let mut cursor = Cursor::new(data);

        let magic = cursor.read_string(4)?;
        if magic != "WBND" {
            return Err(Error::InvalidMagic(magic));
        }

        let version = cursor.read_u32::<LE>()?;
        debug!("Recognised file (version {})", version);
        if version != 43 {
            warn!("The provided file has an unsupported version ({})", version);
        }

        let _header_version = cursor.read_u32::<LE>()?;
        let mut segment_positions = Vec::new();
        for _ in 0..5 {
            let offset = cursor.read_u32::<LE>()?;
            let length = cursor.read_u32::<LE>()?;
            segment_positions.push(SegmentPosition {
                offset: offset.try_into()?,
                length: length.try_into()?,
            })
        }
        Ok(Header { segment_positions })
    }
}

#[derive(Debug)]
struct Info {
    entry_count: usize,
    name: String,
    entry_name_element_size: usize,
}

impl Info {
    fn parse(data: &[u8]) -> Result<Self, Error> {
        let mut cursor = Cursor::new(data);

        let _flags = cursor.read_u32::<LE>()?;
        let entry_count = cursor.read_u32::<LE>()?;
        debug!("Number of entries: {}", entry_count);
        let name = cursor.read_string(64)?;
        debug!("Name of wave bank: {}", name);
        let _entry_meta_data_element_size = cursor.read_u32::<LE>()?;
        let entry_name_element_size = cursor.read_u32::<LE>()?;
        debug!("Size of entry names: {}", entry_name_element_size);
        let _alignment = cursor.read_u32::<LE>()?;
        let _compact_format = cursor.read_u32::<LE>()?;
        let _build_time = cursor.read_u32::<LE>()?;

        Ok(Self {
            entry_count: entry_count.try_into()?,
            name,
            entry_name_element_size: entry_name_element_size.try_into()?,
        })
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
    fn parse(data: &[u8]) -> Result<Self, Error> {
        let mut cursor = Cursor::new(data);

        let _flags_and_duration = cursor.read_u32::<LE>()?;
        let format = cursor.read_u32::<LE>()?;
        let data_offset = cursor.read_u32::<LE>()?;
        let data_length = cursor.read_u32::<LE>()?;
        let _loop_start = cursor.read_u32::<LE>()?;
        let _loop_length = cursor.read_u32::<LE>()?;

        trace!(
            "Parsed Entry with Format {:?} at offset {} (length {})",
            Format::from(format),
            data_offset,
            data_length
        );

        Ok(Self {
            name: "".to_string(),
            format: format.into(),
            data_offset: data_offset.try_into()?,
            data_length: data_length.try_into()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct WaveBank<'a> {
    pub name: String,
    pub sounds: HashMap<String, Sound<'a>>,
}

impl WaveBank<'_> {
    pub fn parse(data: &'_ [u8]) -> Result<WaveBank<'_>, Error> {
        debug!("Parsing header");
        let header = Header::parse(mini_parser::get_slice_range(data, 0..52)?)?;

        debug!("Getting segments from file");
        let segments: Vec<&'_ [u8]> = header
            .segment_positions
            .iter()
            .map(|segment| segment.get_from(data))
            .collect::<Result<_, _>>()?;

        debug!("Parsing info (length {})", segments[0].len());
        let info = Info::parse(segments[0])?;

        debug!("Parsing entries (length {})", segments[1].len());
        let mut entries: Vec<Entry> = segments[1]
            .chunks_exact(24)
            .map(Entry::parse)
            .collect::<Result<_, _>>()?;

        debug!("Parsing entry names (length {})", segments[3].len());
        let entry_names: Vec<String> = segments[3]
            .chunks_exact(info.entry_name_element_size)
            .map(String::from_utf8_lossy)
            .map(|name| name.into_owned())
            .collect();

        for (i, entry) in entries.iter_mut().enumerate() {
            entry.name = entry_names
                .get(i)
                .map(|name| name.to_string())
                .unwrap_or_else(|| {
                    warn!("Entry does not have name; naming after index {}", i);
                    i.to_string()
                });
        }

        let mut wave_bank = WaveBank {
            name: info.name,
            sounds: HashMap::new(),
        };

        for entry in entries.iter() {
            let end = entry.data_offset + entry.data_length;
            wave_bank.sounds.insert(
                entry.name.replace("\0", "").to_string(),
                Sound {
                    format: entry.format.clone(),
                    data: mini_parser::get_slice_range(segments[4], entry.data_offset..end)?,
                    size: entry.data_length,
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
    pub size: usize,
}

impl Sound<'_> {
    pub fn to_wav(&self) -> Result<Vec<u8>, Error> {
        match &self.format.tag {
            FormatTag::ADPCM => Ok(adpcm::build_wav(
                self.format.clone().try_into()?,
                self.data,
            )?),
            _ => Err(Error::UnsupportedFormat(self.format.tag.clone())),
        }
    }
}
