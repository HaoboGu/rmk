//! Combo endpoint types.

use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::combo::ComboConfig;

/// Request payload for `SetCombo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct SetComboRequest {
    pub index: u8,
    pub config: ComboConfig,
}

// ---------------------------------------------------------------------------
// Bulk transfer types
// ---------------------------------------------------------------------------

#[cfg(feature = "bulk")]
use heapless::Vec;
#[cfg(feature = "bulk")]
use crate::constants::PROTOCOL_MAX_BULK_SIZE;

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
        + crate::varint_max_size(PROTOCOL_MAX_BULK_SIZE);
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
        ComboConfig::POSTCARD_MAX_SIZE * PROTOCOL_MAX_BULK_SIZE + crate::varint_max_size(PROTOCOL_MAX_BULK_SIZE);
}
