use std::collections::HashMap;
use std::convert::TryInto;
use std::io;
use std::io::prelude::*;
use std::io::Cursor;
use std::num;
use std::path::PathBuf;

use byteorder::{ReadBytesExt, LE};
use konami_lz77::decompress;
use log::{debug, info, trace, warn};
use thiserror::Error;

use crate::mini_parser;

const MAGIC: u32 = 0x19751120;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid magic (expected {expected:#x}, found {found:#x})")]
    InvalidMagic { expected: u32, found: u32 },
    #[error("Invalid size after decompresseion (expected {expected}, found {found})")]
    DecompressionSize { expected: usize, found: usize },
    #[error(transparent)]
    IOError(#[from] io::Error),
    #[error(transparent)]
    TryFromIntError(#[from] num::TryFromIntError),
    #[error(transparent)]
    MiniParserError(#[from] mini_parser::MiniParserError),
}

#[derive(Debug)]
struct CueEntry {
    name_offset: usize,
    data_offset: usize,
    decompressed_size: usize,
    compressed_size: usize,
}

impl CueEntry {
    fn parse(data: &[u8]) -> Result<Self, Error> {
        let mut cursor = Cursor::new(data);

        let name_offset = cursor.read_u32::<LE>()?.try_into()?;
        let data_offset = cursor.read_u32::<LE>()?.try_into()?;
        let decompressed_size = cursor.read_u32::<LE>()?.try_into()?;
        let compressed_size = cursor.read_u32::<LE>()?.try_into()?;

        Ok(Self {
            name_offset,
            data_offset,
            decompressed_size,
            compressed_size,
        })
    }
}

pub struct ARC {
    pub files: HashMap<PathBuf, Vec<u8>>,
}

impl ARC {
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        let mut cursor = Cursor::new(data);

        let magic = cursor.read_u32::<LE>()?;
        if magic != MAGIC {
            return Err(Error::InvalidMagic {
                expected: MAGIC,
                found: magic,
            });
        }

        let version = cursor.read_u32::<LE>()?;
        debug!("Recognised archive (version {})", version);
        if version != 1 {
            warn!("Unknown version {}, continuing anyway", version);
        }

        let file_count = cursor.read_u32::<LE>()?;
        debug!("Archive contains {} files", file_count);

        let _compression = cursor.read_u32::<LE>()?;

        let mut cue = Vec::new();
        cursor
            .take((4 * 4 * file_count).into())
            .read_to_end(&mut cue)?;
        let cue: Vec<CueEntry> = cue
            .chunks(4 * 4)
            .map(CueEntry::parse)
            .collect::<Result<_, _>>()?;

        let mut files = HashMap::new();

        for entry in cue {
            let path = PathBuf::from(
                String::from_utf8_lossy(
                    &mini_parser::get_slice_range(data, entry.name_offset..data.len())?
                        .iter()
                        .take_while(|byte| **byte != 0)
                        .cloned()
                        .collect::<Vec<u8>>(),
                )
                .into_owned(),
            );

            trace!("Found entry with path {}", path.display());

            let data = mini_parser::get_slice_range(
                data,
                entry.data_offset..entry.data_offset + entry.compressed_size,
            )?;

            let data = if entry.compressed_size != entry.decompressed_size {
                trace!("Decompressing file");
                decompress(data)
            } else {
                trace!("File is not compressed");
                data.to_vec()
            };

            if data.len() != entry.decompressed_size {
                return Err(Error::DecompressionSize {
                    expected: entry.decompressed_size,
                    found: data.len(),
                });
            }

            debug!(
                "Processed entry with path {} and length {}",
                path.display(),
                data.len()
            );

            files.insert(path, data);
        }

        info!("Processed {} files", files.len());

        Ok(Self { files })
    }
}
