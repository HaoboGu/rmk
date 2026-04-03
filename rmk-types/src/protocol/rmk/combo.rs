//! Combo endpoint types.

use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

/// ComboConfig — re-export of `Combo` for protocol-layer naming.
pub type ComboConfig = crate::combo::Combo;

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
use crate::constants::BULK_SIZE;
#[cfg(feature = "bulk")]
use crate::protocol::Vec;

/// Request payload for `GetComboBulk`.
#[cfg(feature = "bulk")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct GetComboBulkRequest {
    pub start_index: u8,
    pub count: u8,
}

/// Bulk request payload for setting multiple combos at once.
#[cfg(feature = "bulk")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct SetComboBulkRequest {
    pub start_index: u8,
    pub configs: Vec<ComboConfig, BULK_SIZE>,
}

#[cfg(feature = "bulk")]
impl MaxSize for SetComboBulkRequest {
    const POSTCARD_MAX_SIZE: usize = u8::POSTCARD_MAX_SIZE
        + <ComboConfig>::POSTCARD_MAX_SIZE * BULK_SIZE
        + crate::varint_max_size(BULK_SIZE);
}

/// Bulk response for getting multiple combos at once.
#[cfg(feature = "bulk")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct GetComboBulkResponse {
    pub configs: Vec<ComboConfig, BULK_SIZE>,
}

#[cfg(feature = "bulk")]
impl MaxSize for GetComboBulkResponse {
    const POSTCARD_MAX_SIZE: usize =
        <ComboConfig>::POSTCARD_MAX_SIZE * BULK_SIZE
            + crate::varint_max_size(BULK_SIZE);
}
