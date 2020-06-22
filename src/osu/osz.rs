use std::fs::File;
use std::io::{Result, Write};
use std::path::PathBuf;

use zip::write::{FileOptions, ZipWriter};

use crate::osu::beatmap;

pub struct Archive<'a> {
    pub beatmaps: Vec<beatmap::Beatmap>,
    pub assets: Vec<(&'a str, &'a [u8])>,
}

impl Archive<'_> {
    pub fn write(&self, filename: &PathBuf) -> Result<()> {
        let file = File::create(filename)?;
        let mut zip = ZipWriter::new(file);

        for beatmap in &self.beatmaps {
            let filename = format!(
                "{} - {} ({}) [{}].osu",
                beatmap.metadata.artist,
                beatmap.metadata.title,
                beatmap.metadata.creator,
                beatmap.metadata.version
            );
            let options = FileOptions::default();
            zip.start_file(filename, options)?;
            zip.write_all(format!("{}", beatmap).as_bytes())?;
        }

        for asset in &self.assets {
            // Assets mostly are already compressed (e.g. JPEG, MP3)
            let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);
            zip.start_file(asset.0, options)?;
            zip.write_all(asset.1)?;
        }

        zip.finish()?;

        Ok(())
    }
}
