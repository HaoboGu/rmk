use crate::MACRO_SPACE_SIZE;

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
