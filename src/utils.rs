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
