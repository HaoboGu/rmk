//! Morse endpoint types.

use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::morse::Morse;

/// Request payload for `SetMorse`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct SetMorseRequest {
    pub index: u8,
    pub config: Morse,
}

// ---------------------------------------------------------------------------
// Bulk transfer types
// ---------------------------------------------------------------------------

#[cfg(feature = "bulk")]
use heapless::Vec;

#[cfg(feature = "bulk")]
use crate::constants::BULK_SIZE;

/// Request payload for `GetMorseBulk`.
#[cfg(feature = "bulk")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct GetMorseBulkRequest {
    pub start_index: u8,
    pub count: u8,
}

/// Bulk request payload for setting multiple morse configs at once.
#[cfg(feature = "bulk")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct SetMorseBulkRequest {
    pub start_index: u8,
    pub configs: Vec<Morse, BULK_SIZE>,
}

#[cfg(feature = "bulk")]
impl MaxSize for SetMorseBulkRequest {
    const POSTCARD_MAX_SIZE: usize =
        u8::POSTCARD_MAX_SIZE + <Morse>::POSTCARD_MAX_SIZE * BULK_SIZE + crate::varint_max_size(BULK_SIZE);
}

/// Bulk response for getting multiple morse configs at once.
#[cfg(feature = "bulk")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct GetMorseBulkResponse {
    pub configs: Vec<Morse, BULK_SIZE>,
}

#[cfg(feature = "bulk")]
impl MaxSize for GetMorseBulkResponse {
    const POSTCARD_MAX_SIZE: usize = <Morse>::POSTCARD_MAX_SIZE * BULK_SIZE + crate::varint_max_size(BULK_SIZE);
}
