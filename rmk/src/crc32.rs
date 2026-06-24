/// Incremental CRC-32 (IEEE 802.3 / PKZip) calculator.
///
/// `no_std` compatible — no lookup tables, just the bitwise loop.
/// Feed data with [`Crc32::update`], then call [`Crc32::finalize`].
pub struct Crc32 {
    state: u32,
}

impl Crc32 {
    pub fn new() -> Self {
        Self { state: !0u32 }
    }

    /// Feed a chunk of data into the running CRC.
    pub fn update(&mut self, data: &[u8]) {
        for &byte in data {
            self.state ^= byte as u32;
            for _ in 0..8 {
                // branchless multiply-by-polynomial
                self.state =
                    (self.state >> 1) ^ (0xEDB88320 & !((self.state & 1).wrapping_sub(1)));
            }
        }
    }

    /// Finalise and return the CRC-32.
    pub fn finalize(&self) -> u32 {
        !self.state
    }

}

/// Compute CRC-32 (IEEE 802.3 / PKZip) over `data` in one shot.
///
/// `no_std` compatible — no lookup tables, just the bitwise loop.
pub fn crc32(data: &[u8]) -> u32 {
    let mut c = Crc32::new();
    c.update(data);
    c.finalize()
}
