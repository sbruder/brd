use std::ops::Deref;
use std::path::PathBuf;
use std::str::FromStr;

use quick_xml::de::{from_str, DeError};
use serde::de;
use serde::Deserialize;
use thiserror::Error;

use crate::ddr::arc;

#[derive(Debug, Error)]
pub enum Error {
    #[error("“data/gamedata/musicdb.xml” not found in archive")]
    NotInArchive,
    #[error(transparent)]
    DeError(#[from] DeError),
    #[error(transparent)]
    ArcError(#[from] arc::Error),
    #[error(transparent)]
    FromUtf8Error(#[from] std::string::FromUtf8Error),
}

/// Type that implements [`serde::de::Deserialize`] for space separated lists in xml tag bodies.
///
/// [`serde::de::Deserialize`]: ../../../serde/de/trait.Deserialize.html
#[derive(Debug)]
pub struct XMLList<T>(Vec<T>);

impl<'de, T> serde::de::Deserialize<'de> for XMLList<T>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self(
            s.split(' ')
                .map(|x| x.parse().map_err(de::Error::custom))
                .collect::<Result<Vec<T>, _>>()?,
        ))
    }
}

impl<T> Deref for XMLList<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// This currently only includes fields present in every entry.
#[derive(Debug, Deserialize)]
pub struct Entry {
    pub mcode: u32,
    pub basename: String,
    pub title: String,
    pub artist: String,
    pub bpmmax: u16,
    pub series: u8,
    #[serde(rename = "diffLv")]
    pub diff_lv: XMLList<u8>,
}

/// Holds entries from `musicdb.xml` and can be deserialized from it with [`parse`]
///
/// [`parse`]: fn.parse.html
#[derive(Debug, Deserialize)]
pub struct MusicDB {
    pub music: Vec<Entry>,
}

impl MusicDB {
    /// Parses `musicdb.xml` found in the `startup.arc` archive of DDR A. Currently does not work
    /// for older versions.
    pub fn parse(data: &str) -> Result<Self, DeError> {
        from_str(data)
    }

    /// Convenience function that reads `musicdb.xml` from `startup.arc` and then parses it.
    pub fn parse_from_startup_arc(data: &[u8]) -> Result<Self, Error> {
        let arc = arc::ARC::parse(&data)?;

        let musicdb_data = arc
            .files
            .get(&PathBuf::from("data/gamedata/musicdb.xml"))
            .ok_or(Error::NotInArchive)?;

        Self::parse(&String::from_utf8(musicdb_data.to_vec())?).map_err(|err| err.into())
    }

    pub fn get_entry_from_basename(&self, basename: &str) -> Option<&Entry> {
        for entry in &self.music {
            if entry.basename == basename {
                return Some(entry);
            }
        }

        None
    }
}
