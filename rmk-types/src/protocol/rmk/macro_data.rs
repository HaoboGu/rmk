//! Macro endpoint types.

use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::constants::MACRO_DATA_SIZE;
use crate::protocol::Vec;

/// Raw macro data for a single macro chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct MacroData {
    pub data: Vec<u8, MACRO_DATA_SIZE>,
}

impl MaxSize for MacroData {
    const POSTCARD_MAX_SIZE: usize =
        u8::POSTCARD_MAX_SIZE * MACRO_DATA_SIZE + crate::varint_max_size(MACRO_DATA_SIZE);
}

/// Request payload for `GetMacro`.
///
/// The host reads macro data in chunks of up to `MACRO_DATA_SIZE` bytes.
/// A response shorter than `MACRO_DATA_SIZE` signals the end of the macro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct GetMacroRequest {
    pub index: u8,
    pub offset: u16,
}

/// Request payload for `SetMacro`.
///
/// The host writes macro data in chunks of up to `MACRO_DATA_SIZE` bytes.
/// A final chunk shorter than `MACRO_DATA_SIZE` signals the end of the macro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct SetMacroRequest {
    pub index: u8,
    pub offset: u16,
    pub data: MacroData,
}
