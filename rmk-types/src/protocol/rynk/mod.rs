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
//! │              postcard-encoded payload         │  ← LEN bytes
//! └───────────────────────────────────────────────┘
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
//! The per-domain modules (`keymap`, `encoder`, `combo`, …) are private;
//! their types are re-exported flat at `protocol::rynk::*`. Only `cmd`,
//! `message`, and `buffer` are public.
//!
//! ## Protocol handshake
//!
//! 1. Host connects over USB bulk or BLE GATT (length-prefixed messages).
//! 2. Host sends `Cmd::GetVersion`. If `major` differs from the host's
//!    supported major, or `minor` exceeds the host's known max, the host
//!    aborts with an "update host" diagnostic.
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

pub mod buffer;
pub mod cmd;
pub mod message;

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
pub use self::keymap::*;
pub use self::macro_data::*;
pub use self::message::{RYNK_HEADER_SIZE, RynkMessage};
pub use self::morse::*;
pub use self::status::*;
pub use self::system::*;

/// Largest single GATT write/notification on the Rynk BLE characteristics.
pub const RYNK_BLE_CHUNK_SIZE: usize = 244;

/// Rynk GATT service UUID
pub const RYNK_SERVICE_UUID: u128 = 0x10900067_537f_4f0a_9b55_929e271f61ab;
/// Rynk `input_data` characteristic UUID.
pub const RYNK_INPUT_CHAR_UUID: u128 = 0x80f9319b_0c74_43a5_9738_c59d6dda3db9;
/// Rynk `output_data` characteristic UUID.
pub const RYNK_OUTPUT_CHAR_UUID: u128 = 0x19802524_6f90_4346_93c2_63dbc509ab55;

/// Protocol-level error returned in every response payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[non_exhaustive]
pub enum RynkError {
    /// The request could not be decoded
    Malformed,
    /// Device is not currently in a state to satisfy the request
    NotReady,
    /// Persistent storage failed on a write path (flash erase/write error)
    StorageFault,
    /// Internal firmware fault.
    Internal,
    /// Command is recognized but the handler is not implemented yet.
    Unimplemented,
    /// The request decoded cleanly but is semantically invalid.
    Invalid,
    /// The frame is well-formed but its CMD is unknown.
    UnknownCmd,
}

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
        round_trip(&RynkError::Malformed);
        round_trip(&RynkError::NotReady);
        round_trip(&RynkError::StorageFault);
        round_trip(&RynkError::Internal);
        round_trip(&RynkError::Unimplemented);
        round_trip(&RynkError::Invalid);
        round_trip(&RynkError::UnknownCmd);
        let ok: Result<(), RynkError> = Ok(());
        let err: Result<(), RynkError> = Err(RynkError::StorageFault);
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
    ///
    /// One exemplar per Rynk wire type, plus every variant of the positional
    /// enums (`KeyAction`, `Action`, and the status enums) so a reordered or
    /// inserted variant flips the bytes. Postcard tags enums by declaration
    /// order, *not* the `#[repr]` discriminant, so the keycode exemplars also
    /// pin variant ordinals. Structs use distinct per-field values so a field
    /// swap is caught too. Only feature-independent values belong here: the
    /// gated `Action::Steno`, the `bulk` request/response payloads, and
    /// `PeripheralStatus` are excluded so every `rynk` feature set yields the
    /// same snapshot.
    #[test]
    fn wire_values_locked() {
        use crate::action::{Action, EncoderAction, KeyAction, KeyboardAction, LightAction};
        use crate::battery::{BatteryStatus, ChargeState};
        use crate::ble::{BleState, BleStatus};
        use crate::combo::Combo;
        use crate::connection::{ConnectionStatus, UsbState};
        use crate::fork::{Fork, StateBits};
        use crate::keycode::{ConsumerKey, HidKeyCode, KeyCode, SpecialKey, SystemControlKey};
        use crate::led_indicator::LedIndicator;
        use crate::modifier::ModifierCombination;
        use crate::morse::{Morse, MorseMode, MorseProfile, TAP};
        use crate::mouse_button::MouseButtons;

        let mut bitmap: heapless::Vec<u8, MATRIX_BITMAP_SIZE> = heapless::Vec::new();
        bitmap.extend_from_slice(&[0x05, 0x00, 0x20]).unwrap();
        let matrix = MatrixState { pressed_bitmap: bitmap };

        // Distinct ascending per-field values so a field reorder flips bytes.
        let capabilities = DeviceCapabilities {
            num_layers: 1,
            num_rows: 2,
            num_cols: 3,
            num_encoders: 4,
            max_combos: 5,
            max_combo_keys: 6,
            max_macros: 7,
            macro_space_size: 8,
            max_morse: 9,
            max_patterns_per_key: 10,
            max_forks: 11,
            storage_enabled: true,
            lighting_enabled: false,
            is_split: true,
            num_split_peripherals: 12,
            ble_enabled: false,
            num_ble_profiles: 13,
            max_payload_size: 14,
            max_bulk_keys: 15,
            macro_chunk_size: 16,
            bulk_transfer_supported: true,
        };
        let behavior = BehaviorConfig {
            combo_timeout_ms: 50,
            oneshot_timeout_ms: 500,
            tap_interval_ms: 200,
            tap_capslock_interval_ms: 20,
        };
        let connection = ConnectionStatus {
            usb: UsbState::Configured,
            ble: BleStatus {
                profile: 1,
                state: BleState::Advertising,
            },
            preferred: ConnectionType::Ble,
        };
        // All three sub-bitfields distinct so a StateBits field swap shows.
        let state_bits = StateBits::new_from(
            ModifierCombination::LCTRL,
            LedIndicator::CAPS_LOCK,
            MouseButtons::BUTTON1,
        );
        let combo = Combo::new(
            [KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A)))],
            KeyAction::Morse(1),
            Some(2),
        );
        let fork = Fork::new(
            KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A))),
            KeyAction::No,
            KeyAction::Morse(2),
            state_bits,
            StateBits::default(),
            ModifierCombination::LSHIFT,
            true,
        );
        // Pins Morse's custom serde shape: (MorseProfile, Vec<(u16, Action)>).
        let mut morse_actions = heapless::LinearMap::new();
        morse_actions
            .insert(TAP, Action::Key(KeyCode::Hid(HidKeyCode::A)))
            .unwrap();
        let morse = Morse {
            profile: MorseProfile::const_default(),
            actions: morse_actions,
        };
        let mut macro_bytes = heapless::Vec::new();
        macro_bytes.extend_from_slice(&[0x01, 0x02, 0x03]).unwrap();
        let macro_data = MacroData { data: macro_bytes };
        let mut unlock_keys: heapless::Vec<(u8, u8), UNLOCK_KEYS_SIZE> = heapless::Vec::new();
        unlock_keys.push((1, 2)).unwrap();
        unlock_keys.push((3, 4)).unwrap();
        let unlock = UnlockChallenge {
            key_positions: unlock_keys,
        };
        let encoder = EncoderAction::new(KeyAction::Morse(3), KeyAction::No);
        let profile = MorseProfile::new(None, Some(MorseMode::Normal), Some(200), Some(150));

        let entries: alloc::vec::Vec<(&str, alloc::vec::Vec<u8>)> = alloc::vec![
            // --- Response envelope + connection ---
            ("ConnectionType::Ble", encode(&ConnectionType::Ble)),
            ("ConnectionType::Usb", encode(&ConnectionType::Usb)),
            (
                "Result<(),RynkError>::Err(StorageFault)",
                encode::<Result<(), RynkError>>(&Err(RynkError::StorageFault)),
            ),
            ("Result<(),RynkError>::Ok", encode::<Result<(), RynkError>>(&Ok(()))),
            ("RynkError::Internal", encode(&RynkError::Internal)),
            ("RynkError::Invalid", encode(&RynkError::Invalid)),
            ("RynkError::Malformed", encode(&RynkError::Malformed)),
            ("RynkError::NotReady", encode(&RynkError::NotReady)),
            ("RynkError::StorageFault", encode(&RynkError::StorageFault)),
            ("RynkError::Unimplemented", encode(&RynkError::Unimplemented)),
            ("RynkError::UnknownCmd", encode(&RynkError::UnknownCmd)),
            // --- KeyAction: every variant tag (positional) ---
            ("KeyAction::No", encode(&KeyAction::No)),
            ("KeyAction::Transparent", encode(&KeyAction::Transparent)),
            (
                "KeyAction::Single(Action::Key(Hid(A)))",
                encode(&KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A)))),
            ),
            ("KeyAction::Tap(Action::No)", encode(&KeyAction::Tap(Action::No))),
            (
                "KeyAction::TapHold(Key(A),LayerOn(3))",
                encode(&KeyAction::TapHold(
                    Action::Key(KeyCode::Hid(HidKeyCode::A)),
                    Action::LayerOn(3),
                    MorseProfile::const_default(),
                )),
            ),
            ("KeyAction::Morse(3)", encode(&KeyAction::Morse(3))),
            // --- Action: every feature-independent variant tag (positional) ---
            ("Action::No", encode(&Action::No)),
            ("Action::Key(Hid(A))", encode(&Action::Key(KeyCode::Hid(HidKeyCode::A)))),
            (
                "Action::Modifier(LCtrl)",
                encode(&Action::Modifier(ModifierCombination::LCTRL))
            ),
            (
                "Action::KeyWithModifier(A,LShift)",
                encode(&Action::KeyWithModifier(
                    KeyCode::Hid(HidKeyCode::A),
                    ModifierCombination::LSHIFT
                )),
            ),
            ("Action::LayerOn(1)", encode(&Action::LayerOn(1))),
            (
                "Action::LayerOnWithModifier(2,LCtrl)",
                encode(&Action::LayerOnWithModifier(2, ModifierCombination::LCTRL)),
            ),
            ("Action::LayerOff(3)", encode(&Action::LayerOff(3))),
            ("Action::LayerToggle(4)", encode(&Action::LayerToggle(4))),
            ("Action::DefaultLayer(5)", encode(&Action::DefaultLayer(5))),
            ("Action::LayerToggleOnly(6)", encode(&Action::LayerToggleOnly(6))),
            ("Action::TriLayerLower", encode(&Action::TriLayerLower)),
            ("Action::TriLayerUpper", encode(&Action::TriLayerUpper)),
            ("Action::TriggerMacro(7)", encode(&Action::TriggerMacro(7))),
            ("Action::OneShotLayer(8)", encode(&Action::OneShotLayer(8))),
            (
                "Action::OneShotModifier(LAlt)",
                encode(&Action::OneShotModifier(ModifierCombination::LALT))
            ),
            (
                "Action::OneShotKey(Hid(B))",
                encode(&Action::OneShotKey(KeyCode::Hid(HidKeyCode::B)))
            ),
            ("Action::Light(RgbTog)", encode(&Action::Light(LightAction::RgbTog))),
            (
                "Action::KeyboardControl(Bootloader)",
                encode(&Action::KeyboardControl(KeyboardAction::Bootloader)),
            ),
            (
                "Action::Special(GraveEscape)",
                encode(&Action::Special(SpecialKey::GraveEscape))
            ),
            ("Action::User(9)", encode(&Action::User(9))),
            // --- KeyCode discriminants (postcard tags by ordinal, not repr) ---
            ("KeyCode::Hid(A)", encode(&KeyCode::Hid(HidKeyCode::A))),
            (
                "KeyCode::Consumer(VolumeIncrement)",
                encode(&KeyCode::Consumer(ConsumerKey::VolumeIncrement)),
            ),
            (
                "KeyCode::SystemControl(Sleep)",
                encode(&KeyCode::SystemControl(SystemControlKey::Sleep))
            ),
            // --- Bitfields: pin LSB bit order ---
            (
                "ModifierCombination(LCtrl|RGui)",
                encode(&(ModifierCombination::LCTRL | ModifierCombination::RGUI)),
            ),
            (
                "LedIndicator(Num|Scroll)",
                encode(&(LedIndicator::NUM_LOCK | LedIndicator::SCROLL_LOCK))
            ),
            (
                "MouseButtons(B1|B8)",
                encode(&(MouseButtons::BUTTON1 | MouseButtons::BUTTON8))
            ),
            ("MorseProfile(Normal,200,150)", encode(&profile)),
            // --- Keymap / encoder / behavior config payloads ---
            (
                "KeyPosition{layer:0,row:5,col:13}",
                encode(&KeyPosition {
                    layer: 0,
                    row: 5,
                    col: 13
                })
            ),
            ("EncoderAction{Morse(3),No}", encode(&encoder)),
            ("Combo{[Single(A)],Morse(1),L2}", encode(&combo)),
            ("Fork{Single(A),No,Morse(2)}", encode(&fork)),
            ("StateBits{LCtrl,Caps,B1}", encode(&state_bits)),
            ("Morse{TAP->Key(A)}", encode(&morse)),
            ("MacroData{[0x01,0x02,0x03]}", encode(&macro_data)),
            // --- Status / system responses ---
            ("MatrixState{[0x05,0x00,0x20]}", encode(&matrix)),
            ("DeviceCapabilities{1..16}", encode(&capabilities)),
            ("BehaviorConfig{50,500,200,20}", encode(&behavior)),
            ("ConnectionStatus{Configured,{1,Adv},Ble}", encode(&connection)),
            ("ProtocolVersion{1,0}", encode(&ProtocolVersion { major: 1, minor: 0 })),
            (
                "LockStatus{true,false,3}",
                encode(&LockStatus {
                    locked: true,
                    awaiting_keys: false,
                    remaining_keys: 3
                }),
            ),
            ("UnlockChallenge{[(1,2),(3,4)]}", encode(&unlock)),
            ("BatteryStatus::Unavailable", encode(&BatteryStatus::Unavailable)),
            (
                "BatteryStatus::Available{Discharging,85}",
                encode(&BatteryStatus::Available {
                    charge_state: ChargeState::Discharging,
                    level: Some(85)
                }),
            ),
            ("ChargeState::Charging", encode(&ChargeState::Charging)),
            ("ChargeState::Discharging", encode(&ChargeState::Discharging)),
            ("ChargeState::Unknown", encode(&ChargeState::Unknown)),
            ("BleState::Advertising", encode(&BleState::Advertising)),
            ("BleState::Connected", encode(&BleState::Connected)),
            ("BleState::Inactive", encode(&BleState::Inactive)),
            (
                "BleStatus{2,Connected}",
                encode(&BleStatus {
                    profile: 2,
                    state: BleState::Connected
                })
            ),
            ("UsbState::Disabled", encode(&UsbState::Disabled)),
            ("UsbState::Enabled", encode(&UsbState::Enabled)),
            ("UsbState::Configured", encode(&UsbState::Configured)),
            ("UsbState::Suspended", encode(&UsbState::Suspended)),
            ("StorageResetMode::Full", encode(&StorageResetMode::Full)),
            ("StorageResetMode::LayoutOnly", encode(&StorageResetMode::LayoutOnly)),
            // --- Request payloads: pin field order of the Get/Set structs ---
            (
                "SetKeyRequest{{0,5,13},Morse(7)}",
                encode(&SetKeyRequest {
                    position: KeyPosition {
                        layer: 0,
                        row: 5,
                        col: 13
                    },
                    action: KeyAction::Morse(7),
                }),
            ),
            (
                "GetEncoderRequest{1,2}",
                encode(&GetEncoderRequest {
                    encoder_id: 1,
                    layer: 2
                })
            ),
            (
                "SetEncoderRequest{1,2,{Morse(3),No}}",
                encode(&SetEncoderRequest {
                    encoder_id: 1,
                    layer: 2,
                    action: encoder
                }),
            ),
            (
                "GetMacroRequest{1,256}",
                encode(&GetMacroRequest { index: 1, offset: 256 })
            ),
            (
                "SetMacroRequest{1,2,[0x01,0x02,0x03]}",
                encode(&SetMacroRequest {
                    index: 1,
                    offset: 2,
                    data: macro_data
                }),
            ),
            (
                "SetComboRequest{3,combo}",
                encode(&SetComboRequest {
                    index: 3,
                    config: combo
                })
            ),
            (
                "SetMorseRequest{0,morse}",
                encode(&SetMorseRequest {
                    index: 0,
                    config: morse
                })
            ),
            (
                "SetForkRequest{2,fork}",
                encode(&SetForkRequest { index: 2, config: fork })
            ),
        ];
        let view: alloc::vec::Vec<(&str, &[u8])> = entries.iter().map(|(l, b)| (*l, b.as_slice())).collect();

        let actual = snapshot::format_value_snapshot("snapshots/wire_values.snap", &view);
        snapshot::assert_snapshot("snapshots/wire_values.snap", actual);
    }
}
