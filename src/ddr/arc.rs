use std::collections::HashMap;
use std::convert::TryInto;
use std::default::Default;
use std::io;
use std::io::prelude::*;
use std::io::Cursor;
use std::num;
use std::path::PathBuf;

use byteorder::{ReadBytesExt, LE};
use derive_more::Deref;
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
    MiniParserError(#[from] mini_parser::Error),
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Default, PartialEq)]
struct CueEntry {
    path_offset: usize,
    data_offset: usize,
    decompressed_size: usize,
    compressed_size: usize,
}

impl CueEntry {
    fn parse(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);

        let path_offset = cursor.read_u32::<LE>()?.try_into()?;
        let data_offset = cursor.read_u32::<LE>()?.try_into()?;
        let decompressed_size = cursor.read_u32::<LE>()?.try_into()?;
        let compressed_size = cursor.read_u32::<LE>()?.try_into()?;

        Ok(Self {
            path_offset,
            data_offset,
            decompressed_size,
            compressed_size,
        })
    }

    fn parse_path(&self, data: &[u8]) -> Result<PathBuf> {
        Ok(PathBuf::from(
            String::from_utf8_lossy(
                &mini_parser::get_slice_range(data, self.path_offset..data.len())?
                    .iter()
                    .take_while(|byte| **byte != 0)
                    .cloned()
                    .collect::<Vec<u8>>(),
            )
            .into_owned(),
        ))
    }
}

#[derive(Debug, Deref, PartialEq)]
struct Cue(HashMap<PathBuf, CueEntry>);

impl Cue {
    fn parse(data: &[u8], arc_data: &[u8]) -> Result<Self> {
        let mut cue = HashMap::new();

        for chunk in data.chunks(4 * 4) {
            let entry = CueEntry::parse(chunk)?;
            let path = entry.parse_path(arc_data)?;
            trace!(
                "Found cue entry with path {} at {} (size {})",
                path.display(),
                entry.data_offset,
                entry.decompressed_size,
            );
            cue.insert(path, entry);
        }

        Ok(Self(cue))
    }
}

#[derive(Debug, PartialEq)]
pub struct ARC<'a> {
    data: &'a [u8],
    file_count: u32,
    version: u32,
    cue: Cue,
}

impl<'a> ARC<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
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

        let mut cue_data = vec![0u8; (4 * 4 * file_count).try_into().unwrap()];
        cursor.read_exact(&mut cue_data)?;
        let cue = Cue::parse(&cue_data, &data)?;

        info!("ARC archive has {} files", cue.len());

        Ok(Self {
            data,
            file_count,
            version,
            cue,
        })
    }

    pub fn has_file(&self, path: &PathBuf) -> bool {
        self.cue.get(path).is_some()
    }

    pub fn file_paths(&self) -> Vec<&PathBuf> {
        self.cue.keys().collect()
    }

    /// Gets a single file from the archive.
    ///
    /// Returns `Ok(None)` when the file does not exist and returns an error when the file could
    /// not be read.
    pub fn get_file(&self, path: &PathBuf) -> Result<Option<Vec<u8>>> {
        let entry = match self.cue.get(path) {
            Some(entry) => entry,
            None => return Ok(None),
        };

        let data = mini_parser::get_slice_range(
            self.data,
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
            "Got file with path {} and length {}",
            path.display(),
            data.len()
        );

        Ok(Some(data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cue_entry_parse() {
        assert_eq!(
            CueEntry::parse(b"\xa0\x00\x00\x00\xc0\x01\x00\x00\x5c\x04\x00\x00\x2d\x02\x00\x00")
                .unwrap(),
            CueEntry {
                path_offset: 160,
                data_offset: 448,
                decompressed_size: 1116,
                compressed_size: 557,
            }
        );
    }

    #[quickcheck]
    fn test_cue_entry_parse_size(data: Vec<u8>) -> bool {
        let cue_entry = CueEntry::parse(&data);
        if dbg!(data.len()) >= 16 {
            cue_entry.is_ok()
        } else {
            cue_entry.is_err()
        }
    }

    #[test]
    fn test_cue_entry_parse_path() {
        let cue_entry = CueEntry {
            path_offset: 7,
            ..Default::default()
        };
        cue_entry.parse_path(b"").unwrap_err();
        let path = cue_entry
            .parse_path(b"1234567test/file/name\0after path")
            .unwrap();
        assert_eq!(path, PathBuf::from("test/file/name"));
    }

    #[test]
    fn test_parse_cue() {
        // only path_offset is required to have a useful value to test the cue
        #[rustfmt::skip]
        let cue = Cue::parse(&[
            0x02, 0x00, 0x00, 0x00, // first file (path offset 2)
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x0f, 0x00, 0x00, 0x00, // second file (path offset 15)
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ], b"abpath/to/file\0other/file\0z").unwrap();
        let mut expected_cue = HashMap::new();
        expected_cue.insert(
            PathBuf::from("path/to/file"),
            CueEntry {
                path_offset: 2,
                ..Default::default()
            },
        );
        expected_cue.insert(
            PathBuf::from("other/file"),
            CueEntry {
                path_offset: 15,
                ..Default::default()
            },
        );
        assert_eq!(cue, Cue(expected_cue));
    }
}
