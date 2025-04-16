/// Default macro space size
/// the sum of all macro elements + number of macro elements
pub const MACRO_SPACE_SIZE: usize = 256;

/// Default number of keyboard macros
pub(crate) const NUM_MACRO: usize = 256;

#[derive(Debug)]
pub struct KeyboardMacrosConfig {
    /// macros stored in biunary format to be compatible with Vial
    pub macro_sequences: [u8; MACRO_SPACE_SIZE],
}

impl Default for KeyboardMacrosConfig {
    fn default() -> Self {
        Self {
            macro_sequences: [0; MACRO_SPACE_SIZE],
        }
    }
}

impl KeyboardMacrosConfig {
    pub fn new(macro_sequences: [u8; MACRO_SPACE_SIZE]) -> Self {
        Self { macro_sequences }
    }
}
