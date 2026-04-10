//! Endpoint declarations for the RMK protocol.
//!
//! Each `endpoints!` block declares one logical group. Feature-gated groups
//! provide an empty fallback constant when their feature is disabled, so the
//! combined `ENDPOINT_LIST` always type-checks.
//!
//! ## Adding a new conditional endpoint group
//!
//! 1. Add the `endpoints!` block with its `#[cfg(...)]` guard.
//! 2. Add a `#[cfg(not(...))]` empty fallback constant (`EndpointMap` with empty slices).
//! 3. Add the list to the single `ENDPOINT_LIST` definition.
//! 4. Add the group's endpoints to `endpoint_list_contains_all_non_bulk_endpoints` test.
//! 5. If feature-gated, add the group to the matching `endpoint_keys_*_locked` snapshot test.

// The postcard-rpc endpoints! macro performs heavy const-eval for type uniqueness checks.
#![allow(long_running_const_eval)]

use postcard_rpc::endpoints;

use super::*;
use crate::action::{EncoderAction, KeyAction};
#[cfg(feature = "_ble")]
use crate::battery::BatteryStatus;
#[cfg(feature = "_ble")]
use crate::ble::BleStatus;
use crate::combo::Combo;
use crate::connection::ConnectionType;
use crate::fork::Fork;
use crate::morse::Morse;

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
    | EndpointTy      | RequestTy            | ResponseTy            | Path              |
    | ----------      | ---------            | ----------            | ----              |
    | GetKeymapBulk   | GetKeymapBulkRequest | GetKeymapBulkResponse | "keymap/bulk_get" |
    | SetKeymapBulk   | SetKeymapBulkRequest | RmkResult             | "keymap/bulk_set" |
}

#[cfg(not(feature = "bulk"))]
pub const KEYMAP_BULK_ENDPOINT_LIST: postcard_rpc::EndpointMap = postcard_rpc::EndpointMap {
    types: &[],
    endpoints: &[],
};

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
    | GetCombo    | u8              | Combo       | "combo/get"  |
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

#[cfg(not(feature = "bulk"))]
pub const COMBO_BULK_ENDPOINT_LIST: postcard_rpc::EndpointMap = postcard_rpc::EndpointMap {
    types: &[],
    endpoints: &[],
};

endpoints! {
    list = MORSE_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy | RequestTy       | ResponseTy  | Path         |
    | ---------- | ---------       | ----------  | ----         |
    | GetMorse   | u8              | Morse       | "morse/get"  |
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

#[cfg(not(feature = "bulk"))]
pub const MORSE_BULK_ENDPOINT_LIST: postcard_rpc::EndpointMap = postcard_rpc::EndpointMap {
    types: &[],
    endpoints: &[],
};

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

endpoints! {
    list = STATUS_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy      | RequestTy | ResponseTy  | Path                |
    | ----------      | --------- | ----------  | ----                |
    | GetCurrentLayer | ()        | u8          | "status/layer/get"  |
    | GetMatrixState  | ()        | MatrixState | "status/matrix/get" |
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

#[cfg(feature = "_ble")]
endpoints! {
    list = BLE_STATUS_ENDPOINT_LIST;
    omit_std = true;
    | EndpointTy       | RequestTy | ResponseTy    | Path                 |
    | ----------       | --------- | ----------    | ----                 |
    | GetBatteryStatus | ()        | BatteryStatus | "status/battery/get" |
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
    | EndpointTy          | RequestTy | ResponseTy       | Path                    |
    | ----------          | --------- | ----------       | ----                    |
    | GetPeripheralStatus | u8        | PeripheralStatus | "status/peripheral/get" |
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
/// Feature-gated groups (bulk, BLE, split) use empty fallback constants
/// when the corresponding feature is disabled.
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

#[cfg(test)]
mod tests {
    extern crate alloc;

    use postcard_rpc::Key;

    use super::*;
    use crate::protocol::rmk::snapshot;

    /// Collect (path, req_bytes, resp_bytes) entries for a set of endpoint groups.
    fn collect<'a>(groups: &[&'a [(&'a str, Key, Key)]]) -> alloc::vec::Vec<(&'a str, [u8; 8], [u8; 8])> {
        groups
            .iter()
            .flat_map(|g| g.iter())
            .map(|(path, req, resp)| (*path, req.to_bytes(), resp.to_bytes()))
            .collect()
    }

    /// Lock down endpoint schema fingerprints for the always-on groups.
    /// Each Key is an 8-byte hash of (path, postcard schema of req/resp).
    /// Any change to a request/response type — including transitively-referenced
    /// types — flips the corresponding Key, and this snapshot fails.
    /// Update the snapshot intentionally with `UPDATE_SNAPSHOTS=1`.
    #[test]
    fn endpoint_keys_base_locked() {
        let entries = collect(&[
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
        ]);
        let actual = snapshot::format_endpoint_keys("snapshots/endpoint_keys_base.snap", &entries);
        snapshot::assert_snapshot("snapshots/endpoint_keys_base.snap", actual);
    }

    #[cfg(feature = "bulk")]
    #[test]
    fn endpoint_keys_bulk_locked() {
        let entries = collect(&[
            KEYMAP_BULK_ENDPOINT_LIST.endpoints,
            COMBO_BULK_ENDPOINT_LIST.endpoints,
            MORSE_BULK_ENDPOINT_LIST.endpoints,
        ]);
        let actual = snapshot::format_endpoint_keys("snapshots/endpoint_keys_bulk.snap", &entries);
        snapshot::assert_snapshot("snapshots/endpoint_keys_bulk.snap", actual);
    }

    #[cfg(feature = "_ble")]
    #[test]
    fn endpoint_keys_ble_locked() {
        let entries = collect(&[
            BLE_CONNECTION_ENDPOINT_LIST.endpoints,
            BLE_STATUS_ENDPOINT_LIST.endpoints,
        ]);
        let actual = snapshot::format_endpoint_keys("snapshots/endpoint_keys_ble.snap", &entries);
        snapshot::assert_snapshot("snapshots/endpoint_keys_ble.snap", actual);
    }

    #[cfg(all(feature = "_ble", feature = "split"))]
    #[test]
    fn endpoint_keys_ble_split_locked() {
        let entries = collect(&[SPLIT_STATUS_ENDPOINT_LIST.endpoints]);
        let actual = snapshot::format_endpoint_keys("snapshots/endpoint_keys_ble_split.snap", &entries);
        snapshot::assert_snapshot("snapshots/endpoint_keys_ble_split.snap", actual);
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
                    "Endpoint '{}' missing from ENDPOINT_LIST — update both cfg blocks in endpoints.rs",
                    path
                );
            }
        }
    }

    /// Verify that every bulk endpoint is present in the combined ENDPOINT_LIST.
    #[cfg(feature = "bulk")]
    #[test]
    fn endpoint_list_contains_all_bulk_endpoints() {
        let endpoint_paths: alloc::collections::BTreeSet<&str> =
            ENDPOINT_LIST.endpoints.iter().map(|(path, _, _)| *path).collect();

        let bulk_groups: &[&[(&str, Key, Key)]] = &[
            KEYMAP_BULK_ENDPOINT_LIST.endpoints,
            COMBO_BULK_ENDPOINT_LIST.endpoints,
            MORSE_BULK_ENDPOINT_LIST.endpoints,
        ];

        for group in bulk_groups {
            for (path, _, _) in *group {
                assert!(
                    endpoint_paths.contains(path),
                    "Bulk endpoint '{}' missing from ENDPOINT_LIST",
                    path
                );
            }
        }
    }
}
