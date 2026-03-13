//! Keymap-related protocol types.

use heapless::Vec;
use serde::{Deserialize, Serialize};

use super::MAX_BULK;
use crate::action::KeyAction;

/// Identifies a specific key position in the keymap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct KeyPosition {
    pub layer: u8,
    pub row: u8,
    pub col: u8,
}

/// Request payload for `SetKeyAction` endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetKeyRequest {
    pub position: KeyPosition,
    pub action: KeyAction,
}

/// Request payload for bulk keymap operations.
///
/// The `count` field is clamped to [`MAX_BULK`](super::MAX_BULK) (512) by the
/// firmware. Requesting more than `MAX_BULK` keys will return at most 512.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BulkRequest {
    pub layer: u8,
    pub start_row: u8,
    pub start_col: u8,
    /// Number of key actions to retrieve. Clamped to `MAX_BULK` (512).
    pub count: u16,
}

/// Response type for bulk keymap operations.
pub type BulkKeyActions = Vec<KeyAction, MAX_BULK>;

/// Request payload for `SetKeymapBulk` endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetKeymapBulkRequest {
    pub request: BulkRequest,
    pub actions: Vec<KeyAction, MAX_BULK>,
}
