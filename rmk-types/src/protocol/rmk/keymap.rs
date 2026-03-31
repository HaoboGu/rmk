//! Keymap-related protocol types.

use heapless::Vec;
use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use super::MAX_BULK;
use crate::action::KeyAction;

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

/// Response type for bulk keymap operations.
pub type BulkKeyActions = Vec<KeyAction, MAX_BULK>;

/// Request payload for `SetKeymapBulk` endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct SetKeymapBulkRequest {
    pub request: BulkRequest,
    pub actions: Vec<KeyAction, MAX_BULK>,
}

impl MaxSize for SetKeymapBulkRequest {
    const POSTCARD_MAX_SIZE: usize = BulkRequest::POSTCARD_MAX_SIZE
        + KeyAction::POSTCARD_MAX_SIZE * MAX_BULK
        + super::varint_size(MAX_BULK);
}
