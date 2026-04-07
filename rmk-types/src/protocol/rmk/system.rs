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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::rmk::test_utils::round_trip;

    #[test]
    fn round_trip_protocol_version() {
        round_trip(&ProtocolVersion { major: 1, minor: 0 });
        round_trip(&ProtocolVersion { major: 255, minor: 255 });
    }

    #[test]
    fn round_trip_device_capabilities() {
        // Populated and all-zero edge cases.
        round_trip(&DeviceCapabilities {
            num_layers: 4,
            num_rows: 6,
            num_cols: 14,
            num_encoders: 2,
            max_combos: 16,
            max_combo_keys: 4,
            max_macros: 32,
            macro_space_size: 2048,
            max_morse: 8,
            max_patterns_per_key: 8,
            max_forks: 4,
            storage_enabled: true,
            lighting_enabled: false,
            is_split: false,
            num_split_peripherals: 0,
            ble_enabled: true,
            num_ble_profiles: 4,
            max_payload_size: 256,
            max_bulk_keys: 8,
            macro_chunk_size: 64,
            bulk_transfer_supported: true,
        });
        round_trip(&DeviceCapabilities {
            num_layers: 0,
            num_rows: 0,
            num_cols: 0,
            num_encoders: 0,
            max_combos: 0,
            max_combo_keys: 0,
            max_macros: 0,
            macro_space_size: 0,
            max_morse: 0,
            max_patterns_per_key: 0,
            max_forks: 0,
            storage_enabled: false,
            lighting_enabled: false,
            is_split: false,
            num_split_peripherals: 0,
            ble_enabled: false,
            num_ble_profiles: 0,
            max_payload_size: 0,
            max_bulk_keys: 0,
            macro_chunk_size: 0,
            bulk_transfer_supported: false,
        });
    }

    #[test]
    fn round_trip_lock_status() {
        round_trip(&LockStatus {
            locked: true,
            awaiting_keys: false,
            remaining_keys: 0,
        });
        round_trip(&LockStatus {
            locked: false,
            awaiting_keys: true,
            remaining_keys: 3,
        });
    }

    #[test]
    fn round_trip_unlock_challenge() {
        let mut kp = Vec::new();
        kp.push((1, 2)).unwrap();
        kp.push((3, 4)).unwrap();
        round_trip(&UnlockChallenge { key_positions: kp });
        round_trip(&UnlockChallenge {
            key_positions: Vec::new(),
        });
    }

    #[test]
    fn round_trip_storage_reset_mode() {
        round_trip(&StorageResetMode::Full);
        round_trip(&StorageResetMode::LayoutOnly);
    }

    #[test]
    fn round_trip_behavior_config() {
        round_trip(&BehaviorConfig {
            combo_timeout_ms: 50,
            oneshot_timeout_ms: 500,
            tap_interval_ms: 200,
            tap_capslock_interval_ms: 20,
        });
    }
}
