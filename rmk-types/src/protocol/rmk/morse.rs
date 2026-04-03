//! Morse endpoint types.

use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::constants::PROTOCOL_MORSE_VEC_SIZE;
use crate::morse::Morse;

/// Morse configuration with protocol-level Vec capacity.
pub type MorseConfig = Morse<PROTOCOL_MORSE_VEC_SIZE>;

/// Request payload for `SetMorse`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Schema)]
pub struct SetMorseRequest {
    pub index: u8,
    pub config: Morse<PROTOCOL_MORSE_VEC_SIZE>,
}

impl MaxSize for SetMorseRequest {
    const POSTCARD_MAX_SIZE: usize = u8::POSTCARD_MAX_SIZE + <Morse<PROTOCOL_MORSE_VEC_SIZE>>::POSTCARD_MAX_SIZE;
}

// ---------------------------------------------------------------------------
// Bulk transfer types
// ---------------------------------------------------------------------------

#[cfg(feature = "bulk")]
use crate::constants::PROTOCOL_MAX_BULK_SIZE;
#[cfg(feature = "bulk")]
use crate::protocol_vec::ProtocolVec;

/// Request payload for `GetMorseBulk`.
#[cfg(feature = "bulk")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct GetMorseBulkRequest {
    pub start_index: u8,
    pub count: u8,
}

/// Bulk request payload for setting multiple morse configs at once.
#[cfg(feature = "bulk")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Schema)]
pub struct SetMorseBulkRequest {
    pub start_index: u8,
    pub configs: ProtocolVec<Morse<PROTOCOL_MORSE_VEC_SIZE>, PROTOCOL_MAX_BULK_SIZE>,
}

#[cfg(feature = "bulk")]
impl MaxSize for SetMorseBulkRequest {
    const POSTCARD_MAX_SIZE: usize = u8::POSTCARD_MAX_SIZE
        + <Morse<PROTOCOL_MORSE_VEC_SIZE>>::POSTCARD_MAX_SIZE * PROTOCOL_MAX_BULK_SIZE
        + crate::varint_max_size(PROTOCOL_MAX_BULK_SIZE);
}

/// Bulk response for getting multiple morse configs at once.
#[cfg(feature = "bulk")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Schema)]
pub struct GetMorseBulkResponse {
    pub configs: ProtocolVec<Morse<PROTOCOL_MORSE_VEC_SIZE>, PROTOCOL_MAX_BULK_SIZE>,
}

#[cfg(feature = "bulk")]
impl MaxSize for GetMorseBulkResponse {
    const POSTCARD_MAX_SIZE: usize = <Morse<PROTOCOL_MORSE_VEC_SIZE>>::POSTCARD_MAX_SIZE * PROTOCOL_MAX_BULK_SIZE
        + crate::varint_max_size(PROTOCOL_MAX_BULK_SIZE);
}
