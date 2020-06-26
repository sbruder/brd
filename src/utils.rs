use std::fmt;

fn get_nth_bit(byte: u8, n: u8) -> bool {
    ((byte & (0b1 << n)) >> n) != 0
}

pub fn byte_to_bitarray(byte: u8) -> [bool; 8] {
    let mut bitarray = [false; 8];
    for (i, bit) in bitarray.iter_mut().enumerate() {
        *bit = get_nth_bit(byte, i as u8);
    }
    bitarray
}

#[allow(dead_code)]
/// Used to test `byte_to_bitarray`
pub fn bitarray_to_byte(bitarray: [bool; 8]) -> u8 {
    bitarray
        .iter()
        .enumerate()
        .map(|(i, bit)| (*bit as u8) << i)
        .sum()
}

pub fn join_display_values<T: fmt::Display>(iterable: Vec<T>, separator: &'_ str) -> String {
    iterable
        .iter()
        .map(|val| val.to_string())
        .collect::<Vec<_>>()
        .join(&separator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::TestResult;

    /// Helper function to test bitarray functions
    fn vec_to_arr_bool_8(vector: Vec<bool>) -> [bool; 8] {
        let mut array = [false; 8];
        for (arr_el, vec_el) in array.iter_mut().zip(vector.into_iter()) {
            *arr_el = vec_el;
        }
        array
    }

    #[quickcheck]
    fn test_vec_to_arr_bool_8(vector: Vec<bool>) -> TestResult {
        if vector.len() != 8 {
            return TestResult::discard();
        }
        TestResult::from_bool(vec_to_arr_bool_8(vector.clone()).to_vec() == vector)
    }

    #[quickcheck]
    fn test_byte_to_bitarray(byte: u8) -> bool {
        bitarray_to_byte(byte_to_bitarray(byte)) == byte
    }

    #[quickcheck]
    fn test_bitarray_to_byte(bitvec: Vec<bool>) -> TestResult {
        if bitvec.len() != 8 {
            return TestResult::discard();
        }
        let bitarray = vec_to_arr_bool_8(bitvec);
        TestResult::from_bool(byte_to_bitarray(bitarray_to_byte(bitarray)) == bitarray)
    }
}
