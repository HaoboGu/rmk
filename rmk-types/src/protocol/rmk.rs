//! RMK protocol ICD (Interface Control Document).
//!
//! This module defines the shared type contract between firmware and host for the
//! RMK communication protocol. It contains all endpoint and topic declarations,
//! request/response types, and protocol constants.
//!
//! The protocol uses postcard-rpc's type-level endpoint definitions over COBS-framed
//! byte streams (USB bulk transfer and BLE serial).
use heapless::Vec;
use postcard_rpc::{TopicDirection, endpoints, topics};
use serde::{Deserialize, Serialize};

use crate::action::{EncoderAction, KeyAction, MorseProfile};
use crate::connection::ConnectionType;
pub use crate::event::{
    BatteryStatus, BleStatus, ConnectionPayload, LayerChangePayload, LedPayload, SleepPayload, WpmPayload,
};
use crate::fork::ForkStateBits;
use crate::modifier::ModifierCombination;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of key positions in an unlock challenge.
pub const MAX_UNLOCK_KEYS: usize = 2;

/// Maximum number of key actions in a bulk get/set operation.
pub const MAX_BULK: usize = 32;

/// Maximum number of combo input keys in a protocol combo config.
pub const MAX_COMBO_KEYS: usize = 8;

/// Maximum number of morse pattern/action pairs in a protocol morse config.
pub const MAX_MORSE_PATTERNS: usize = 16;

/// Maximum macro data size for a single macro over the protocol.
pub const MAX_MACRO_DATA: usize = 256;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

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

/// Identifies a specific key position in the keymap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct KeyPosition {
    pub layer: u8,
    pub row: u8,
    pub col: u8,
}

/// Request payload for `SetKeyAction` endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct SetKeyRequest {
    pub position: KeyPosition,
    pub action: KeyAction,
}

/// Request payload for bulk keymap operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct BulkRequest {
    pub layer: u8,
    pub start_row: u8,
    pub start_col: u8,
    pub count: u16,
}

/// Response type for bulk keymap operations.
pub type BulkKeyActions = Vec<KeyAction, MAX_BULK>;

/// Request payload for `SetKeymapBulk` endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct SetKeymapBulkRequest {
    pub request: BulkRequest,
    pub actions: Vec<KeyAction, MAX_BULK>,
}

/// Storage reset mode for the `StorageReset` endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub enum StorageResetMode {
    /// Reset all stored data.
    Full,
    /// Reset only the layout/keymap data.
    LayoutOnly,
}

// ---------------------------------------------------------------------------
// Connection / status types
// ---------------------------------------------------------------------------

/// Current connection information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ConnectionInfo {
    pub connection_type: ConnectionType,
    pub ble_profile: u8,
    pub ble_connected: bool,
}

/// Current matrix key-press state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MatrixState {
    pub num_pressed: u8,
}

/// Split keyboard peripheral status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SplitStatus {
    pub num_peripherals: u8,
    pub connected_peripherals: u8,
}

// ---------------------------------------------------------------------------
// Macro types
// ---------------------------------------------------------------------------

/// Summary information about macro capabilities.
///
/// These fields are also available via [`DeviceCapabilities`]; this type
/// provides a lightweight query for tools that only need macro info.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct MacroInfo {
    pub max_macros: u8,
    pub macro_space_size: u16,
}

impl From<DeviceCapabilities> for MacroInfo {
    fn from(caps: DeviceCapabilities) -> Self {
        Self {
            max_macros: caps.max_macros,
            macro_space_size: caps.macro_space_size,
        }
    }
}

/// Raw macro data for a single macro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct MacroData {
    pub data: Vec<u8, MAX_MACRO_DATA>,
}

// ---------------------------------------------------------------------------
// Combo / Morse / Fork / Behavior config types (protocol-facing)
// ---------------------------------------------------------------------------

/// Protocol-facing combo configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct ComboConfig {
    pub actions: Vec<KeyAction, MAX_COMBO_KEYS>,
    pub output: KeyAction,
    pub layer: Option<u8>,
}

/// Protocol-facing morse/tap-dance configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct MorseConfig {
    pub profile: MorseProfile,
    pub patterns: Vec<MorsePatternEntry, MAX_MORSE_PATTERNS>,
}

/// A single morse pattern/action pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct MorsePatternEntry {
    pub pattern: u16,
    pub action: KeyAction,
}

/// Protocol-facing fork (key override) configuration.
///
/// This mirrors firmware `Fork` fields without reducing match-state dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct ForkConfig {
    pub trigger: KeyAction,
    pub negative_output: KeyAction,
    pub positive_output: KeyAction,
    pub match_any: ForkStateBits,
    pub match_none: ForkStateBits,
    pub kept_modifiers: ModifierCombination,
    pub bindable: bool,
}

/// Protocol-facing behavior configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct BehaviorConfig {
    pub combo_timeout_ms: u16,
    pub oneshot_timeout_ms: u16,
    pub tap_interval_ms: u16,
    pub tap_tolerance: u8,
}

// ---------------------------------------------------------------------------
// Encoder request types
// ---------------------------------------------------------------------------

/// Request payload for `GetEncoderAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct GetEncoderRequest {
    pub encoder_id: u8,
    pub layer: u8,
}

/// Request payload for `SetEncoderAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct SetEncoderRequest {
    pub encoder_id: u8,
    pub layer: u8,
    pub action: EncoderAction,
}

/// Request payload for `SetMacro`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct SetMacroRequest {
    pub index: u8,
    pub data: MacroData,
}

/// Request payload for `SetCombo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct SetComboRequest {
    pub index: u8,
    pub config: ComboConfig,
}

/// Request payload for `SetMorse`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct SetMorseRequest {
    pub index: u8,
    pub config: MorseConfig,
}

/// Request payload for `SetFork`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
pub struct SetForkRequest {
    pub index: u8,
    pub config: ForkConfig,
}

// ---------------------------------------------------------------------------
// Topic payload types (re-exported from crate::event)
// ---------------------------------------------------------------------------

// All topic payload types (LayerChangePayload, WpmPayload, BatteryStatus,
// BleStatus, ConnectionPayload, SleepPayload,
// LedPayload) are re-exported from `crate::event` at the top of this file.

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
    | GetKeymapBulk   | BulkRequest          | BulkKeyActions      | "keymap/bulk_get"          |
    | SetKeymapBulk   | SetKeymapBulkRequest | RmkResult           | "keymap/bulk_set"          |
    | GetLayerCount   | ()                   | u8                  | "keymap/layer_count"       |
    | GetDefaultLayer | ()                   | u8                  | "keymap/default_layer"     |
    | SetDefaultLayer | u8                   | RmkResult           | "keymap/set_default_layer" |
    | ResetKeymap     | ()                   | RmkResult           | "keymap/reset"             |
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
    | EndpointTy   | RequestTy       | ResponseTy | Path          |
    | ----------   | ---------       | ---------- | ----          |
    | GetMacroInfo | ()              | MacroInfo  | "macro/info"  |
    | GetMacro     | u8              | MacroData  | "macro/get"   |
    | SetMacro     | SetMacroRequest | RmkResult  | "macro/set"   |
    | ResetMacros  | ()              | RmkResult  | "macro/reset" |
}

endpoints! {
    list = COMBO_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy  | RequestTy       | ResponseTy  | Path         |
    | ----------  | ---------       | ----------  | ----         |
    | GetCombo    | u8              | ComboConfig | "combo/get"  |
    | SetCombo    | SetComboRequest | RmkResult   | "combo/set"  |
    | ResetCombos | ()              | RmkResult   | "combo/reset"|
}

endpoints! {
    list = MORSE_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy | RequestTy       | ResponseTy  | Path         |
    | ---------- | ---------       | ----------  | ----         |
    | GetMorse   | u8              | MorseConfig | "morse/get"  |
    | SetMorse   | SetMorseRequest | RmkResult   | "morse/set"  |
    | ResetMorse | ()              | RmkResult   | "morse/reset"|
}

endpoints! {
    list = FORK_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy | RequestTy      | ResponseTy | Path        |
    | ---------- | ---------      | ---------- | ----        |
    | GetFork    | u8             | ForkConfig | "fork/get"  |
    | SetFork    | SetForkRequest | RmkResult  | "fork/set"  |
    | ResetForks | ()             | RmkResult  | "fork/reset"|
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
    | GetConnectionInfo | ()             | ConnectionInfo | "conn/info"    |
    | SetConnectionType | ConnectionType | RmkResult  | "conn/set_type"   |
    | SwitchBleProfile  | u8             | RmkResult  | "conn/switch_ble" |
    | ClearBleProfile   | u8             | RmkResult  | "conn/clear_ble"  |
}

endpoints! {
    list = STATUS_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy       | RequestTy | ResponseTy    | Path             |
    | ----------       | --------- | ----------    | ----             |
    | GetBatteryStatus | ()        | BatteryStatus | "status/battery" |
    | GetCurrentLayer  | ()        | u8            | "status/layer"   |
    | GetMatrixState   | ()        | MatrixState   | "status/matrix"  |
    | GetSplitStatus   | ()        | SplitStatus   | "status/split"   |
}

/// Full endpoint map for the RMK protocol.
///
/// This is assembled from smaller endpoint groups to avoid very large const-eval
/// workloads in a single `endpoints!` invocation.
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

// ---------------------------------------------------------------------------
// Topic declarations
// ---------------------------------------------------------------------------

topics! {
    list = TOPICS_OUT_LIST;
    direction = TopicDirection::ToClient;
    | TopicTy               | MessageTy              | Path                  |
    | -------               | ---------              | ----                  |
    | LayerChangeTopic      | LayerChangePayload     | "event/layer"         |
    | WpmUpdateTopic        | WpmPayload             | "event/wpm"           |
    | BatteryStatusTopic     | BatteryStatus          | "event/battery"       |
    | BleStatusChangeTopic  | BleStatus              | "event/ble_status"    |
    | ConnectionChangeTopic | ConnectionPayload      | "event/connection"    |
    | SleepStateTopic       | SleepPayload           | "event/sleep"         |
    | LedIndicatorTopic     | LedPayload             | "event/led"           |
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    extern crate alloc;

    use super::ENDPOINT_LIST;
    use super::TOPICS_OUT_LIST;
    use postcard_rpc::{Endpoint, Key, Topic};

    use super::*;
    use crate::event::{BleState, ChargeState};
    use crate::led_indicator::LedIndicator;
    use crate::mouse_button::MouseButtons;

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
    fn all_endpoint_keys() -> &'static [Key] {
        &[
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
            GetKeymapBulk::REQ_KEY,
            SetKeymapBulk::REQ_KEY,
            GetLayerCount::REQ_KEY,
            GetDefaultLayer::REQ_KEY,
            SetDefaultLayer::REQ_KEY,
            ResetKeymap::REQ_KEY,
            // Encoder
            GetEncoderAction::REQ_KEY,
            SetEncoderAction::REQ_KEY,
            // Macro
            GetMacroInfo::REQ_KEY,
            GetMacro::REQ_KEY,
            SetMacro::REQ_KEY,
            ResetMacros::REQ_KEY,
            // Combo
            GetCombo::REQ_KEY,
            SetCombo::REQ_KEY,
            ResetCombos::REQ_KEY,
            // Morse
            GetMorse::REQ_KEY,
            SetMorse::REQ_KEY,
            ResetMorse::REQ_KEY,
            // Fork
            GetFork::REQ_KEY,
            SetFork::REQ_KEY,
            ResetForks::REQ_KEY,
            // Behavior
            GetBehaviorConfig::REQ_KEY,
            SetBehaviorConfig::REQ_KEY,
            // Connection
            GetConnectionInfo::REQ_KEY,
            SetConnectionType::REQ_KEY,
            SwitchBleProfile::REQ_KEY,
            ClearBleProfile::REQ_KEY,
            // Status
            GetBatteryStatus::REQ_KEY,
            GetCurrentLayer::REQ_KEY,
            GetMatrixState::REQ_KEY,
            GetSplitStatus::REQ_KEY,
        ]
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
            max_macros: 32,
            macro_space_size: 2048,
            max_morse: 8,
            max_forks: 4,
            has_storage: true,
            has_split: false,
            num_split_peripherals: 0,
            has_ble: true,
            num_ble_profiles: 4,
            has_lighting: false,
            max_payload_size: 256,
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
            max_macros: 0,
            macro_space_size: 0,
            max_morse: 0,
            max_forks: 0,
            has_storage: false,
            has_split: false,
            num_split_peripherals: 0,
            has_ble: false,
            num_ble_profiles: 0,
            has_lighting: false,
            max_payload_size: 0,
        };
        round_trip(&caps);
    }

    #[test]
    fn round_trip_rmk_error() {
        round_trip(&RmkError::InvalidParameter);
        round_trip(&RmkError::BadState);
        round_trip(&RmkError::Busy);
        round_trip(&RmkError::StorageError);
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
        round_trip(&ConnectionInfo {
            connection_type: ConnectionType::Ble,
            ble_profile: 2,
            ble_connected: true,
        });
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
        round_trip(&MatrixState { num_pressed: 3 });
    }

    #[test]
    fn round_trip_split_status() {
        round_trip(&SplitStatus {
            num_peripherals: 1,
            connected_peripherals: 1,
        });
    }

    #[test]
    fn round_trip_macro_types() {
        round_trip(&MacroInfo {
            max_macros: 32,
            macro_space_size: 2048,
        });
        let mut data = Vec::new();
        data.extend_from_slice(&[0x01, 0x02, 0x03]).unwrap();
        round_trip(&MacroData { data });
    }

    #[test]
    fn round_trip_macro_data_empty() {
        round_trip(&MacroData { data: Vec::new() });
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
    fn round_trip_morse_config() {
        round_trip(&MorseConfig {
            profile: MorseProfile::const_default(),
            patterns: Vec::new(),
        });
    }

    #[test]
    fn round_trip_fork_config() {
        round_trip(&ForkConfig {
            trigger: KeyAction::No,
            negative_output: KeyAction::No,
            positive_output: KeyAction::No,
            match_any: ForkStateBits {
                modifiers: ModifierCombination::new(),
                leds: LedIndicator::new(),
                mouse: MouseButtons::new(),
            },
            match_none: ForkStateBits {
                modifiers: ModifierCombination::new(),
                leds: LedIndicator::new(),
                mouse: MouseButtons::new(),
            },
            kept_modifiers: ModifierCombination::new(),
            bindable: false,
        });
    }

    #[test]
    fn round_trip_behavior_config() {
        round_trip(&BehaviorConfig {
            combo_timeout_ms: 50,
            oneshot_timeout_ms: 500,
            tap_interval_ms: 200,
            tap_tolerance: 3,
        });
    }

    #[test]
    fn round_trip_topic_payloads() {
        round_trip(&LayerChangePayload { layer: 3 });
        round_trip(&WpmPayload { wpm: 120 });
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
        round_trip(&ConnectionPayload {
            connection_type: ConnectionType::Usb,
        });
        round_trip(&SleepPayload { sleeping: true });
        round_trip(&LedPayload {
            indicator: LedIndicator::new(),
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

    // Intra-group collisions are caught at compile time by endpoints!/topics! macros.

    #[test]
    fn no_cross_endpoint_topic_key_collisions() {
        let mut all_keys = alloc::vec::Vec::new();
        all_keys.extend_from_slice(all_endpoint_keys());
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
