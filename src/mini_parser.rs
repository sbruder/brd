use std::convert::TryInto;
use std::io;
use std::io::prelude::*;
use std::num;
use std::ops::Range;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum MiniParserError {
    #[error(transparent)]
    TryFromIntError(#[from] num::TryFromIntError),
    #[error(transparent)]
    IOError(#[from] io::Error),
}

/// Provides convenience methods for parsing binary formats.
pub trait MiniParser: io::Read {
    /// Read a `String` of length `length` and strip NUL bytes.
    #[inline]
    fn read_string(&mut self, length: usize) -> Result<String, MiniParserError> {
        let mut buf = String::new();
        self.take(length.try_into()?).read_to_string(&mut buf)?;
        Ok(buf.replace("\0", ""))
    }

    /// Read `n` `u32`.
    #[inline]
    fn read_n_u32(&mut self, n: usize) -> Result<Vec<i32>, MiniParserError> {
        let mut buf = vec![0; 4 * n];
        self.read_exact(&mut buf)?;
        Ok(buf
            .chunks_exact(4)
            .map(|x| x.try_into().unwrap()) // chunks are guarenteed to be of size 4
            .map(i32::from_le_bytes)
            .collect::<Vec<i32>>())
    }
}

/// Implement MiniParser for all io::Read implementors.
impl<R: io::Read + ?Sized> MiniParser for R {}

/// Gets the requested `range` from `slice` and errors with `UnexpectedEof` when range does not fit
/// in slice.
pub fn get_slice_range(slice: &[u8], range: Range<usize>) -> Result<&[u8], MiniParserError> {
    slice.get(range).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "File ended while there was data left to process",
        )
        .into()
    })
}
