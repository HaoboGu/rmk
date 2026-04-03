//! Combo endpoint types.

use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::combo::ComboConfig;
use crate::constants::COMBO_VEC_SIZE;

/// ComboConfig instantiated with protocol-level Vec capacity.
pub type ProtocolComboConfig = ComboConfig<COMBO_VEC_SIZE>;

/// Request payload for `SetCombo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct SetComboRequest {
    pub index: u8,
    pub config: ComboConfig<COMBO_VEC_SIZE>,
}

// ---------------------------------------------------------------------------
// Bulk transfer types
// ---------------------------------------------------------------------------

#[cfg(feature = "bulk")]
use crate::constants::PROTOCOL_MAX_BULK_SIZE;
#[cfg(feature = "bulk")]
use crate::protocol_vec::ProtocolVec;

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
    pub configs: ProtocolVec<ComboConfig<COMBO_VEC_SIZE>, PROTOCOL_MAX_BULK_SIZE>,
}

#[cfg(feature = "bulk")]
impl MaxSize for SetComboBulkRequest {
    const POSTCARD_MAX_SIZE: usize = u8::POSTCARD_MAX_SIZE
        + <ComboConfig<COMBO_VEC_SIZE>>::POSTCARD_MAX_SIZE * PROTOCOL_MAX_BULK_SIZE
        + crate::varint_max_size(PROTOCOL_MAX_BULK_SIZE);
}

/// Bulk response for getting multiple combos at once.
#[cfg(feature = "bulk")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct GetComboBulkResponse {
    pub configs: ProtocolVec<ComboConfig<COMBO_VEC_SIZE>, PROTOCOL_MAX_BULK_SIZE>,
}

#[cfg(feature = "bulk")]
impl MaxSize for GetComboBulkResponse {
    const POSTCARD_MAX_SIZE: usize =
        <ComboConfig<COMBO_VEC_SIZE>>::POSTCARD_MAX_SIZE * PROTOCOL_MAX_BULK_SIZE
            + crate::varint_max_size(PROTOCOL_MAX_BULK_SIZE);
}
