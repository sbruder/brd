use std::convert::TryInto;
use std::io::{Cursor, Write};

use byteorder::{WriteBytesExt, LE};
use log::{debug, trace};
use thiserror::Error;

/// Standard ADPCM coefficients
#[rustfmt::skip]
const COEFFS: &[CoefSet] = &[
    (256,    0),
    (512, -256),
    (  0,    0),
    (192,   64),
    (240,    0),
    (460, -208),
    (392, -232),
];

#[derive(Debug, Error)]
pub enum Error {
    /// WAVE only supports file sizes up to 2<sup>32</sup> bytes (2<sup>32</sup> - 82 bytes of
    /// usable audio data in this case).
    #[error("unable to create file of size {0} (larger than 2^32 - 82 bytes)")]
    TooLargeError(usize),
}

/// All wave chunks implement this trait.
trait WaveChunk {
    /// Serialize to byte vector that is used as a part of the resulting wave file.
    fn to_chunk(&self) -> Vec<u8>;
}

/// One set of ADPCM coefficients
type CoefSet = (i16, i16);

/// `WAVE_FORMAT_ADPCM` header.
///
/// It only includes fields that are usful for usage in conjunction with XACT3. The other fields
/// are static and defined in the [`to_chunk`] method of this type.
///
/// [`to_chunk`]: trait.WaveChunk.html#tymethod.to_chunk
pub struct WaveFormat {
    // wFormatTag = 2
    /// `nChannels`: Number of channels
    pub channels: u16,
    /// `nSamplesPerSec`: Sample rate
    pub sample_rate: u32,
    // nAvgBytesPerSec (calculated),
    /// `nBlockAlign`: Block alignment (in bytes)
    pub block_align: u16,
    // wBitsPerSample = 4
    // cbSize = 32
    // nSamplesPerBlock (calculated)
    // nNumCoeff = 7
    // aCoeff = COEFFS
}

impl WaveChunk for WaveFormat {
    fn to_chunk(&self) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        write!(buf, "fmt ").unwrap();
        buf.write_u32::<LE>(2 + 2 + 4 + 4 + 2 + 2 + 2 + 2 + 2 + 4 * COEFFS.len() as u32)
            .unwrap();
        buf.write_u16::<LE>(2).unwrap(); // WAVE_FORMAT_ADPCM
        buf.write_u16::<LE>(self.channels).unwrap();
        buf.write_u32::<LE>(self.sample_rate).unwrap();
        buf.write_u32::<LE>(self.avg_bytes_per_sec()).unwrap(); // nAvgBytesPerSec
        buf.write_u16::<LE>(self.block_align).unwrap();
        buf.write_u16::<LE>(4).unwrap(); // wBitsPerSample
        buf.write_u16::<LE>(32).unwrap(); // cbSize
        buf.write_u16::<LE>(self.samples_per_block()).unwrap();
        buf.write_u16::<LE>(COEFFS.len().try_into().unwrap())
            .unwrap(); // nNumCoeff
        for coef_set in COEFFS {
            buf.write_i16::<LE>(coef_set.0).unwrap();
            buf.write_i16::<LE>(coef_set.1).unwrap();
        }
        buf.into_inner()
    }
}

impl WaveFormat {
    /// Calculate `nSamplesPerBlock`
    fn samples_per_block(&self) -> u16 {
        (((self.block_align - (7 * self.channels)) * 8) / (4 * self.channels)) + 2
    }

    /// Calculate `nAvgBytesPerSec`
    fn avg_bytes_per_sec(&self) -> u32 {
        (self.sample_rate / u32::from(self.samples_per_block())) * u32::from(self.block_align)
    }
}

/// Wave fact chunk
struct WaveFact {
    /// The length of the audio data in samples
    length_samples: u32,
}

impl WaveChunk for WaveFact {
    fn to_chunk(&self) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        write!(buf, "fact").unwrap();
        buf.write_u32::<LE>(4).unwrap(); // length of fact chunk
        buf.write_u32::<LE>(self.length_samples).unwrap();
        buf.into_inner()
    }
}

/// RIFF header chunk
struct RIFFHeader {
    /// Size of the file minus 8 bytes (`RIFF` magic number and the file size)
    file_size: u32,
}

impl WaveChunk for RIFFHeader {
    fn to_chunk(&self) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        write!(buf, "RIFF").unwrap();
        buf.write_u32::<LE>(self.file_size).unwrap();
        write!(buf, "WAVE").unwrap();
        buf.into_inner()
    }
}

/// Builds wave data from a given [`WaveFormat`] and raw ADPCM data.
///
/// # Errors
///
/// This function returns a [`TooLargeError`] when the length of `data` is greater than or equal to 2<sup>32</sup> - 82
///
/// [`WaveFormat`]: struct.WaveFormat.html
/// [`TooLargeError`]: enum.Error.html#variant.TooLargeError
pub fn build_wav(format: WaveFormat, data: &[u8]) -> Result<Vec<u8>, Error> {
    debug!("Building file");
    // returning `u32::MAX` will make the next check fail
    let length: u32 = data.len().try_into().unwrap_or(u32::MAX);

    let riff_header = RIFFHeader {
        file_size: length
            .checked_add(82)
            .ok_or_else(|| Error::TooLargeError(data.len()))?,
    };

    let fact = WaveFact {
        length_samples: ((length / u32::from(format.block_align))
            * u32::from((format.block_align - (7 * format.channels)) * 8)
            / 4)
            / u32::from(format.channels),
    };

    let mut buf = Cursor::new(Vec::new());

    trace!("Building RIFF header");
    buf.write_all(&riff_header.to_chunk()).unwrap();
    trace!("Building fmt  chunk");
    buf.write_all(&format.to_chunk()).unwrap();
    trace!("Building fact chunk");
    buf.write_all(&fact.to_chunk()).unwrap();

    write!(buf, "data").unwrap();
    buf.write_u32::<LE>(length).unwrap();
    buf.write_all(data).unwrap();

    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_riff_header_to_chunk() {
        assert_eq!(
            RIFFHeader { file_size: 12345 }.to_chunk(),
            b"RIFF\x39\x30\x00\x00WAVE"
        );
    }

    #[test]
    fn test_wave_fact_to_chunk() {
        assert_eq!(
            WaveFact {
                length_samples: 12345
            }
            .to_chunk(),
            b"fact\x04\x00\x00\x00\x39\x30\x00\x00"
        );
    }

    #[test]
    fn test_wave_format_to_chunk() {
        assert_eq!(
            WaveFormat {
                channels: 2,
                sample_rate: 44100,
                block_align: 140,
            }
            .to_chunk(),
            vec![
                0x66, 0x6d, 0x74, 0x20, 0x32, 0x00, 0x00, 0x00, 0x02, 0x00, 0x02, 0x00, 0x44, 0xac,
                0x00, 0x00, 0x20, 0xbc, 0x00, 0x00, 0x8c, 0x00, 0x04, 0x00, 0x20, 0x00, 0x80, 0x00,
                0x07, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0xff, 0x00, 0x00, 0x00, 0x00,
                0xc0, 0x00, 0x40, 0x00, 0xf0, 0x00, 0x00, 0x00, 0xcc, 0x01, 0x30, 0xff, 0x88, 0x01,
                0x18, 0xff
            ]
        );
    }

    #[test]
    fn test_build_wav() {
        let built_wav = build_wav(
            WaveFormat {
                channels: 2,
                sample_rate: 44100,
                block_align: 140,
            },
            b"data",
        );
        assert_eq!(
            built_wav.unwrap(),
            vec![
                0x52, 0x49, 0x46, 0x46, 0x56, 0x00, 0x00, 0x00, 0x57, 0x41, 0x56, 0x45, 0x66, 0x6d,
                0x74, 0x20, 0x32, 0x00, 0x00, 0x00, 0x02, 0x00, 0x02, 0x00, 0x44, 0xac, 0x00, 0x00,
                0x20, 0xbc, 0x00, 0x00, 0x8c, 0x00, 0x04, 0x00, 0x20, 0x00, 0x80, 0x00, 0x07, 0x00,
                0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0xff, 0x00, 0x00, 0x00, 0x00, 0xc0, 0x00,
                0x40, 0x00, 0xf0, 0x00, 0x00, 0x00, 0xcc, 0x01, 0x30, 0xff, 0x88, 0x01, 0x18, 0xff,
                0x66, 0x61, 0x63, 0x74, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x64, 0x61,
                0x74, 0x61, 0x04, 0x00, 0x00, 0x00, 0x64, 0x61, 0x74, 0x61
            ]
        );
    }
}
