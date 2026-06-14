/// Compute CRC-32 (IEEE 802.3 / PKZip) over `data`.
///
/// `no_std` compatible — no lookup tables, just the bitwise loop.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            // branchless multiply-by-polynomial
            crc = (crc >> 1) ^ (0xEDB88320 & !((crc & 1).wrapping_sub(1)));
        }
    }
    !crc
}
