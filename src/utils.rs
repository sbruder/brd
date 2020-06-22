use anyhow::{anyhow, Result};
use log::debug;

pub fn get_nth_bit(byte: u8, n: u8) -> bool {
    ((byte & (0b1 << n)) >> n) != 0
}

pub fn byte_to_bitarray(byte: u8) -> [bool; 8] {
    let mut bitarray = [false; 8];
    for (i, bit) in bitarray.iter_mut().enumerate() {
        *bit = get_nth_bit(byte, i as u8);
    }
    bitarray
}

pub fn offset_length_to_start_end(offset: usize, length: usize) -> (usize, usize) {
    (offset, offset + length)
}

// This probably isn’t the right way to do this, but after countless attempts to implement
// error conversion (IResult to anyhow::Result) it was the only thing I could come up with.
pub fn exec_nom_parser<'a, F, R>(func: F, input: &'a [u8]) -> Result<R>
where
    F: Fn(&'a [u8]) -> nom::IResult<&[u8], R>,
{
    match func(input) {
        Ok((unprocessed, result)) => {
            if !unprocessed.is_empty() {
                debug!(
                    "Parser returned {} bytes of unprocessed input: {:?}",
                    unprocessed.len(),
                    unprocessed
                );
            }
            Ok(result)
        }
        Err(error) => Err(anyhow!("Nom returned error: {}", error)),
    }
}
