//! Macro endpoint types.

use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::constants::PROTOCOL_MAX_MACRO_DATA;
use crate::protocol_vec::ProtocolVec;

/// Raw macro data for a single macro chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct MacroData {
    pub data: ProtocolVec<u8, PROTOCOL_MAX_MACRO_DATA>,
}

impl MaxSize for MacroData {
    const POSTCARD_MAX_SIZE: usize =
        u8::POSTCARD_MAX_SIZE * PROTOCOL_MAX_MACRO_DATA + crate::varint_max_size(PROTOCOL_MAX_MACRO_DATA);
}

/// Request payload for `GetMacro`.
///
/// The host reads macro data in chunks of up to `MAX_MACRO_DATA` bytes.
/// A response shorter than `MAX_MACRO_DATA` signals the end of the macro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct GetMacroRequest {
    pub index: u8,
    pub offset: u16,
}

/// Request payload for `SetMacro`.
///
/// The host writes macro data in chunks of up to `MAX_MACRO_DATA` bytes.
/// A final chunk shorter than `MAX_MACRO_DATA` signals the end of the macro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct SetMacroRequest {
    pub index: u8,
    pub offset: u16,
    pub data: MacroData,
}
