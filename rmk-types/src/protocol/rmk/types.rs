//! Core protocol types.

use serde::{Deserialize, Serialize};

use super::MAX_UNLOCK_KEYS;
use heapless::Vec;

/// Protocol version advertised during the connection handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ProtocolVersion {
    pub major: u8,
    pub minor: u8,
}

impl ProtocolVersion {
    /// Current protocol version for this firmware release.
    pub const CURRENT: Self = Self { major: 1, minor: 1 };

    /// Check if this version is backward-compatible with another.
    /// Versions are compatible if they share the same major version and
    /// this version's minor is >= the other's minor.
    ///
    /// The newer side calls this with the older side's version. For example,
    /// the host tool checks:
    /// `ProtocolVersion::CURRENT.is_backward_compatible_with(&firmware_version)`
    pub fn is_backward_compatible_with(&self, other: &Self) -> bool {
        self.major == other.major && self.minor >= other.minor
    }
}

impl Default for ProtocolVersion {
    fn default() -> Self {
        Self::CURRENT
    }
}

/// Device capabilities discovered during the connection handshake.
///
/// Includes the protocol version so the host can discover capabilities and
/// version in a single round-trip via `GetCapabilities`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct DeviceCapabilities {
    pub protocol_version: ProtocolVersion,
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
    /// Maximum incoming frame size (receive buffer) in bytes.
    /// The host should not send frames larger than this value.
    pub max_payload_size: u16,
}

/// Protocol-level error type returned by write operations.
///
/// This enum is `#[non_exhaustive]`: future firmware may add new variants.
/// Host tools must include a wildcard match arm.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum RmkError {
    /// Valid endpoint but bad parameter values.
    InvalidParameter,
    /// Operation not valid in current state (e.g. device is locked).
    BadState,
    /// Device is locked; unlock required before this operation.
    Locked,
    /// Temporary contention, retry recommended.
    Busy,
    /// Flash read/write failure.
    StorageError,
    /// Unexpected firmware error.
    InternalError,
}

impl core::fmt::Display for RmkError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RmkError::InvalidParameter => write!(f, "invalid parameter"),
            RmkError::BadState => write!(f, "bad state"),
            RmkError::Locked => write!(f, "device is locked"),
            RmkError::Busy => write!(f, "busy, retry recommended"),
            RmkError::StorageError => write!(f, "storage error"),
            RmkError::InternalError => write!(f, "internal firmware error"),
            // Future variants from newer firmware — display the Debug repr
            #[allow(unreachable_patterns)]
            other => write!(f, "unknown error: {:?}", other),
        }
    }
}

/// Result type for write operations.
pub type RmkResult = Result<(), RmkError>;

/// Current lock/unlock state of the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct LockStatus {
    pub locked: bool,
    pub awaiting_keys: bool,
    pub remaining_keys: u8,
}

/// Challenge returned by the Unlock endpoint containing physical key positions to press.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct UnlockChallenge {
    /// Key positions (row, col) that must be pressed to unlock.
    pub key_positions: Vec<(u8, u8), MAX_UNLOCK_KEYS>,
}

/// Storage reset mode for the `StorageReset` endpoint.
///
/// This enum is `#[non_exhaustive]`: future firmware may add new variants.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum StorageResetMode {
    /// Reset all stored data.
    Full,
    /// Reset only the layout/keymap data.
    ///
    /// **Note:** Layout-only reset is not yet implemented in firmware.
    /// The current firmware falls back to a full erase, which also clears
    /// BLE bonding information and behavior configuration.
    LayoutOnly,
}
