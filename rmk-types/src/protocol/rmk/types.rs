//! Core protocol types.

use heapless::Vec;
use serde::{Deserialize, Serialize};

use super::MAX_UNLOCK_KEYS;

/// Protocol version advertised during the connection handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct ProtocolVersion {
    pub major: u8,
    pub minor: u8,
}

impl ProtocolVersion {
    /// Current protocol version for this firmware release.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

/// Device capabilities discovered during the connection handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct DeviceCapabilities {
    pub num_layers: u8,
    pub num_rows: u8,
    pub num_cols: u8,
    pub num_encoders: u8,
    pub max_combos: u8,
    pub max_macros: u8,
    pub macro_space_size: u16,
    pub max_morse: u8,
    pub max_forks: u8,
    pub has_storage: bool,
    pub has_split: bool,
    pub num_split_peripherals: u8,
    pub has_ble: bool,
    pub num_ble_profiles: u8,
    pub has_lighting: bool,
    pub max_payload_size: u16,
}

/// Protocol-level error type returned by write operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub enum RmkError {
    /// Valid endpoint but bad parameter values.
    InvalidParameter,
    /// Operation not valid in current state (e.g. device is locked).
    BadState,
    /// Temporary contention, retry recommended.
    Busy,
    /// Flash read/write failure.
    StorageError,
    /// Unexpected firmware error.
    InternalError,
}

/// Result type for write operations.
pub type RmkResult = Result<(), RmkError>;

/// Current lock/unlock state of the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct LockStatus {
    pub locked: bool,
    pub awaiting_keys: bool,
    pub remaining_keys: u8,
}

/// Challenge returned by the Unlock endpoint containing physical key positions to press.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct UnlockChallenge {
    pub key_positions: Vec<(u8, u8), MAX_UNLOCK_KEYS>,
}

/// Storage reset mode for the `StorageReset` endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub enum StorageResetMode {
    /// Reset all stored data.
    Full,
    /// Reset only the layout/keymap data.
    LayoutOnly,
}
