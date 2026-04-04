//! Keymap endpoint types.

use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::action::KeyAction;
#[cfg(feature = "bulk")]
use crate::constants::BULK_SIZE;
#[cfg(feature = "bulk")]
use crate::vec::Vec;

/// Identifies a specific key position in the keymap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct KeyPosition {
    pub layer: u8,
    pub row: u8,
    pub col: u8,
}

/// Request payload for `SetKeyAction` endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct SetKeyRequest {
    pub position: KeyPosition,
    pub action: KeyAction,
}

/// Request payload for bulk keymap operations.
///
/// Keys are linearized in row-major order starting from `(start_row, start_col)`.
/// `count` is the number of keys to read/write; iteration wraps to subsequent
/// rows when the end of a row is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct BulkRequest {
    pub layer: u8,
    pub start_row: u8,
    pub start_col: u8,
    pub count: u8,
}

// ---------------------------------------------------------------------------
// Bulk transfer types
// ---------------------------------------------------------------------------

/// Response type for bulk keymap operations.
#[cfg(feature = "bulk")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[serde(transparent)]
pub struct BulkKeyActions(pub Vec<KeyAction, BULK_SIZE>);

#[cfg(feature = "bulk")]
impl MaxSize for BulkKeyActions {
    const POSTCARD_MAX_SIZE: usize =
        KeyAction::POSTCARD_MAX_SIZE * BULK_SIZE + crate::varint_max_size(BULK_SIZE);
}

/// Request payload for `SetKeymapBulk` endpoint.
#[cfg(feature = "bulk")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct SetKeymapBulkRequest {
    pub request: BulkRequest,
    pub actions: Vec<KeyAction, BULK_SIZE>,
}

#[cfg(feature = "bulk")]
impl MaxSize for SetKeymapBulkRequest {
    const POSTCARD_MAX_SIZE: usize = BulkRequest::POSTCARD_MAX_SIZE
        + KeyAction::POSTCARD_MAX_SIZE * BULK_SIZE
        + crate::varint_max_size(BULK_SIZE);
}
