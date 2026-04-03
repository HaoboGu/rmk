//! RMK protocol ICD (Interface Control Document).
//!
//! This module defines the shared type contract between firmware and host for the
//! RMK communication protocol. It contains all endpoint and topic declarations,
//! request/response types, and protocol constants.
//!
//! The protocol uses postcard-rpc's type-level endpoint definitions over COBS-framed
//! byte streams (USB bulk transfer and BLE serial).

mod combo;
mod encoder;
mod fork;
mod keymap;
mod macro_data;
mod morse;
mod status;
mod system;

use postcard_rpc::{TopicDirection, endpoints, topics};

pub use self::combo::*;
pub use self::encoder::*;
pub use self::fork::*;
pub use self::keymap::*;
pub use self::macro_data::*;
pub use self::morse::*;
pub use self::status::*;
pub use self::system::*;
use crate::action::{EncoderAction, KeyAction};
#[cfg(feature = "_ble")]
use crate::battery::BatteryStatus;
#[cfg(feature = "_ble")]
use crate::ble::BleStatus;
use crate::connection::ConnectionType;
use crate::fork::Fork;
use crate::led_indicator::LedIndicator;

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
    | GetCombo    | u8              | ComboConfig         | "combo/get"  |
    | SetCombo    | SetComboRequest | RmkResult   | "combo/set"  |
}

#[cfg(feature = "bulk")]
endpoints! {
    list = COMBO_BULK_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy    | RequestTy           | ResponseTy           | Path              |
    | ----------    | ---------           | ----------           | ----              |
    | GetComboBulk  | GetComboBulkRequest | GetComboBulkResponse | "combo/bulk_get"  |
    | SetComboBulk  | SetComboBulkRequest | RmkResult            | "combo/bulk_set"  |
}

endpoints! {
    list = MORSE_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy | RequestTy       | ResponseTy  | Path         |
    | ---------- | ---------       | ----------  | ----         |
    | GetMorse   | u8              | MorseConfig | "morse/get"  |
    | SetMorse   | SetMorseRequest | RmkResult   | "morse/set"  |
}

#[cfg(feature = "bulk")]
endpoints! {
    list = MORSE_BULK_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy    | RequestTy           | ResponseTy           | Path              |
    | ----------    | ---------           | ----------           | ----              |
    | GetMorseBulk  | GetMorseBulkRequest | GetMorseBulkResponse | "morse/bulk_get"  |
    | SetMorseBulk  | SetMorseBulkRequest | RmkResult            | "morse/bulk_set"  |
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
    | EndpointTy        | RequestTy      | ResponseTy     | Path            |
    | ----------        | ---------      | ----------     | ----            |
    | GetConnectionType | ()             | ConnectionType | "conn/type"     |
    | SetConnectionType | ConnectionType | RmkResult      | "conn/set_type" |
}

#[cfg(feature = "_ble")]
endpoints! {
    list = BLE_CONNECTION_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy       | RequestTy | ResponseTy | Path              |
    | ----------       | --------- | ---------- | ----              |
    | GetBleStatus     | ()        | BleStatus  | "conn/ble"        |
    | SwitchBleProfile | u8        | RmkResult  | "conn/switch_ble" |
    | ClearBleProfile  | u8        | RmkResult  | "conn/clear_ble"  |
}

/// Empty endpoint list for when BLE is not available.
#[cfg(not(feature = "_ble"))]
pub const BLE_CONNECTION_ENDPOINT_LIST: postcard_rpc::EndpointMap = postcard_rpc::EndpointMap {
    types: &[],
    endpoints: &[],
};

endpoints! {
    list = STATUS_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy      | RequestTy | ResponseTy  | Path             |
    | ----------      | --------- | ----------  | ----             |
    | GetCurrentLayer | ()        | u8          | "status/layer"   |
    | GetMatrixState  | ()        | MatrixState | "status/matrix"  |
}

#[cfg(feature = "_ble")]
endpoints! {
    list = BLE_STATUS_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy       | RequestTy | ResponseTy    | Path             |
    | ----------       | --------- | ----------    | ----             |
    | GetBatteryStatus | ()        | BatteryStatus | "status/battery" |
}

#[cfg(not(feature = "_ble"))]
pub const BLE_STATUS_ENDPOINT_LIST: postcard_rpc::EndpointMap = postcard_rpc::EndpointMap {
    types: &[],
    endpoints: &[],
};

#[cfg(all(feature = "_ble", feature = "split"))]
endpoints! {
    list = SPLIT_STATUS_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy          | RequestTy | ResponseTy       | Path                |
    | ----------          | --------- | ----------       | ----                |
    | GetPeripheralStatus | u8        | PeripheralStatus | "status/peripheral" |
}

#[cfg(not(all(feature = "_ble", feature = "split")))]
pub const SPLIT_STATUS_ENDPOINT_LIST: postcard_rpc::EndpointMap = postcard_rpc::EndpointMap {
    types: &[],
    endpoints: &[],
};

/// Build an `EndpointMap` from a list of endpoint group constants.
///
/// Each argument must be a `postcard_rpc::EndpointList` (as produced by `endpoints!`).
/// The standard ICD endpoints are always included automatically.
macro_rules! build_endpoint_map {
    ($($list:expr),* $(,)?) => {
        const {
            use postcard_rpc::postcard_schema::schema::{DataModelType, NamedType};
            use postcard_rpc::{EndpointMap, Key};

            const NULL_KEY: Key = unsafe { Key::from_bytes([0u8; 8]) };
            const NULL_TY: &NamedType = &NamedType {
                name: "",
                ty: &DataModelType::Unit,
            };

            const TYPE_SLICES: &[&[&NamedType]] = &[
                postcard_rpc::standard_icd::STANDARD_ICD_ENDPOINTS.types,
                $($list.types,)*
            ];
            const TYPE_LEN: usize = postcard_rpc::uniques::total_len(TYPE_SLICES);
            const TYPES: [&NamedType; TYPE_LEN] =
                postcard_rpc::uniques::combine_with_copy(TYPE_SLICES, NULL_TY);

            const EP_SLICES: &[&[(&str, Key, Key)]] = &[
                postcard_rpc::standard_icd::STANDARD_ICD_ENDPOINTS.endpoints,
                $($list.endpoints,)*
            ];
            const EP_LEN: usize = postcard_rpc::uniques::total_len(EP_SLICES);
            const EPS: [(&str, Key, Key); EP_LEN] =
                postcard_rpc::uniques::combine_with_copy(EP_SLICES, ("", NULL_KEY, NULL_KEY));

            EndpointMap {
                types: TYPES.as_slice(),
                endpoints: EPS.as_slice(),
            }
        }
    };
}

/// Full endpoint map for the RMK protocol.
///
/// Assembled from smaller endpoint groups to avoid very large const-eval
/// workloads in a single `endpoints!` invocation.
/// When the `bulk` feature is enabled, bulk transfer endpoints are included.
///
/// NOTE: the two `#[cfg]` variants share most of their body. A macro was attempted
/// but `const {}` blocks inside `macro_rules!` hit Rust const-eval limitations.
/// If you add a new endpoint group, update **both** blocks.
#[cfg(not(feature = "bulk"))]
pub const ENDPOINT_LIST: postcard_rpc::EndpointMap = build_endpoint_map!(
    SYSTEM_ENDPOINT_LIST,
    KEYMAP_ENDPOINT_LIST,
    ENCODER_ENDPOINT_LIST,
    MACRO_ENDPOINT_LIST,
    COMBO_ENDPOINT_LIST,
    MORSE_ENDPOINT_LIST,
    FORK_ENDPOINT_LIST,
    BEHAVIOR_ENDPOINT_LIST,
    CONNECTION_ENDPOINT_LIST,
    BLE_CONNECTION_ENDPOINT_LIST,
    STATUS_ENDPOINT_LIST,
    BLE_STATUS_ENDPOINT_LIST,
    SPLIT_STATUS_ENDPOINT_LIST,
);

/// Full endpoint map including bulk transfer endpoints.
#[cfg(feature = "bulk")]
pub const ENDPOINT_LIST: postcard_rpc::EndpointMap = build_endpoint_map!(
    SYSTEM_ENDPOINT_LIST,
    KEYMAP_ENDPOINT_LIST,
    KEYMAP_BULK_ENDPOINT_LIST,
    ENCODER_ENDPOINT_LIST,
    MACRO_ENDPOINT_LIST,
    COMBO_ENDPOINT_LIST,
    COMBO_BULK_ENDPOINT_LIST,
    MORSE_ENDPOINT_LIST,
    MORSE_BULK_ENDPOINT_LIST,
    FORK_ENDPOINT_LIST,
    BEHAVIOR_ENDPOINT_LIST,
    CONNECTION_ENDPOINT_LIST,
    BLE_CONNECTION_ENDPOINT_LIST,
    STATUS_ENDPOINT_LIST,
    BLE_STATUS_ENDPOINT_LIST,
    SPLIT_STATUS_ENDPOINT_LIST,
);

// ---------------------------------------------------------------------------
// Topic declarations
// ---------------------------------------------------------------------------

topics! {
    list = TOPICS_OUT_LIST;
    direction = TopicDirection::ToClient;
    | TopicTy               | MessageTy      | Path               |
    | -------               | ---------      | ----               |
    | LayerChangeTopic      | u8             | "event/layer"      |
    | WpmUpdateTopic        | u16            | "event/wpm"        |
    | ConnectionChangeTopic | ConnectionType | "event/connection" |
    | SleepStateTopic       | bool           | "event/sleep"      |
    | LedIndicatorTopic     | LedIndicator   | "event/led"        |
}

#[cfg(feature = "_ble")]
topics! {
    list = BLE_TOPICS_OUT_LIST;
    direction = TopicDirection::ToClient;
    | TopicTy              | MessageTy     | Path               |
    | -------              | ---------     | ----               |
    | BatteryStatusTopic   | BatteryStatus | "event/battery"    |
    | BleStatusChangeTopic | BleStatus     | "event/ble_status" |
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    extern crate alloc;

    use postcard_rpc::{Endpoint, Key, Topic};
    use serde::{Deserialize, Serialize};

    use super::{ENDPOINT_LIST, TOPICS_OUT_LIST, *};
    use crate::action::{Action, MorseProfile};
    #[cfg(feature = "_ble")]
    use crate::battery::ChargeState;
    #[cfg(feature = "_ble")]
    use crate::ble::BleState;
    use crate::fork::{Fork, StateBits};
    use crate::modifier::ModifierCombination;
    use crate::morse::{Morse, MorsePattern};
    use crate::protocol::Vec;

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
            SetConnectionType::REQ_KEY,
            // Status
            GetCurrentLayer::REQ_KEY,
            GetMatrixState::REQ_KEY,
        ];
        // BLE endpoints (feature-gated)
        #[cfg(feature = "_ble")]
        {
            keys.extend_from_slice(&[
                GetBleStatus::REQ_KEY,
                SwitchBleProfile::REQ_KEY,
                ClearBleProfile::REQ_KEY,
                GetBatteryStatus::REQ_KEY,
            ]);
        }
        // Split + BLE endpoints (feature-gated)
        #[cfg(all(feature = "_ble", feature = "split"))]
        {
            keys.extend_from_slice(&[GetPeripheralStatus::REQ_KEY]);
        }
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
    fn all_topic_keys() -> alloc::vec::Vec<Key> {
        #[allow(unused_mut)]
        let mut keys = alloc::vec![
            LayerChangeTopic::TOPIC_KEY,
            WpmUpdateTopic::TOPIC_KEY,
            ConnectionChangeTopic::TOPIC_KEY,
            SleepStateTopic::TOPIC_KEY,
            LedIndicatorTopic::TOPIC_KEY,
        ];
        #[cfg(feature = "_ble")]
        {
            keys.extend_from_slice(&[
                BatteryStatusTopic::TOPIC_KEY,
                BleStatusChangeTopic::TOPIC_KEY,
            ]);
        }
        keys
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
            lighting_enabled: false,
            is_split: false,
            num_split_peripherals: 0,
            ble_enabled: true,
            num_ble_profiles: 4,
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
            lighting_enabled: false,
            is_split: false,
            num_split_peripherals: 0,
            ble_enabled: false,
            num_ble_profiles: 0,
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
        let mut kp = heapless::Vec::new();
        kp.push((1, 2)).unwrap();
        kp.push((3, 4)).unwrap();
        round_trip(&UnlockChallenge { key_positions: kp });
    }

    #[test]
    fn round_trip_unlock_challenge_empty() {
        round_trip(&UnlockChallenge {
            key_positions: heapless::Vec::new(),
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

    #[cfg(feature = "_ble")]
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

    #[cfg(all(feature = "_ble", feature = "split"))]
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
        let mut data: Vec<u8, { crate::constants::MACRO_DATA_SIZE }> = Vec::new();
        data.extend_from_slice(&[0x01, 0x02, 0x03]).unwrap();
        round_trip(&MacroData { data });
    }

    #[test]
    fn round_trip_macro_data_empty() {
        round_trip(&MacroData {
            data: Vec::new(),
        });
    }

    #[test]
    fn round_trip_get_macro_request() {
        round_trip(&GetMacroRequest { index: 0, offset: 0 });
        round_trip(&GetMacroRequest { index: 3, offset: 256 });
    }

    #[test]
    fn round_trip_set_macro_request() {
        let mut data: Vec<u8, { crate::constants::MACRO_DATA_SIZE }> = Vec::new();
        data.extend_from_slice(&[0x01, 0x02]).unwrap();
        round_trip(&SetMacroRequest {
            index: 1,
            offset: 0,
            data: MacroData { data },
        });
    }

    #[test]
    fn round_trip_combo_config() {
        round_trip(&ComboConfig::new([KeyAction::No], KeyAction::No, Some(1)));
        // Empty combo
        round_trip(&ComboConfig::empty());
    }

    #[test]
    fn round_trip_morse() {
        let morse = Morse {
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
        round_trip(&ConnectionType::Usb); // ConnectionChangeTopic
        round_trip(&true); // SleepStateTopic
        round_trip(&LedIndicator::new()); // LedIndicatorTopic
    }

    #[cfg(feature = "_ble")]
    #[test]
    fn round_trip_ble_topic_payloads() {
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
        let mut actions: Vec<KeyAction, { crate::constants::BULK_SIZE }> = Vec::new();
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
        round_trip(&SetComboRequest {
            index: 3,
            config: ComboConfig::new([KeyAction::No], KeyAction::No, Some(1)),
        });
    }

    #[test]
    fn round_trip_set_morse_request() {
        let mut morse = Morse {
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
        all_keys.extend_from_slice(&all_topic_keys());
        assert_unique_keys(&all_keys, "cross endpoint/topic");
    }

    #[test]
    fn endpoint_list_contains_all_declared() {
        assert!(ENDPOINT_LIST.endpoints.len() >= all_endpoint_keys().len());
    }

    #[test]
    fn topic_list_contains_all_declared() {
        let mut total_topics = TOPICS_OUT_LIST.topics.len();
        #[cfg(feature = "_ble")]
        {
            total_topics += BLE_TOPICS_OUT_LIST.topics.len();
        }
        assert!(total_topics >= all_topic_keys().len());
    }

    // -- Max-capacity round-trip tests --

    #[test]
    fn round_trip_macro_data_max_capacity() {
        let mut data = Vec::new();
        for i in 0..crate::constants::MACRO_DATA_SIZE {
            data.push(i as u8).unwrap();
        }
        round_trip(&MacroData { data });
    }

    #[test]
    fn round_trip_matrix_state_max_capacity() {
        let mut bitmap = heapless::Vec::new();
        for i in 0..super::status::MATRIX_BITMAP_SIZE {
            bitmap.push(i as u8).unwrap();
        }
        round_trip(&MatrixState { pressed_bitmap: bitmap });
    }

    /// Verify that every non-bulk endpoint is present in the combined ENDPOINT_LIST.
    /// This catches divergence when new endpoint groups are added to one cfg block
    /// but not the other.
    #[test]
    fn endpoint_list_contains_all_non_bulk_endpoints() {
        let endpoint_paths: alloc::collections::BTreeSet<&str> =
            ENDPOINT_LIST.endpoints.iter().map(|(path, _, _)| *path).collect();

        let non_bulk_groups: &[&[(&str, Key, Key)]] = &[
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
            BLE_CONNECTION_ENDPOINT_LIST.endpoints,
            STATUS_ENDPOINT_LIST.endpoints,
            BLE_STATUS_ENDPOINT_LIST.endpoints,
            SPLIT_STATUS_ENDPOINT_LIST.endpoints,
        ];

        for group in non_bulk_groups {
            for (path, _, _) in *group {
                assert!(
                    endpoint_paths.contains(path),
                    "Endpoint '{}' missing from ENDPOINT_LIST — update both cfg blocks in mod.rs",
                    path
                );
            }
        }
    }
}
