//! RMK protocol ICD (Interface Control Document).
//!
//! This module defines the shared type contract between firmware and host for the
//! RMK communication protocol. It contains all endpoint and topic declarations,
//! request/response types, and protocol constants.
//!
//! The protocol uses postcard-rpc's type-level endpoint definitions over COBS-framed
//! byte streams (USB bulk transfer and BLE serial).

mod config;
mod keymap;
mod request;
mod status;
mod types;

use postcard_rpc::{TopicDirection, endpoints, topics};

pub use self::config::*;
pub use self::keymap::*;
pub use self::request::*;
pub use self::status::*;
pub use self::types::*;
use crate::action::{EncoderAction, KeyAction};
use crate::battery::BatteryStatus;
use crate::ble::BleStatus;
use crate::combo::ComboConfig;
use crate::connection::ConnectionType;
use crate::constants::PROTOCOL_MORSE_VEC_SIZE;
use crate::fork::Fork;
use crate::led_indicator::LedIndicator;
use crate::morse::Morse;

/// Type alias for a Morse configuration with protocol-level Vec capacity.
pub type ProtocolMorse = Morse<PROTOCOL_MORSE_VEC_SIZE>;

// ---------------------------------------------------------------------------
// MaxSize helper (postcard 1.x only implements MaxSize for heapless 0.7,
// but we use heapless 0.9, so Vec-containing types need manual impls)
// ---------------------------------------------------------------------------

pub(crate) use crate::varint_max_size as varint_size;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of key positions in an unlock challenge.
pub const PROTOCOL_MAX_UNLOCK_KEYS: usize = 2;

// ---------------------------------------------------------------------------
// Endpoint declarations
// ---------------------------------------------------------------------------

endpoints! {
    list = SYSTEM_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy      | RequestTy        | ResponseTy          | Path                |
    | ----------      | ---------        | ----------          | ----                |
    | GetVersion      | ()               | ProtocolVersion     | "sys/version"       |
    | GetCapabilities | ()               | DeviceCapabilities  | "sys/caps"          |
    | GetLockStatus   | ()               | LockStatus          | "sys/lock_status"   |
    | UnlockRequest   | ()               | UnlockChallenge     | "sys/unlock"        |
    | LockRequest     | ()               | ()                  | "sys/lock"          |
    | Reboot          | ()               | ()                  | "sys/reboot"        |
    | BootloaderJump  | ()               | ()                  | "sys/bootloader"    |
    | StorageReset    | StorageResetMode | ()                  | "sys/storage_reset" |
}

endpoints! {
    list = KEYMAP_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy      | RequestTy            | ResponseTy          | Path                       |
    | ----------      | ---------            | ----------          | ----                       |
    | GetKeyAction    | KeyPosition          | KeyAction           | "keymap/get"               |
    | SetKeyAction    | SetKeyRequest        | RmkResult           | "keymap/set"               |
    | GetDefaultLayer | ()                   | u8                  | "keymap/default_layer"     |
    | SetDefaultLayer | u8                   | RmkResult           | "keymap/set_default_layer" |
}

#[cfg(feature = "bulk")]
endpoints! {
    list = KEYMAP_BULK_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy      | RequestTy            | ResponseTy          | Path              |
    | ----------      | ---------            | ----------          | ----              |
    | GetKeymapBulk   | BulkRequest          | BulkKeyActions      | "keymap/bulk_get" |
    | SetKeymapBulk   | SetKeymapBulkRequest | RmkResult           | "keymap/bulk_set" |
}

endpoints! {
    list = ENCODER_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy       | RequestTy         | ResponseTy    | Path          |
    | ----------       | ---------         | ----------    | ----          |
    | GetEncoderAction | GetEncoderRequest | EncoderAction | "encoder/get" |
    | SetEncoderAction | SetEncoderRequest | RmkResult     | "encoder/set" |
}

endpoints! {
    list = MACRO_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy   | RequestTy        | ResponseTy | Path          |
    | ----------   | ---------        | ---------- | ----          |
    | GetMacro     | GetMacroRequest  | MacroData  | "macro/get"   |
    | SetMacro     | SetMacroRequest  | RmkResult  | "macro/set"   |
}

endpoints! {
    list = COMBO_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy  | RequestTy       | ResponseTy  | Path         |
    | ----------  | ---------       | ----------  | ----         |
    | GetCombo    | u8              | ComboConfig | "combo/get"  |
    | SetCombo    | SetComboRequest | RmkResult   | "combo/set"  |
}

#[cfg(feature = "bulk")]
endpoints! {
    list = COMBO_BULK_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy    | RequestTy           | ResponseTy         | Path              |
    | ----------    | ---------           | ----------         | ----              |
    | GetComboBulk  | BulkRequest         | GetComboBulkResponse | "combo/bulk_get" |
    | SetComboBulk  | SetComboBulkRequest | RmkResult            | "combo/bulk_set" |
}

endpoints! {
    list = MORSE_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy | RequestTy       | ResponseTy  | Path         |
    | ---------- | ---------       | ----------  | ----         |
    | GetMorse   | u8              | ProtocolMorse | "morse/get"  |
    | SetMorse   | SetMorseRequest | RmkResult   | "morse/set"  |
}

#[cfg(feature = "bulk")]
endpoints! {
    list = MORSE_BULK_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy    | RequestTy           | ResponseTy         | Path              |
    | ----------    | ---------           | ----------         | ----              |
    | GetMorseBulk  | BulkRequest         | GetMorseBulkResponse | "morse/bulk_get" |
    | SetMorseBulk  | SetMorseBulkRequest | RmkResult            | "morse/bulk_set" |
}

endpoints! {
    list = FORK_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy | RequestTy      | ResponseTy | Path        |
    | ---------- | ---------      | ---------- | ----        |
    | GetFork    | u8             | Fork       | "fork/get"  |
    | SetFork    | SetForkRequest | RmkResult  | "fork/set"  |
}

endpoints! {
    list = BEHAVIOR_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy        | RequestTy      | ResponseTy     | Path           |
    | ----------        | ---------      | ----------     | ----           |
    | GetBehaviorConfig | ()             | BehaviorConfig | "behavior/get" |
    | SetBehaviorConfig | BehaviorConfig | RmkResult      | "behavior/set" |
}

endpoints! {
    list = CONNECTION_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy        | RequestTy      | ResponseTy | Path              |
    | ----------        | ---------      | ---------- | ----              |
    | GetConnectionType | ()             | ConnectionType | "conn/type"    |
    | GetBleStatus      | ()             | BleStatus      | "conn/ble"     |
    | SetConnectionType | ConnectionType | RmkResult  | "conn/set_type"   |
    | SwitchBleProfile  | u8             | RmkResult  | "conn/switch_ble" |
    | ClearBleProfile   | u8             | RmkResult  | "conn/clear_ble"  |
}

endpoints! {
    list = STATUS_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy          | RequestTy | ResponseTy       | Path                |
    | ----------          | --------- | ----------       | ----                |
    | GetBatteryStatus    | ()        | BatteryStatus    | "status/battery"    |
    | GetCurrentLayer     | ()        | u8               | "status/layer"      |
    | GetMatrixState      | ()        | MatrixState      | "status/matrix"     |
    | GetPeripheralStatus | u8        | PeripheralStatus | "status/peripheral" |
}

/// Full endpoint map for the RMK protocol.
///
/// This is assembled from smaller endpoint groups to avoid very large const-eval
/// workloads in a single `endpoints!` invocation.
/// When the `bulk` feature is enabled, bulk transfer endpoints are included.
#[cfg(not(feature = "bulk"))]
pub const ENDPOINT_LIST: postcard_rpc::EndpointMap = const {
    use postcard_rpc::postcard_schema::schema::{DataModelType, NamedType};
    use postcard_rpc::{EndpointMap, Key};

    const NULL_KEY: Key = unsafe { Key::from_bytes([0u8; 8]) };
    const NULL_TY: &NamedType = &NamedType {
        name: "",
        ty: &DataModelType::Unit,
    };

    const TYPE_SLICES: &[&[&NamedType]] = &[
        postcard_rpc::standard_icd::STANDARD_ICD_ENDPOINTS.types,
        SYSTEM_ENDPOINT_LIST.types,
        KEYMAP_ENDPOINT_LIST.types,
        ENCODER_ENDPOINT_LIST.types,
        MACRO_ENDPOINT_LIST.types,
        COMBO_ENDPOINT_LIST.types,
        MORSE_ENDPOINT_LIST.types,
        FORK_ENDPOINT_LIST.types,
        BEHAVIOR_ENDPOINT_LIST.types,
        CONNECTION_ENDPOINT_LIST.types,
        STATUS_ENDPOINT_LIST.types,
    ];
    const TYPE_LEN: usize = postcard_rpc::uniques::total_len(TYPE_SLICES);
    const TYPES: [&NamedType; TYPE_LEN] = postcard_rpc::uniques::combine_with_copy(TYPE_SLICES, NULL_TY);

    const ENDPOINT_SLICES: &[&[(&str, Key, Key)]] = &[
        postcard_rpc::standard_icd::STANDARD_ICD_ENDPOINTS.endpoints,
        SYSTEM_ENDPOINT_LIST.endpoints,
        KEYMAP_ENDPOINT_LIST.endpoints,
        ENCODER_ENDPOINT_LIST.endpoints,
        MACRO_ENDPOINT_LIST.endpoints,
        COMBO_ENDPOINT_LIST.endpoints,
        MORSE_ENDPOINT_LIST.endpoints,
        FORK_ENDPOINT_LIST.endpoints,
        BEHAVIOR_ENDPOINT_LIST.endpoints,
        CONNECTION_ENDPOINT_LIST.endpoints,
        STATUS_ENDPOINT_LIST.endpoints,
    ];
    const ENDPOINT_LEN: usize = postcard_rpc::uniques::total_len(ENDPOINT_SLICES);
    const ENDPOINTS: [(&str, Key, Key); ENDPOINT_LEN] =
        postcard_rpc::uniques::combine_with_copy(ENDPOINT_SLICES, ("", NULL_KEY, NULL_KEY));

    EndpointMap {
        types: TYPES.as_slice(),
        endpoints: ENDPOINTS.as_slice(),
    }
};

/// Full endpoint map including bulk transfer endpoints.
#[cfg(feature = "bulk")]
pub const ENDPOINT_LIST: postcard_rpc::EndpointMap = const {
    use postcard_rpc::postcard_schema::schema::{DataModelType, NamedType};
    use postcard_rpc::{EndpointMap, Key};

    const NULL_KEY: Key = unsafe { Key::from_bytes([0u8; 8]) };
    const NULL_TY: &NamedType = &NamedType {
        name: "",
        ty: &DataModelType::Unit,
    };

    const TYPE_SLICES: &[&[&NamedType]] = &[
        postcard_rpc::standard_icd::STANDARD_ICD_ENDPOINTS.types,
        SYSTEM_ENDPOINT_LIST.types,
        KEYMAP_ENDPOINT_LIST.types,
        KEYMAP_BULK_ENDPOINT_LIST.types,
        ENCODER_ENDPOINT_LIST.types,
        MACRO_ENDPOINT_LIST.types,
        COMBO_ENDPOINT_LIST.types,
        COMBO_BULK_ENDPOINT_LIST.types,
        MORSE_ENDPOINT_LIST.types,
        MORSE_BULK_ENDPOINT_LIST.types,
        FORK_ENDPOINT_LIST.types,
        BEHAVIOR_ENDPOINT_LIST.types,
        CONNECTION_ENDPOINT_LIST.types,
        STATUS_ENDPOINT_LIST.types,
    ];
    const TYPE_LEN: usize = postcard_rpc::uniques::total_len(TYPE_SLICES);
    const TYPES: [&NamedType; TYPE_LEN] = postcard_rpc::uniques::combine_with_copy(TYPE_SLICES, NULL_TY);

    const ENDPOINT_SLICES: &[&[(&str, Key, Key)]] = &[
        postcard_rpc::standard_icd::STANDARD_ICD_ENDPOINTS.endpoints,
        SYSTEM_ENDPOINT_LIST.endpoints,
        KEYMAP_ENDPOINT_LIST.endpoints,
        KEYMAP_BULK_ENDPOINT_LIST.endpoints,
        ENCODER_ENDPOINT_LIST.endpoints,
        MACRO_ENDPOINT_LIST.endpoints,
        COMBO_ENDPOINT_LIST.endpoints,
        COMBO_BULK_ENDPOINT_LIST.endpoints,
        MORSE_ENDPOINT_LIST.endpoints,
        MORSE_BULK_ENDPOINT_LIST.endpoints,
        FORK_ENDPOINT_LIST.endpoints,
        BEHAVIOR_ENDPOINT_LIST.endpoints,
        CONNECTION_ENDPOINT_LIST.endpoints,
        STATUS_ENDPOINT_LIST.endpoints,
    ];
    const ENDPOINT_LEN: usize = postcard_rpc::uniques::total_len(ENDPOINT_SLICES);
    const ENDPOINTS: [(&str, Key, Key); ENDPOINT_LEN] =
        postcard_rpc::uniques::combine_with_copy(ENDPOINT_SLICES, ("", NULL_KEY, NULL_KEY));

    EndpointMap {
        types: TYPES.as_slice(),
        endpoints: ENDPOINTS.as_slice(),
    }
};

// ---------------------------------------------------------------------------
// Topic declarations
// ---------------------------------------------------------------------------

topics! {
    list = TOPICS_OUT_LIST;
    direction = TopicDirection::ToClient;
    | TopicTy               | MessageTy              | Path                  |
    | -------               | ---------              | ----                  |
    | LayerChangeTopic      | u8                     | "event/layer"         |
    | WpmUpdateTopic        | u16                    | "event/wpm"           |
    | BatteryStatusTopic     | BatteryStatus          | "event/battery"       |
    | BleStatusChangeTopic  | BleStatus              | "event/ble_status"    |
    | ConnectionChangeTopic | ConnectionType         | "event/connection"    |
    | SleepStateTopic       | bool                   | "event/sleep"         |
    | LedIndicatorTopic     | LedIndicator           | "event/led"           |
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    extern crate alloc;

    use heapless::Vec;
    use postcard_rpc::{Endpoint, Key, Topic};
    use serde::{Deserialize, Serialize};

    use super::{ENDPOINT_LIST, TOPICS_OUT_LIST, *};
    use crate::action::{Action, MorseProfile};
    use crate::battery::ChargeState;
    use crate::ble::BleState;
    use crate::fork::{Fork, StateBits};
    use crate::modifier::ModifierCombination;
    use crate::morse::{Morse, MorsePattern};

    /// Helper: postcard round-trip for a value using a stack buffer.
    fn round_trip<T>(val: &T) -> T
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + core::fmt::Debug,
    {
        let mut buf = [0u8; 1024];
        let bytes = postcard::to_slice(val, &mut buf).expect("serialize");
        let decoded: T = postcard::from_bytes(bytes).expect("deserialize");
        assert_eq!(&decoded, val);
        decoded
    }

    /// Helper: assert no duplicate keys in a slice.
    fn assert_unique_keys(keys: &[Key], label: &str) {
        let mut seen = alloc::collections::BTreeSet::new();
        for key in keys {
            assert!(
                seen.insert(*key),
                "Duplicate {} key detected: {:?}",
                label,
                key.to_bytes()
            );
        }
    }

    /// All endpoint request keys, defined once and shared across collision tests.
    fn all_endpoint_keys() -> alloc::vec::Vec<Key> {
        #[allow(unused_mut)]
        let mut keys = alloc::vec![
            // System
            GetVersion::REQ_KEY,
            GetCapabilities::REQ_KEY,
            GetLockStatus::REQ_KEY,
            UnlockRequest::REQ_KEY,
            LockRequest::REQ_KEY,
            Reboot::REQ_KEY,
            BootloaderJump::REQ_KEY,
            StorageReset::REQ_KEY,
            // Keymap
            GetKeyAction::REQ_KEY,
            SetKeyAction::REQ_KEY,
            GetDefaultLayer::REQ_KEY,
            SetDefaultLayer::REQ_KEY,
            // Encoder
            GetEncoderAction::REQ_KEY,
            SetEncoderAction::REQ_KEY,
            // Macro
            GetMacro::REQ_KEY,
            SetMacro::REQ_KEY,
            // Combo
            GetCombo::REQ_KEY,
            SetCombo::REQ_KEY,
            // Morse
            GetMorse::REQ_KEY,
            SetMorse::REQ_KEY,
            // Fork
            GetFork::REQ_KEY,
            SetFork::REQ_KEY,
            // Behavior
            GetBehaviorConfig::REQ_KEY,
            SetBehaviorConfig::REQ_KEY,
            // Connection
            GetConnectionType::REQ_KEY,
            GetBleStatus::REQ_KEY,
            SetConnectionType::REQ_KEY,
            SwitchBleProfile::REQ_KEY,
            ClearBleProfile::REQ_KEY,
            // Status
            GetBatteryStatus::REQ_KEY,
            GetCurrentLayer::REQ_KEY,
            GetMatrixState::REQ_KEY,
            GetPeripheralStatus::REQ_KEY,
        ];
        // Bulk endpoints (feature-gated)
        #[cfg(feature = "bulk")]
        {
            keys.extend_from_slice(&[
                GetKeymapBulk::REQ_KEY,
                SetKeymapBulk::REQ_KEY,
                GetComboBulk::REQ_KEY,
                SetComboBulk::REQ_KEY,
                GetMorseBulk::REQ_KEY,
                SetMorseBulk::REQ_KEY,
            ]);
        }
        keys
    }

    /// All topic keys, defined once and shared across collision tests.
    fn all_topic_keys() -> &'static [Key] {
        &[
            LayerChangeTopic::TOPIC_KEY,
            WpmUpdateTopic::TOPIC_KEY,
            BatteryStatusTopic::TOPIC_KEY,
            BleStatusChangeTopic::TOPIC_KEY,
            ConnectionChangeTopic::TOPIC_KEY,
            SleepStateTopic::TOPIC_KEY,
            LedIndicatorTopic::TOPIC_KEY,
        ]
    }

    // -- Round-trip tests --

    #[test]
    fn round_trip_protocol_version() {
        round_trip(&ProtocolVersion { major: 1, minor: 0 });
        round_trip(&ProtocolVersion { major: 255, minor: 255 });
    }

    #[test]
    fn round_trip_device_capabilities() {
        let caps = DeviceCapabilities {
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
            is_split: false,
            num_split_peripherals: 0,
            ble_enabled: true,
            num_ble_profiles: 4,
            lighting_enabled: false,
            max_payload_size: 256,
            max_bulk_keys: 8,
            macro_chunk_size: 64,
            bulk_transfer_supported: true,
        };
        round_trip(&caps);
    }

    #[test]
    fn round_trip_device_capabilities_all_zero() {
        let caps = DeviceCapabilities {
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
            is_split: false,
            num_split_peripherals: 0,
            ble_enabled: false,
            num_ble_profiles: 0,
            lighting_enabled: false,
            max_payload_size: 0,
            max_bulk_keys: 0,
            macro_chunk_size: 0,
            bulk_transfer_supported: false,
        };
        round_trip(&caps);
    }

    #[test]
    fn round_trip_rmk_error() {
        round_trip(&RmkError::InvalidParameter);
        round_trip(&RmkError::BadState);
        round_trip(&RmkError::InternalError);
    }

    #[test]
    fn round_trip_rmk_result() {
        let ok: RmkResult = Ok(());
        let err: RmkResult = Err(RmkError::BadState);
        let _ = round_trip(&ok);
        let _ = round_trip(&err);
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
    }

    #[test]
    fn round_trip_unlock_challenge_empty() {
        round_trip(&UnlockChallenge {
            key_positions: Vec::new(),
        });
    }

    #[test]
    fn round_trip_key_position() {
        round_trip(&KeyPosition {
            layer: 0,
            row: 5,
            col: 13,
        });
    }

    #[test]
    fn round_trip_bulk_request() {
        round_trip(&BulkRequest {
            layer: 2,
            start_row: 0,
            start_col: 0,
            count: 32,
        });
    }

    #[test]
    fn round_trip_storage_reset_mode() {
        round_trip(&StorageResetMode::Full);
        round_trip(&StorageResetMode::LayoutOnly);
    }

    #[test]
    fn round_trip_connection_types() {
        round_trip(&ConnectionType::Usb);
        round_trip(&ConnectionType::Ble);
    }

    #[test]
    fn round_trip_battery_status() {
        round_trip(&BatteryStatus::Unavailable);
        round_trip(&BatteryStatus::Available {
            charge_state: ChargeState::Charging,
            level: Some(85),
        });
        round_trip(&BatteryStatus::Available {
            charge_state: ChargeState::Discharging,
            level: Some(50),
        });
        round_trip(&BatteryStatus::Available {
            charge_state: ChargeState::Unknown,
            level: None,
        });
    }

    #[test]
    fn round_trip_matrix_state() {
        let mut bitmap = heapless::Vec::new();
        bitmap.extend_from_slice(&[0b0000_0101, 0x00, 0b0010_0000]).unwrap();
        round_trip(&MatrixState { pressed_bitmap: bitmap });
    }

    #[test]
    fn round_trip_peripheral_status() {
        round_trip(&PeripheralStatus {
            connected: true,
            battery: BatteryStatus::Available {
                charge_state: ChargeState::Discharging,
                level: Some(85),
            },
        });
        round_trip(&PeripheralStatus {
            connected: false,
            battery: BatteryStatus::Unavailable,
        });
    }

    #[test]
    fn round_trip_macro_data() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0x01, 0x02, 0x03]).unwrap();
        round_trip(&MacroData { data });
    }

    #[test]
    fn round_trip_macro_data_empty() {
        round_trip(&MacroData { data: Vec::new() });
    }

    #[test]
    fn round_trip_get_macro_request() {
        round_trip(&GetMacroRequest { index: 0, offset: 0 });
        round_trip(&GetMacroRequest { index: 3, offset: 256 });
    }

    #[test]
    fn round_trip_set_macro_request() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0x01, 0x02]).unwrap();
        round_trip(&SetMacroRequest {
            index: 1,
            offset: 0,
            data: MacroData { data },
        });
    }

    #[test]
    fn round_trip_combo_config() {
        let mut actions = Vec::new();
        actions.push(KeyAction::No).unwrap();
        actions.push(KeyAction::No).unwrap();
        round_trip(&ComboConfig {
            actions,
            output: KeyAction::No,
            layer: Some(1),
        });
    }

    #[test]
    fn round_trip_morse() {
        let morse: Morse<8> = Morse {
            profile: MorseProfile::const_default(),
            actions: heapless::LinearMap::new(),
        };
        round_trip(&morse);
    }

    #[test]
    fn round_trip_fork() {
        round_trip(&Fork::new(
            KeyAction::No,
            KeyAction::No,
            KeyAction::No,
            StateBits::default(),
            StateBits::default(),
            ModifierCombination::new(),
            false,
        ));
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

    #[test]
    fn round_trip_topic_payloads() {
        round_trip(&3u8); // LayerChangeTopic
        round_trip(&120u16); // WpmUpdateTopic
        round_trip(&BatteryStatus::Available {
            charge_state: ChargeState::Discharging,
            level: Some(100),
        });
        round_trip(&BleStatus {
            profile: 0,
            state: BleState::Advertising,
        });
        round_trip(&BleStatus {
            profile: 2,
            state: BleState::Connected,
        });
        round_trip(&BleStatus {
            profile: 0,
            state: BleState::Inactive,
        });
        round_trip(&ConnectionType::Usb); // ConnectionChangeTopic
        round_trip(&true); // SleepStateTopic
        round_trip(&LedIndicator::new()); // LedIndicatorTopic
    }

    #[test]
    fn round_trip_set_key_request() {
        round_trip(&SetKeyRequest {
            position: KeyPosition {
                layer: 0,
                row: 0,
                col: 0,
            },
            action: KeyAction::No,
        });
    }

    #[cfg(feature = "bulk")]
    #[test]
    fn round_trip_set_keymap_bulk_request() {
        let mut actions = Vec::new();
        actions.push(KeyAction::No).unwrap();
        round_trip(&SetKeymapBulkRequest {
            request: BulkRequest {
                layer: 0,
                start_row: 0,
                start_col: 0,
                count: 1,
            },
            actions,
        });
    }

    #[test]
    fn round_trip_encoder_requests() {
        round_trip(&GetEncoderRequest {
            encoder_id: 0,
            layer: 1,
        });
        round_trip(&SetEncoderRequest {
            encoder_id: 0,
            layer: 1,
            action: EncoderAction::default(),
        });
    }

    #[test]
    fn round_trip_set_combo_request() {
        let mut actions = Vec::new();
        actions.push(KeyAction::No).unwrap();
        actions.push(KeyAction::No).unwrap();
        round_trip(&SetComboRequest {
            index: 3,
            config: ComboConfig {
                actions,
                output: KeyAction::No,
                layer: Some(1),
            },
        });
    }

    #[test]
    fn round_trip_set_morse_request() {
        let mut morse: Morse<{ crate::constants::PROTOCOL_MORSE_VEC_SIZE }> = Morse {
            profile: MorseProfile::const_default(),
            actions: heapless::LinearMap::new(),
        };
        morse.actions.insert(MorsePattern::from_u16(0b101), Action::No).unwrap();
        round_trip(&SetMorseRequest {
            index: 0,
            config: morse,
        });
    }

    #[test]
    fn round_trip_set_fork_request() {
        round_trip(&SetForkRequest {
            index: 2,
            config: Fork::new(
                KeyAction::No,
                KeyAction::No,
                KeyAction::No,
                StateBits::default(),
                StateBits::default(),
                ModifierCombination::new(),
                true,
            ),
        });
    }

    #[test]
    fn round_trip_set_encoder_request_with_actions() {
        use crate::action::{Action, EncoderAction};
        use crate::keycode::{ConsumerKey, KeyCode};
        round_trip(&SetEncoderRequest {
            encoder_id: 1,
            layer: 2,
            action: EncoderAction::new(
                KeyAction::Single(Action::Key(KeyCode::Consumer(ConsumerKey::VolumeIncrement))),
                KeyAction::Single(Action::Key(KeyCode::Consumer(ConsumerKey::VolumeDecrement))),
            ),
        });
    }

    // Intra-group collisions are caught at compile time by endpoints!/topics! macros.

    #[test]
    fn no_cross_endpoint_topic_key_collisions() {
        let mut all_keys = all_endpoint_keys();
        all_keys.extend_from_slice(all_topic_keys());
        assert_unique_keys(&all_keys, "cross endpoint/topic");
    }

    #[test]
    fn endpoint_list_contains_all_declared() {
        assert!(ENDPOINT_LIST.endpoints.len() >= all_endpoint_keys().len());
    }

    #[test]
    fn topic_list_contains_all_declared() {
        assert!(TOPICS_OUT_LIST.topics.len() >= all_topic_keys().len());
    }
}
