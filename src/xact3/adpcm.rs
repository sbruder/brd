use std::convert::TryInto;
use std::io::{Cursor, Write};

use byteorder::{LittleEndian, WriteBytesExt};
use log::{debug, trace};
use thiserror::Error;

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
    #[error("unable to create file of size {0} (larger than 2^32)")]
    TooLarge(usize),
}

trait WaveChunk {
    fn to_chunk(&self) -> Vec<u8>;
}

type CoefSet = (i16, i16);

pub struct WaveFormat {
    // w_format_tag = 2
    pub n_channels: u16,
    pub n_samples_per_sec: u32,
    pub n_avg_bytes_per_sec: u32,
    pub n_block_align: u16,
    // w_bits_per_sample = 4
    // cb_size = 32
    pub n_samples_per_block: u16,
    // w_num_coeff = 7
    // a_coeff = COEFFS
}

impl WaveChunk for WaveFormat {
    fn to_chunk(&self) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        write!(buf, "fmt ").unwrap();
        buf.write_u32::<LittleEndian>(2 + 2 + 4 + 4 + 2 + 2 + 2 + 2 + 2 + 4 * COEFFS.len() as u32)
            .unwrap();
        buf.write_u16::<LittleEndian>(2).unwrap(); // WAVE_FORMAT_ADPCM
        buf.write_u16::<LittleEndian>(self.n_channels).unwrap();
        buf.write_u32::<LittleEndian>(self.n_samples_per_sec)
            .unwrap();
        buf.write_u32::<LittleEndian>(self.n_avg_bytes_per_sec)
            .unwrap();
        buf.write_u16::<LittleEndian>(self.n_block_align).unwrap();
        buf.write_u16::<LittleEndian>(4).unwrap(); // wBitsPerSample
        buf.write_u16::<LittleEndian>(32).unwrap(); // cbSize
        buf.write_u16::<LittleEndian>(self.n_samples_per_block)
            .unwrap();
        buf.write_u16::<LittleEndian>(7).unwrap(); // wNumCoeff
        for coef_set in COEFFS {
            buf.write_i16::<LittleEndian>(coef_set.0).unwrap();
            buf.write_i16::<LittleEndian>(coef_set.1).unwrap();
        }
        buf.into_inner()
    }
}

struct WaveFact {
    length_samples: u32,
}

impl WaveChunk for WaveFact {
    fn to_chunk(&self) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        write!(buf, "fact").unwrap();
        buf.write_u32::<LittleEndian>(4).unwrap();
        buf.write_u32::<LittleEndian>(self.length_samples).unwrap();
        buf.into_inner()
    }
}

struct RIFFHeader {
    file_size: u32,
}

impl WaveChunk for RIFFHeader {
    fn to_chunk(&self) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        write!(buf, "RIFF").unwrap();
        buf.write_u32::<LittleEndian>(self.file_size).unwrap();
        write!(buf, "WAVE").unwrap();
        buf.into_inner()
    }
}

pub fn build_wav(format: WaveFormat, data: &[u8]) -> Result<Vec<u8>, Error> {
    debug!("Building file");
    let length: u32 = data
        .len()
        .try_into()
        .map_err(|_| Error::TooLarge(data.len()))?;

    let riff_header = RIFFHeader {
        file_size: 82 + length,
    };

    let fact = WaveFact {
        length_samples: ((length / u32::from(format.n_block_align))
            * u32::from((format.n_block_align - (7 * format.n_channels)) * 8)
            / 4)
            / u32::from(format.n_channels),
    };

    let mut buf = Cursor::new(Vec::new());

    trace!("Building RIFF header");
    buf.write_all(&riff_header.to_chunk()).unwrap();
    trace!("Building fmt  chunk");
    buf.write_all(&format.to_chunk()).unwrap();
    trace!("Building fact chunk");
    buf.write_all(&fact.to_chunk()).unwrap();

    write!(buf, "data").unwrap();
    buf.write_u32::<LittleEndian>(length).unwrap();
    buf.write_all(data).unwrap();

    Ok(buf.into_inner())
}
