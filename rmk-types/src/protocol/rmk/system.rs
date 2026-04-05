//! System-level protocol types.
//!
//! Types for protocol handshake, device discovery, security, and global configuration.

use heapless::Vec;
use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

/// Maximum number of key positions in an unlock challenge.
pub const UNLOCK_KEYS_SIZE: usize = 2;

/// Protocol version advertised during the connection handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct ProtocolVersion {
    pub major: u8,
    pub minor: u8,
}

impl ProtocolVersion {
    /// Current protocol version for this firmware release.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

/// Device capabilities discovered during the connection handshake.
///
/// The host reads this once after connecting to learn the firmware's layout,
/// feature set, and protocol limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct DeviceCapabilities {
    // -- Layout --
    pub num_layers: u8,
    pub num_rows: u8,
    pub num_cols: u8,

    // -- Input devices --
    pub num_encoders: u8,
    pub max_combos: u8,
    pub max_combo_keys: u8,
    pub max_macros: u8,
    pub macro_space_size: u16,
    pub max_morse: u8,
    pub max_patterns_per_key: u8,
    pub max_forks: u8,

    // -- Feature flags --
    pub storage_enabled: bool,
    pub lighting_enabled: bool,

    // -- Connectivity --
    pub is_split: bool,
    pub num_split_peripherals: u8,
    pub ble_enabled: bool,
    pub num_ble_profiles: u8,

    // -- Protocol limits --
    pub max_payload_size: u16,
    pub max_bulk_keys: u8,
    pub macro_chunk_size: u16,
    pub bulk_transfer_supported: bool,
}

/// Protocol-level error type returned by write operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub enum RmkError {
    /// The request parameters are invalid or out of range.
    InvalidParameter,
    /// Operation not valid in current device state (e.g. device is locked).
    BadState,
    /// An internal firmware error occurred (storage, contention, etc).
    InternalError,
}

/// Result type for write operations.
///
/// This is a type alias rather than a newtype. `Schema` and `MaxSize` are
/// provided by postcard's blanket impls for `Result<T, E>`. The endpoint
/// key is derived from the schema structure (not the Rust path), so the
/// alias is stable. Cross-endpoint collision tests in this module verify
/// key uniqueness.
pub type RmkResult = Result<(), RmkError>;

/// Current lock/unlock state of the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct LockStatus {
    pub locked: bool,
    pub awaiting_keys: bool,
    pub remaining_keys: u8,
}

/// Challenge returned by the Unlock endpoint containing physical key positions to press.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct UnlockChallenge {
    pub key_positions: Vec<(u8, u8), UNLOCK_KEYS_SIZE>,
}

impl MaxSize for UnlockChallenge {
    const POSTCARD_MAX_SIZE: usize =
        <(u8, u8)>::POSTCARD_MAX_SIZE * UNLOCK_KEYS_SIZE + crate::varint_max_size(UNLOCK_KEYS_SIZE);
}

/// Storage reset mode for the `StorageReset` endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub enum StorageResetMode {
    /// Reset all stored data.
    Full,
    /// Reset only the layout/keymap data.
    LayoutOnly,
}

/// Protocol-facing behavior configuration (global timing settings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct BehaviorConfig {
    pub combo_timeout_ms: u16,
    pub oneshot_timeout_ms: u16,
    pub tap_interval_ms: u16,
    pub tap_capslock_interval_ms: u16,
}
