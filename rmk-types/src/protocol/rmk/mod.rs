//! RMK protocol ICD (Interface Control Document).
//!
//! This module defines the shared type contract between firmware and host for the
//! RMK communication protocol. It contains all endpoint and topic declarations,
//! request/response types, and protocol constants.
//!
//! The protocol uses postcard-rpc's type-level endpoint definitions over COBS-framed
//! byte streams (USB bulk transfer and BLE serial).
//!
//! ## Module layout
//!
//! - [`endpoints`] — `endpoints!` macro invocations + assembled `ENDPOINT_LIST`
//! - [`topics`] — `topics!` macro invocations
//! - [`system`] — handshake, lock/unlock, storage reset, behavior config
//! - [`keymap`], [`encoder`], [`macro_data`], [`combo`], [`morse`], [`fork`] — per-domain request/response types
//! - [`status`] — runtime status types (matrix state, peripheral status)
//!
//! ## Protocol Handshake
//!
//! The expected connection flow is:
//! 1. Host connects over USB bulk or BLE serial (COBS-framed).
//! 2. Host sends `GetVersion` — verifies protocol compatibility.
//! 3. Host sends `GetCapabilities` — learns layout, feature set, and limits.
//! 4. Host checks capability flags (e.g., `bulk_transfer_supported`,
//!    `ble_enabled`) before using conditional endpoint groups.
//! 5. If the device is locked, host sends `UnlockRequest` and completes
//!    the physical key challenge before issuing write operations.

mod combo;
mod encoder;
mod endpoints;
mod fork;
mod keymap;
mod macro_data;
mod morse;
mod status;
mod system;
mod topics;

use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

// Re-export every submodule's public items into `protocol::rmk::*` for
// convenient endpoint registration. Domain types (Combo, Morse, Fork, etc.)
// are NOT re-exported here — import them from their canonical crate-root
// modules instead.
pub use self::combo::*;
pub use self::encoder::*;
pub use self::endpoints::*;
pub use self::fork::*;
pub use self::keymap::*;
pub use self::macro_data::*;
pub use self::morse::*;
pub use self::status::*;
pub use self::system::*;
pub use self::topics::*;

// ---------------------------------------------------------------------------
// Protocol-wide error primitives
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Test utilities (shared across submodule test mods)
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod test_utils {
    extern crate alloc;

    use alloc::vec;

    use postcard::experimental::max_size::MaxSize;
    use serde::{Deserialize, Serialize};

    /// Buffer size used by round-trip / max-size helpers.
    ///
    /// Sized at twice the type's declared `POSTCARD_MAX_SIZE` plus a small
    /// fixed slack so that:
    /// - under feature configurations with a large `BULK_SIZE` (notably
    ///   `host`, where `BULK_SIZE = MAX_BULK_SIZE = 16` and
    ///   `MORSE_SIZE = MAX_MORSE_SIZE = 32`), max-capacity bulk payloads
    ///   still fit comfortably;
    /// - an under-counted manual `MaxSize` impl produces a clear assertion
    ///   failure in `assert_max_size_bound` instead of a `SerializeBufferFull`
    ///   panic.
    fn buffer_capacity<T: MaxSize>() -> usize {
        T::POSTCARD_MAX_SIZE.saturating_mul(2).saturating_add(64)
    }

    /// Postcard round-trip helper used by every submodule's tests.
    pub fn round_trip<T>(val: &T) -> T
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + core::fmt::Debug + MaxSize,
    {
        let mut buf = vec![0u8; buffer_capacity::<T>()];
        let bytes = postcard::to_slice(val, &mut buf).expect("serialize");
        let decoded: T = postcard::from_bytes(bytes).expect("deserialize");
        assert_eq!(&decoded, val);
        decoded
    }

    /// Assert that `val` serializes within its declared `POSTCARD_MAX_SIZE`.
    /// Use alongside `round_trip` in max-capacity tests to catch under-counted
    /// manual `MaxSize` impls (the dangerous bug — buffer overflows downstream).
    pub fn assert_max_size_bound<T>(val: &T)
    where
        T: Serialize + MaxSize,
    {
        let mut buf = vec![0u8; buffer_capacity::<T>()];
        let bytes = postcard::to_slice(val, &mut buf).expect("serialize");
        assert!(
            bytes.len() <= T::POSTCARD_MAX_SIZE,
            "{} encoded to {} bytes but POSTCARD_MAX_SIZE = {}",
            core::any::type_name::<T>(),
            bytes.len(),
            T::POSTCARD_MAX_SIZE,
        );
    }
}

// ---------------------------------------------------------------------------
// Wire-format snapshot harness (golden-file tests)
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod snapshot {
    extern crate alloc;
    extern crate std;

    use alloc::format;
    use alloc::string::String;
    use alloc::vec::Vec;
    use std::path::PathBuf;
    use std::{env, fs};

    /// Format a byte slice as lowercase, space-separated hex (e.g. `01 0a ff`).
    pub fn hex(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(bytes.len() * 3);
        for (i, b) in bytes.iter().enumerate() {
            if i > 0 {
                s.push(' ');
            }
            s.push_str(&format!("{:02x}", b));
        }
        s
    }

    /// Build the snapshot text for an endpoint key list.
    /// Output format: `<path>  REQ <hex>  RESP <hex>`, sorted by path.
    pub fn format_endpoint_keys(rel_path: &str, entries: &[(&str, [u8; 8], [u8; 8])]) -> String {
        let mut sorted: Vec<&(&str, [u8; 8], [u8; 8])> = entries.iter().collect();
        sorted.sort_by_key(|(path, _, _)| *path);

        // Pad the path column to the longest entry for readable diffs.
        let path_width = sorted.iter().map(|(p, _, _)| p.len()).max().unwrap_or(0);

        let mut out = String::new();
        out.push_str(&format!(
            "# Endpoint Key snapshot — DO NOT edit by hand.\n\
             # File: {}\n\
             # Each Key is an 8-byte hash of (path, postcard schema of req/resp).\n\
             # Any change to a request/response type — including transitively-referenced\n\
             # types — flips the corresponding Key. If the change is intentional, regenerate:\n\
             #   UPDATE_SNAPSHOTS=1 cargo test -p rmk-types --features rmk_protocol\n\
             # Format: <path>  REQ <8-byte hex>  RESP <8-byte hex>\n\
             \n",
            rel_path,
        ));
        for (path, req, resp) in sorted {
            out.push_str(&format!(
                "{:width$}  REQ {}  RESP {}\n",
                path,
                hex(req),
                hex(resp),
                width = path_width,
            ));
        }
        out
    }

    /// Build the snapshot text for a topic key list.
    /// Output format: `<path>  KEY <hex>`, sorted by path.
    pub fn format_topic_keys(rel_path: &str, entries: &[(&str, [u8; 8])]) -> String {
        let mut sorted: Vec<&(&str, [u8; 8])> = entries.iter().collect();
        sorted.sort_by_key(|(path, _)| *path);

        let path_width = sorted.iter().map(|(p, _)| p.len()).max().unwrap_or(0);

        let mut out = String::new();
        out.push_str(&format!(
            "# Topic Key snapshot — DO NOT edit by hand.\n\
             # File: {}\n\
             # Regenerate intentionally with:\n\
             #   UPDATE_SNAPSHOTS=1 cargo test -p rmk-types --features rmk_protocol\n\
             # Format: <path>  KEY <8-byte hex>\n\
             \n",
            rel_path,
        ));
        for (path, key) in sorted {
            out.push_str(&format!("{:width$}  KEY {}\n", path, hex(key), width = path_width,));
        }
        out
    }

    /// Build the snapshot text for a list of (label, encoded bytes) pairs.
    pub fn format_value_snapshot(rel_path: &str, entries: &[(&str, &[u8])]) -> String {
        let mut sorted: Vec<&(&str, &[u8])> = entries.iter().collect();
        sorted.sort_by_key(|(label, _)| *label);

        let label_width = sorted.iter().map(|(l, _)| l.len()).max().unwrap_or(0);

        let mut out = String::new();
        out.push_str(&format!(
            "# Wire-format value snapshot — DO NOT edit by hand.\n\
             # File: {}\n\
             # Each entry is the postcard byte encoding of a fixed value. A diff here means\n\
             # the wire format changed (either intentionally or by accident). Regenerate with:\n\
             #   UPDATE_SNAPSHOTS=1 cargo test -p rmk-types --features rmk_protocol wire_values\n\
             # Format: <label>  <hex bytes>\n\
             \n",
            rel_path,
        ));
        for (label, bytes) in sorted {
            out.push_str(&format!("{:width$}  {}\n", label, hex(bytes), width = label_width));
        }
        out
    }

    /// Compare actual snapshot text against the on-disk file.
    /// When `UPDATE_SNAPSHOTS` is set in the environment, write the file instead.
    pub fn assert_snapshot(rel_path: &str, actual: String) {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/protocol/rmk")
            .join(rel_path);

        if env::var_os("UPDATE_SNAPSHOTS").is_some() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .unwrap_or_else(|e| panic!("create snapshot dir {}: {}", parent.display(), e));
            }
            fs::write(&path, &actual).unwrap_or_else(|e| panic!("write snapshot {}: {}", path.display(), e));
            return;
        }

        let expected = fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "missing snapshot {} ({}). Run with UPDATE_SNAPSHOTS=1 to create.",
                path.display(),
                e
            )
        });

        if expected != actual {
            panic!(
                "snapshot mismatch: {}\n\
                 --- expected ---\n{}\
                 --- actual ---\n{}\
                 If this change is intentional, regenerate with UPDATE_SNAPSHOTS=1.",
                path.display(),
                expected,
                actual,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Tests: cross-cutting collision checks + value-level wire format snapshot
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    extern crate alloc;

    use heapless::Vec;
    use postcard_rpc::{Endpoint, Key, Topic};

    use super::*;
    use crate::connection::ConnectionType;

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
        #[cfg(feature = "_ble")]
        {
            keys.extend_from_slice(&[
                GetBleStatus::REQ_KEY,
                SwitchBleProfile::REQ_KEY,
                ClearBleProfile::REQ_KEY,
                GetBatteryStatus::REQ_KEY,
            ]);
        }
        #[cfg(all(feature = "_ble", feature = "split"))]
        {
            keys.extend_from_slice(&[GetPeripheralStatus::REQ_KEY]);
        }
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
            keys.extend_from_slice(&[BatteryStatusTopic::TOPIC_KEY, BleStatusChangeTopic::TOPIC_KEY]);
        }
        keys
    }

    // -- Cross-group key collision (the main thing tested at module scope) --

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
        #[allow(unused_mut)]
        let mut total_topics = TOPICS_OUT_LIST.topics.len();
        #[cfg(feature = "_ble")]
        {
            total_topics += BLE_TOPICS_OUT_LIST.topics.len();
        }
        assert!(total_topics >= all_topic_keys().len());
    }

    // -- Top-level type round-trips that don't fit any single submodule --

    #[test]
    fn round_trip_rmk_error_and_result() {
        use super::test_utils::round_trip;
        round_trip(&RmkError::InvalidParameter);
        round_trip(&RmkError::BadState);
        round_trip(&RmkError::InternalError);
        let ok: RmkResult = Ok(());
        let err: RmkResult = Err(RmkError::BadState);
        let _ = round_trip(&ok);
        let _ = round_trip(&err);
    }

    // -- Wire-format value snapshot --

    /// Encode a value to a freshly-allocated `Vec<u8>` for the snapshot table.
    fn encode<T: serde::Serialize>(val: &T) -> alloc::vec::Vec<u8> {
        let mut buf = [0u8; 256];
        let bytes = postcard::to_slice(val, &mut buf).expect("encode");
        bytes.to_vec()
    }

    /// Lock down postcard's actual byte encoding for stability-critical values.
    /// This catches regressions in postcard itself or in serde attributes that
    /// wouldn't change a schema hash (and thus wouldn't be caught by the
    /// endpoint-key snapshot tests).
    #[test]
    fn wire_values_locked() {
        // -- Build the bitmap up front so the borrow lives long enough.
        let mut bitmap: heapless::Vec<u8, MATRIX_BITMAP_SIZE> = heapless::Vec::new();
        bitmap.extend_from_slice(&[0x05, 0x00, 0x20]).unwrap();
        let matrix = MatrixState { pressed_bitmap: bitmap };
        let unlock_empty = UnlockChallenge {
            key_positions: Vec::new(),
        };

        let entries: alloc::vec::Vec<(&str, alloc::vec::Vec<u8>)> = alloc::vec![
            ("ConnectionType::Ble", encode(&ConnectionType::Ble)),
            ("ConnectionType::Usb", encode(&ConnectionType::Usb)),
            (
                "KeyPosition{layer:0,row:5,col:13}",
                encode(&KeyPosition {
                    layer: 0,
                    row: 5,
                    col: 13,
                }),
            ),
            (
                "LockStatus{locked:true,await:false,rem:0}",
                encode(&LockStatus {
                    locked: true,
                    awaiting_keys: false,
                    remaining_keys: 0,
                }),
            ),
            ("MatrixState{[0x05,0x00,0x20]}", encode(&matrix)),
            ("ProtocolVersion{1,0}", encode(&ProtocolVersion { major: 1, minor: 0 }),),
            ("RmkError::BadState", encode(&RmkError::BadState)),
            ("RmkError::InternalError", encode(&RmkError::InternalError)),
            ("RmkError::InvalidParameter", encode(&RmkError::InvalidParameter)),
            (
                "RmkResult::Err(BadState)",
                encode::<RmkResult>(&Err(RmkError::BadState))
            ),
            ("RmkResult::Ok", encode::<RmkResult>(&Ok(()))),
            ("StorageResetMode::Full", encode(&StorageResetMode::Full)),
            ("StorageResetMode::LayoutOnly", encode(&StorageResetMode::LayoutOnly)),
            ("UnlockChallenge{[]}", encode(&unlock_empty)),
        ];
        let view: alloc::vec::Vec<(&str, &[u8])> = entries.iter().map(|(l, b)| (*l, b.as_slice())).collect();

        let actual = snapshot::format_value_snapshot("snapshots/wire_values.snap", &view);
        snapshot::assert_snapshot("snapshots/wire_values.snap", actual);
    }
}
