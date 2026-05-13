//! Rynk protocol ICD (Interface Control Document).
//!
//! Rynk is RMK's native host-communication protocol. It carries RMK's
//! canonical types (`KeyAction`, `Combo`, `Morse`, `Fork`, `EncoderAction`,
//! `BatteryStatus`, `BleStatus`) on the wire using a 5-byte fixed header +
//! postcard-encoded payload.
//!
//! ## Wire format
//!
//! ```text
//! ┌──────────────┬───────────┬────────────────────┐
//! │ CMD u16 LE   │ SEQ u8    │ LEN u16 LE         │  ← 5-byte header
//! ├──────────────┴───────────┴────────────────────┤
//! │              postcard-encoded payload          │  ← LEN bytes
//! └────────────────────────────────────────────────┘
//! ```
//!
//! - **CMD** — `0x0000..=0x7FFF` request/response, `0x8000..=0xFFFF` topic.
//!   Top bit splits dispatch with one mask. See [`Cmd`].
//! - **SEQ** — opaque echo. Firmware copies request's SEQ into response;
//!   topics always send SEQ = 0.
//! - **LEN** — payload byte count. Authoritative end-of-message indicator.
//!
//! ## Response envelope
//!
//! Every response payload is postcard-encoded `Result<T, RynkError>`.
//! `Set*` calls use `T = ()` and `Get*` calls use `T` = the requested
//! type. This costs 1 byte on success and lets reads signal
//! `InvalidParameter` / `BadState` / `InternalError` without a per-cmd
//! sentinel. Requests are *not* wrapped — they're the bare postcard
//! encoding of the request struct.
//!
//! ## Module layout
//!
//! - [`cmd`] — `Cmd` enum (request, response, topic tags)
//! - [`frame`] — `Frame` + `FrameOps` (in-place wire-header accessors)
//! - [`buffer`] — `RYNK_MIN_BUFFER_SIZE` const computed from `MaxSize` of
//!   every wire type
//! - [`system`] — handshake (`ProtocolVersion`, `DeviceCapabilities`,
//!   `StorageResetMode`, `BehaviorConfig`)
//! - [`keymap`], [`encoder`], [`macro_data`], [`combo`], [`morse`],
//!   [`fork`] — per-domain request/response types
//! - [`status`] — runtime status types (`MatrixState`, `PeripheralStatus`)
//!
//! ## Protocol handshake
//!
//! 1. Host connects over USB bulk or BLE GATT (length-prefixed frames).
//! 2. Host sends `Cmd::GetVersion`. If `major` differs from the host's
//!    supported major, or `minor` exceeds the host's known max, the host
//!    aborts with an "update host" diagnostic. `GetVersion`'s shape is
//!    permanent — never modified, even across major bumps.
//! 3. Host sends `Cmd::GetCapabilities` to learn layout, feature flags,
//!    and limits.
//! 4. Host gates every subsequent call on the capability flags.
//!
//! ### Version bump policy
//!
//! - `minor` bump: new `Cmd` variant appended; new field appended to a
//!   wire struct (with explicit version handling); new variant in a wire
//!   enum (including `RynkError`).
//! - `major` bump: `Cmd` variant retyped; struct field reshaped; enum
//!   variant renamed/renumbered. `Cmd::GetVersion`'s shape is exempt —
//!   changing it is forbidden even across major bumps.
//! - Neither: no wire change.

pub mod buffer;
pub mod cmd;
pub mod frame;

mod combo;
mod encoder;
mod fork;
mod keymap;
mod macro_data;
mod morse;
mod status;
mod system;

use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

// Re-export every submodule's public items into `protocol::rynk::*` for
// convenient downstream import.
pub use self::buffer::{RYNK_MAX_PAYLOAD, RYNK_MIN_BUFFER_SIZE};
pub use self::cmd::Cmd;
pub use self::combo::*;
pub use self::encoder::*;
pub use self::fork::*;
pub use self::frame::{Frame, FrameOps, RYNK_HEADER_SIZE};
pub use self::keymap::*;
pub use self::macro_data::*;
pub use self::morse::*;
pub use self::status::*;
pub use self::system::*;

// ---------------------------------------------------------------------------
// Protocol-wide error primitives
// ---------------------------------------------------------------------------

/// Protocol-level error returned in every response payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[non_exhaustive]
pub enum RynkError {
    /// The request parameters are invalid or out of range.
    InvalidParameter,
    /// Operation not valid in the current device state.
    BadState,
    /// An internal firmware error occurred (storage, contention, etc).
    InternalError,
}

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
    /// - under feature configurations with a large `BULK_SIZE`, max-capacity
    ///   bulk payloads still fit comfortably;
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
    /// Use alongside `round_trip` in max-capacity tests to catch
    /// under-counted manual `MaxSize` impls.
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
// Wire-format value snapshot harness (golden-file test)
// ---------------------------------------------------------------------------
//
// Schema drift detection: a single `wire_values.snap` file holds one
// exemplar per wire type, postcard-encoded. Any field reorder / type change
// / variant renumber flips the bytes for the affected type and fails CI.
// If the change is intentional, bump `ProtocolVersion::CURRENT` and
// regenerate the snapshot.

#[cfg(test)]
pub(crate) mod snapshot {
    extern crate alloc;
    extern crate std;

    use alloc::format;
    use alloc::string::String;
    use alloc::vec::Vec;
    use std::path::PathBuf;
    use std::{env, fs};

    /// Format a byte slice as lowercase, space-separated hex.
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
             # the wire format changed (either intentionally or by accident). If intentional,\n\
             # bump ProtocolVersion::CURRENT and regenerate:\n\
             #   UPDATE_SNAPSHOTS=1 cargo test -p rmk-types --features rynk wire_values\n\
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
    /// When `UPDATE_SNAPSHOTS` is set, write the file instead.
    pub fn assert_snapshot(rel_path: &str, actual: String) {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/protocol/rynk")
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
                e,
            )
        });

        if expected != actual {
            panic!(
                "snapshot mismatch: {}\n\
                 --- expected ---\n{}\
                 --- actual ---\n{}\
                 If intentional, regenerate with UPDATE_SNAPSHOTS=1 and bump ProtocolVersion::CURRENT.",
                path.display(),
                expected,
                actual,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Tests: top-level type round-trips + wire-format snapshot
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    extern crate alloc;

    use super::*;
    use crate::connection::ConnectionType;

    #[test]
    fn round_trip_rynk_error_and_result() {
        use super::test_utils::round_trip;
        round_trip(&RynkError::InvalidParameter);
        round_trip(&RynkError::BadState);
        round_trip(&RynkError::InternalError);
        let ok: Result<(), RynkError> = Ok(());
        let err: Result<(), RynkError> = Err(RynkError::BadState);
        let _ = round_trip(&ok);
        let _ = round_trip(&err);
    }

    fn encode<T: serde::Serialize>(val: &T) -> alloc::vec::Vec<u8> {
        let mut buf = [0u8; 256];
        let bytes = postcard::to_slice(val, &mut buf).expect("encode");
        bytes.to_vec()
    }

    /// Lock down postcard's actual byte encoding for stability-critical
    /// values. A diff in this snapshot indicates wire-format drift; if
    /// intentional, regenerate the snapshot and bump `ProtocolVersion::CURRENT`.
    #[test]
    fn wire_values_locked() {
        let mut bitmap: heapless::Vec<u8, MATRIX_BITMAP_SIZE> = heapless::Vec::new();
        bitmap.extend_from_slice(&[0x05, 0x00, 0x20]).unwrap();
        let matrix = MatrixState { pressed_bitmap: bitmap };

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
            ("MatrixState{[0x05,0x00,0x20]}", encode(&matrix)),
            ("ProtocolVersion{1,0}", encode(&ProtocolVersion { major: 1, minor: 0 }),),
            ("RynkError::BadState", encode(&RynkError::BadState)),
            ("RynkError::InternalError", encode(&RynkError::InternalError)),
            ("RynkError::InvalidParameter", encode(&RynkError::InvalidParameter)),
            (
                "Result<(),RynkError>::Err(BadState)",
                encode::<Result<(), RynkError>>(&Err(RynkError::BadState)),
            ),
            ("Result<(),RynkError>::Ok", encode::<Result<(), RynkError>>(&Ok(()))),
            ("StorageResetMode::Full", encode(&StorageResetMode::Full)),
            ("StorageResetMode::LayoutOnly", encode(&StorageResetMode::LayoutOnly)),
        ];
        let view: alloc::vec::Vec<(&str, &[u8])> = entries.iter().map(|(l, b)| (*l, b.as_slice())).collect();

        let actual = snapshot::format_value_snapshot("snapshots/wire_values.snap", &view);
        snapshot::assert_snapshot("snapshots/wire_values.snap", actual);
    }
}
