//! Behavior configuration protocol types.

use heapless::Vec;
use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::constants::PROTOCOL_MAX_MACRO_DATA;

/// Protocol-facing behavior configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct BehaviorConfig {
    pub combo_timeout_ms: u16,
    pub oneshot_timeout_ms: u16,
    pub tap_interval_ms: u16,
    pub tap_capslock_interval_ms: u16,
}

/// Raw macro data for a single macro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct MacroData {
    pub data: Vec<u8, { PROTOCOL_MAX_MACRO_DATA }>,
}

impl MaxSize for MacroData {
    const POSTCARD_MAX_SIZE: usize =
        u8::POSTCARD_MAX_SIZE * PROTOCOL_MAX_MACRO_DATA + super::varint_size(PROTOCOL_MAX_MACRO_DATA);
}

// ---------------------------------------------------------------------------
// Bulk transfer types (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(feature = "bulk")]
use crate::combo::ComboConfig;
#[cfg(feature = "bulk")]
use crate::constants::PROTOCOL_MAX_BULK_SIZE;
#[cfg(feature = "bulk")]
use crate::constants::PROTOCOL_MORSE_VEC_SIZE;
#[cfg(feature = "bulk")]
use crate::morse::Morse;

/// Bulk request payload for setting multiple combos at once.
#[cfg(feature = "bulk")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct SetComboBulkRequest {
    pub start_index: u8,
    pub configs: Vec<ComboConfig, PROTOCOL_MAX_BULK_SIZE>,
}

#[cfg(feature = "bulk")]
impl MaxSize for SetComboBulkRequest {
    const POSTCARD_MAX_SIZE: usize = u8::POSTCARD_MAX_SIZE
        + ComboConfig::POSTCARD_MAX_SIZE * PROTOCOL_MAX_BULK_SIZE
        + super::varint_size(PROTOCOL_MAX_BULK_SIZE);
}

/// Bulk response for getting multiple combos at once.
#[cfg(feature = "bulk")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct GetComboBulkResponse {
    pub configs: Vec<ComboConfig, PROTOCOL_MAX_BULK_SIZE>,
}

#[cfg(feature = "bulk")]
impl MaxSize for GetComboBulkResponse {
    const POSTCARD_MAX_SIZE: usize =
        ComboConfig::POSTCARD_MAX_SIZE * PROTOCOL_MAX_BULK_SIZE + super::varint_size(PROTOCOL_MAX_BULK_SIZE);
}

/// Bulk request payload for setting multiple morse configs at once.
#[cfg(feature = "bulk")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Schema)]
pub struct SetMorseBulkRequest {
    pub start_index: u8,
    pub configs: Vec<Morse<PROTOCOL_MORSE_VEC_SIZE>, PROTOCOL_MAX_BULK_SIZE>,
}

#[cfg(feature = "bulk")]
impl MaxSize for SetMorseBulkRequest {
    const POSTCARD_MAX_SIZE: usize = u8::POSTCARD_MAX_SIZE
        + <Morse<PROTOCOL_MORSE_VEC_SIZE>>::POSTCARD_MAX_SIZE * PROTOCOL_MAX_BULK_SIZE
        + super::varint_size(PROTOCOL_MAX_BULK_SIZE);
}

/// Bulk response for getting multiple morse configs at once.
#[cfg(feature = "bulk")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Schema)]
pub struct GetMorseBulkResponse {
    pub configs: Vec<Morse<PROTOCOL_MORSE_VEC_SIZE>, PROTOCOL_MAX_BULK_SIZE>,
}

#[cfg(feature = "bulk")]
impl MaxSize for GetMorseBulkResponse {
    const POSTCARD_MAX_SIZE: usize = <Morse<PROTOCOL_MORSE_VEC_SIZE>>::POSTCARD_MAX_SIZE * PROTOCOL_MAX_BULK_SIZE
        + super::varint_size(PROTOCOL_MAX_BULK_SIZE);
}
