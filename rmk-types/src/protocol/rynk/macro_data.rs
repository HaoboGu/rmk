//! Macro endpoint types.

use heapless::Vec;
use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

use crate::constants::MACRO_DATA_SIZE;

/// Raw macro data for a single macro chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroData {
    pub data: Vec<u8, MACRO_DATA_SIZE>,
}

impl MaxSize for MacroData {
    const POSTCARD_MAX_SIZE: usize = crate::heapless_vec_max_size::<u8, MACRO_DATA_SIZE>();
}

/// Request payload for `GetMacro`.
///
/// The host reads macro data in chunks of up to `MACRO_DATA_SIZE` bytes.
/// A response shorter than `MACRO_DATA_SIZE` signals the end of the macro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
pub struct GetMacroRequest {
    pub index: u8,
    pub offset: u16,
}

/// Request payload for `SetMacro`.
///
/// The host writes macro data in chunks of up to `MACRO_DATA_SIZE` bytes.
/// A final chunk shorter than `MACRO_DATA_SIZE` signals the end of the macro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
pub struct SetMacroRequest {
    pub index: u8,
    pub offset: u16,
    pub data: MacroData,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::rynk::test_utils::{assert_max_size_bound, round_trip};

    #[test]
    fn round_trip_macro_data() {
        // Empty, populated, and max-capacity edge cases.
        round_trip(&MacroData { data: Vec::new() });

        let mut data: Vec<u8, MACRO_DATA_SIZE> = Vec::new();
        data.extend_from_slice(&[0x01, 0x02, 0x03]).unwrap();
        round_trip(&MacroData { data });

        let mut data: Vec<u8, MACRO_DATA_SIZE> = Vec::new();
        for i in 0..MACRO_DATA_SIZE {
            data.push(i as u8).unwrap();
        }
        let full = MacroData { data };
        round_trip(&full);
        assert_max_size_bound(&full);
    }

    #[test]
    fn round_trip_get_macro_request() {
        round_trip(&GetMacroRequest { index: 0, offset: 0 });
        round_trip(&GetMacroRequest { index: 3, offset: 256 });
    }

    #[test]
    fn round_trip_set_macro_request() {
        let mut data: Vec<u8, MACRO_DATA_SIZE> = Vec::new();
        data.extend_from_slice(&[0x01, 0x02]).unwrap();
        round_trip(&SetMacroRequest {
            index: 1,
            offset: 0,
            data: MacroData { data },
        });
    }
}
