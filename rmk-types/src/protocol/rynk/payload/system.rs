//! System-level protocol types.
//!
//! Types for protocol handshake, device discovery, security, and global configuration.

use heapless::{String, Vec};
use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

/// Maximum number of key positions in an unlock challenge.
pub const UNLOCK_KEYS_SIZE: usize = 2;

/// Maximum byte length of each `DeviceInfo` string field.
pub const DEVICE_INFO_STRING_SIZE: usize = 32;

/// Protocol version advertised during the connection handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ProtocolVersion {
    pub major: u8,
    pub minor: u8,
}

impl ProtocolVersion {
    /// Current protocol version for this firmware release.
    /// Now the protocol is still being developed, so the version is v0.1
    pub const CURRENT: Self = Self { major: 0, minor: 1 };
}

/// Device capabilities discovered during the connection handshake.
///
/// The host reads this once after connecting to learn the firmware's layout,
/// feature set, and protocol limits.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct DeviceCapabilities {
    // -- Layout --
    pub num_layers: u8,
    pub num_rows: u8,
    pub num_cols: u8,

    // -- Input devices --
    pub num_encoders: u8,
    pub max_combos: u8,
    pub max_combo_keys: u8,
    /// Number of macros supported, set by the user at compile time. `0`
    /// disables macros: the host MUST NOT use them or consult `macro_space_size`.
    pub max_macros: u8,
    /// Byte size of the flat macro region; only meaningful when `max_macros > 0`.
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

/// Version of the `rmk` crate baked into the firmware, so hosts can key
/// version-specific behavior off the library release, not the user's app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct FirmwareVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

/// Device identity for display and per-device host profiles.
///
/// Complements [`DeviceCapabilities`]: capabilities answer "what can you do"
/// for feature gating, identity answers "which device is this".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct DeviceInfo {
    pub rmk_version: FirmwareVersion,
    pub vendor_id: u16,
    pub product_id: u16,
    #[cfg_attr(feature = "wasm", tsify(type = "string"))]
    pub manufacturer: String<DEVICE_INFO_STRING_SIZE>,
    #[cfg_attr(feature = "wasm", tsify(type = "string"))]
    pub product_name: String<DEVICE_INFO_STRING_SIZE>,
    #[cfg_attr(feature = "wasm", tsify(type = "string"))]
    pub serial_number: String<DEVICE_INFO_STRING_SIZE>,
}

impl MaxSize for DeviceInfo {
    // A str encodes as varint length + UTF-8 bytes — the same wire shape as
    // `Vec<u8, N>`, so the Vec bound covers each string field.
    const POSTCARD_MAX_SIZE: usize = FirmwareVersion::POSTCARD_MAX_SIZE
        + 2 * u16::POSTCARD_MAX_SIZE
        + 3 * crate::heapless_vec_max_size::<u8, DEVICE_INFO_STRING_SIZE>();
}

/// Current lock/unlock state of the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LockStatus {
    pub locked: bool,
    pub awaiting_keys: bool,
    pub remaining_keys: u8,
}

/// Challenge returned by the Unlock endpoint containing physical key positions to press.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct UnlockChallenge {
    #[cfg_attr(feature = "wasm", tsify(type = "[number, number][]"))]
    pub key_positions: Vec<(u8, u8), UNLOCK_KEYS_SIZE>,
}

impl MaxSize for UnlockChallenge {
    const POSTCARD_MAX_SIZE: usize = crate::heapless_vec_max_size::<(u8, u8), UNLOCK_KEYS_SIZE>();
}

/// Storage reset mode for the `StorageReset` endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[non_exhaustive]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum StorageResetMode {
    /// Reset all stored data — including keymap and BLE bonds.
    Full,
    /// Reset only the layout/keymap data, preserving BLE bonds.
    LayoutOnly,
}

/// Protocol-facing behavior configuration (global timing settings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct BehaviorConfig {
    pub combo_timeout_ms: u16,
    pub oneshot_timeout_ms: u16,
    pub tap_interval_ms: u16,
    pub tap_capslock_interval_ms: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::rynk::tests::{assert_max_size_bound, round_trip};

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
    fn round_trip_device_info() {
        // Max-capacity case: each string filled to 32 bytes with 4-byte chars
        // and both ids at u16::MAX, so every varint takes its full width and
        // the bound is genuinely exercised.
        let full: String<DEVICE_INFO_STRING_SIZE> = String::try_from("🦀🦀🦀🦀🦀🦀🦀🦀").unwrap();
        assert_eq!(full.len(), DEVICE_INFO_STRING_SIZE);
        let info = DeviceInfo {
            rmk_version: FirmwareVersion {
                major: 255,
                minor: 255,
                patch: 255,
            },
            vendor_id: u16::MAX,
            product_id: u16::MAX,
            manufacturer: full.clone(),
            product_name: full.clone(),
            serial_number: full,
        };
        round_trip(&info);
        assert_max_size_bound(&info);
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

        // Max-capacity case: every (u8, u8) at u8::MAX so each varint takes
        // its full width and the bound is genuinely exercised.
        let mut full = Vec::new();
        for _ in 0..UNLOCK_KEYS_SIZE {
            full.push((u8::MAX, u8::MAX)).unwrap();
        }
        let challenge = UnlockChallenge { key_positions: full };
        round_trip(&challenge);
        assert_max_size_bound(&challenge);
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
