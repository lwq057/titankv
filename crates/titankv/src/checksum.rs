pub fn crc32c_calculate(data: &[u8]) -> u32 {
    crc32c::crc32c(data)
}

pub fn crc32c_verify(data: &[u8], expected: u32) -> bool {
    crc32c::crc32c(data) == expected
}
