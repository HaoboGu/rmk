//! Rynk test support: shared serde-test helpers for the submodules, plus
//! the cross-module wire-format tests.
//!
//! Schema drift detection across two golden files: `wire_values.snap` holds
//! one postcard-encoded exemplar per wire type; `wire_frames.snap` holds one
//! full frame (header + payload) per protocol message. Any field reorder /
//! type change / variant renumber / CMD renumber flips the bytes and fails
//! CI. If the change is intentional, bump `ProtocolVersion::CURRENT` and
//! regenerate the snapshots.

extern crate alloc;

use alloc::vec;

use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

use super::*;
use crate::action::{Action, EncoderAction, KeyAction, KeyboardAction, LightAction};
use crate::battery::{BatteryStatus, ChargeState};
use crate::ble::{BleState, BleStatus};
use crate::combo::Combo;
use crate::connection::{ConnectionStatus, ConnectionType, UsbState};
use crate::fork::{Fork, StateBits};
use crate::keycode::{ConsumerKey, HidKeyCode, KeyCode, SpecialKey, SystemControlKey};
use crate::led_indicator::LedIndicator;
use crate::modifier::ModifierCombination;
use crate::morse::{Morse, MorseMode, MorseProfile, TAP};
use crate::mouse_button::MouseButtons;

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

mod snapshot {
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
    /// `title` heads the file and `blurb` (already `#`-prefixed lines) describes
    /// its entries; `test_filter` names the test in the regenerate hint.
    pub fn format_value_snapshot(
        rel_path: &str,
        title: &str,
        blurb: &str,
        test_filter: &str,
        entries: &[(&str, &[u8])],
    ) -> String {
        let mut sorted: Vec<&(&str, &[u8])> = entries.iter().collect();
        sorted.sort_by_key(|(label, _)| *label);

        let label_width = sorted.iter().map(|(l, _)| l.len()).max().unwrap_or(0);

        let mut out = String::new();
        out.push_str(&format!(
            "# {title} — DO NOT edit by hand.\n\
             # File: {rel_path}\n\
             {blurb}\n\
             #   UPDATE_SNAPSHOTS=1 cargo test -p rmk-types --features rynk {test_filter}\n\
             # Format: <label>  <hex bytes>\n\
             \n",
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

#[test]
fn round_trip_rynk_error_and_result() {
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

fn encode_frame<T: serde::Serialize>(cmd: Cmd, seq: u8, val: &T) -> alloc::vec::Vec<u8> {
    let mut buf = [0u8; 256];
    let len = RynkMessage::build(&mut buf, cmd, seq, val).expect("frame").frame_len();
    buf[..len].to_vec()
}

/// Composite wire exemplars shared by both the type and frame snapshots, so
/// a combo / fork / morse / capabilities value encodes to the same bytes in
/// both files. Distinct, ascending per-field values let a field reorder flip
/// the bytes.
struct Exemplars {
    matrix: MatrixState,
    capabilities: DeviceCapabilities,
    behavior: BehaviorConfig,
    connection: ConnectionStatus,
    state_bits: StateBits,
    combo: Combo,
    fork: Fork,
    morse: Morse,
    macro_data: MacroData,
    encoder: EncoderAction,
}

fn exemplars() -> Exemplars {
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
    let encoder = EncoderAction::new(KeyAction::Morse(3), KeyAction::No);

    Exemplars {
        matrix,
        capabilities,
        behavior,
        connection,
        state_bits,
        combo,
        fork,
        morse,
        macro_data,
        encoder,
    }
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
/// same snapshot. Full frames are pinned separately in `wire_frames_locked`.
#[test]
fn wire_values_locked() {
    let ex = exemplars();

    // Values-only exemplars (no frame counterpart).
    let mut unlock_keys: heapless::Vec<(u8, u8), UNLOCK_KEYS_SIZE> = heapless::Vec::new();
    unlock_keys.push((1, 2)).unwrap();
    unlock_keys.push((3, 4)).unwrap();
    let unlock = UnlockChallenge {
        key_positions: unlock_keys,
    };
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
        ("EncoderAction{Morse(3),No}", encode(&ex.encoder)),
        ("Combo{[Single(A)],Morse(1),L2}", encode(&ex.combo)),
        ("Fork{Single(A),No,Morse(2)}", encode(&ex.fork)),
        ("StateBits{LCtrl,Caps,B1}", encode(&ex.state_bits)),
        ("Morse{TAP->Key(A)}", encode(&ex.morse)),
        ("MacroData{[0x01,0x02,0x03]}", encode(&ex.macro_data)),
        // --- Status / system responses ---
        ("MatrixState{[0x05,0x00,0x20]}", encode(&ex.matrix)),
        ("DeviceCapabilities{1..16}", encode(&ex.capabilities)),
        ("BehaviorConfig{50,500,200,20}", encode(&ex.behavior)),
        ("ConnectionStatus{Configured,{1,Adv},Ble}", encode(&ex.connection)),
        ("ProtocolVersion{1,0}", encode(&ProtocolVersion { major: 1, minor: 0 })),
        ("ProtocolVersion::CURRENT", encode(&ProtocolVersion::CURRENT)),
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
                action: ex.encoder
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
                data: ex.macro_data.clone()
            }),
        ),
        (
            "SetComboRequest{3,combo}",
            encode(&SetComboRequest {
                index: 3,
                config: ex.combo.clone()
            })
        ),
        (
            "SetMorseRequest{0,morse}",
            encode(&SetMorseRequest {
                index: 0,
                config: ex.morse.clone()
            })
        ),
        (
            "SetForkRequest{2,fork}",
            encode(&SetForkRequest {
                index: 2,
                config: ex.fork
            })
        ),
    ];
    let view: alloc::vec::Vec<(&str, &[u8])> = entries.iter().map(|(l, b)| (*l, b.as_slice())).collect();

    let actual = snapshot::format_value_snapshot(
        "snapshots/wire_values.snap",
        "Wire-format TYPE snapshot",
        "# Each entry is the postcard byte encoding of one wire-type exemplar. A diff\n\
         # here means a type's payload encoding changed (field reorder, variant\n\
         # renumber, …). If intentional, bump ProtocolVersion::CURRENT and regenerate:",
        "wire_values",
        &view,
    );
    snapshot::assert_snapshot("snapshots/wire_values.snap", actual);
}

/// Lock down full Rynk frames — the 5-byte header (CMD u16 LE + SEQ u8 +
/// LEN u16 LE) plus postcard payload — one per feature-independent protocol
/// message: every request, its `Ok` reply, a representative `Err` reply, and
/// every topic push. A diff here means the header layout, a `Cmd` number,
/// the `Result<T, RynkError>` reply envelope, or a message's frame changed;
/// if intentional, regenerate and bump `ProtocolVersion::CURRENT`.
///
/// Requests and replies use SEQ 1 (a reply echoes its request's SEQ); topics
/// always use SEQ 0. The `GetVersion` probe and reply are frozen across all
/// majors. Feature-gated commands (`bulk`, `_ble`, split) are excluded so
/// every `rynk` feature set yields the same file. Payloads reuse the shared
/// [`exemplars`], so a frame and its bare-payload entry in
/// `wire_values.snap` stay in lockstep.
#[test]
fn wire_frames_locked() {
    let ex = exemplars();

    // Request seq; a reply echoes it. Topics are always seq 0.
    const SEQ: u8 = 1;
    let key_pos = KeyPosition {
        layer: 0,
        row: 5,
        col: 13,
    };
    let set_key = SetKeyRequest {
        position: key_pos,
        action: KeyAction::Morse(7),
    };
    let led = LedIndicator::NUM_LOCK | LedIndicator::SCROLL_LOCK;

    let entries: alloc::vec::Vec<(&str, alloc::vec::Vec<u8>)> = alloc::vec![
        // ── System (0x00xx) ──
        ("GetVersion request ()", encode_frame(Cmd::GetVersion, SEQ, &())),
        (
            "GetVersion reply Ok(CURRENT)",
            encode_frame(
                Cmd::GetVersion,
                SEQ,
                &Ok::<ProtocolVersion, RynkError>(ProtocolVersion::CURRENT)
            ),
        ),
        (
            "GetCapabilities request ()",
            encode_frame(Cmd::GetCapabilities, SEQ, &())
        ),
        (
            "GetCapabilities reply Ok(DeviceCapabilities{1..16})",
            encode_frame(
                Cmd::GetCapabilities,
                SEQ,
                &Ok::<DeviceCapabilities, RynkError>(ex.capabilities)
            ),
        ),
        ("Reboot request ()", encode_frame(Cmd::Reboot, SEQ, &())),
        (
            "Reboot reply Ok(())",
            encode_frame(Cmd::Reboot, SEQ, &Ok::<(), RynkError>(()))
        ),
        ("BootloaderJump request ()", encode_frame(Cmd::BootloaderJump, SEQ, &())),
        (
            "BootloaderJump reply Ok(())",
            encode_frame(Cmd::BootloaderJump, SEQ, &Ok::<(), RynkError>(())),
        ),
        (
            "StorageReset request StorageResetMode::Full",
            encode_frame(Cmd::StorageReset, SEQ, &StorageResetMode::Full)
        ),
        (
            "StorageReset reply Ok(())",
            encode_frame(Cmd::StorageReset, SEQ, &Ok::<(), RynkError>(()))
        ),
        // ── Keymap / encoder (0x01xx) ──
        (
            "GetKeyAction request KeyPosition{0,5,13}",
            encode_frame(Cmd::GetKeyAction, SEQ, &key_pos)
        ),
        (
            "GetKeyAction reply Ok(Morse(7))",
            encode_frame(Cmd::GetKeyAction, SEQ, &Ok::<KeyAction, RynkError>(KeyAction::Morse(7))),
        ),
        (
            "SetKeyAction request SetKeyRequest{{0,5,13},Morse(7)}",
            encode_frame(Cmd::SetKeyAction, SEQ, &set_key)
        ),
        (
            "SetKeyAction reply Ok(())",
            encode_frame(Cmd::SetKeyAction, SEQ, &Ok::<(), RynkError>(()))
        ),
        (
            "SetKeyAction reply Err(Invalid)",
            encode_frame(Cmd::SetKeyAction, SEQ, &Err::<(), RynkError>(RynkError::Invalid)),
        ),
        (
            "GetDefaultLayer request ()",
            encode_frame(Cmd::GetDefaultLayer, SEQ, &())
        ),
        (
            "GetDefaultLayer reply Ok(2)",
            encode_frame(Cmd::GetDefaultLayer, SEQ, &Ok::<u8, RynkError>(2)),
        ),
        (
            "SetDefaultLayer request 2",
            encode_frame(Cmd::SetDefaultLayer, SEQ, &2u8)
        ),
        (
            "SetDefaultLayer reply Ok(())",
            encode_frame(Cmd::SetDefaultLayer, SEQ, &Ok::<(), RynkError>(())),
        ),
        (
            "GetEncoderAction request GetEncoderRequest{1,2}",
            encode_frame(
                Cmd::GetEncoderAction,
                SEQ,
                &GetEncoderRequest {
                    encoder_id: 1,
                    layer: 2
                }
            ),
        ),
        (
            "GetEncoderAction reply Ok(EncoderAction{Morse(3),No})",
            encode_frame(Cmd::GetEncoderAction, SEQ, &Ok::<EncoderAction, RynkError>(ex.encoder)),
        ),
        (
            "SetEncoderAction request SetEncoderRequest{1,2,{Morse(3),No}}",
            encode_frame(
                Cmd::SetEncoderAction,
                SEQ,
                &SetEncoderRequest {
                    encoder_id: 1,
                    layer: 2,
                    action: ex.encoder
                },
            ),
        ),
        (
            "SetEncoderAction reply Ok(())",
            encode_frame(Cmd::SetEncoderAction, SEQ, &Ok::<(), RynkError>(())),
        ),
        // ── Macro (0x02xx) ──
        (
            "GetMacro request GetMacroRequest{1,256}",
            encode_frame(Cmd::GetMacro, SEQ, &GetMacroRequest { index: 1, offset: 256 }),
        ),
        (
            "GetMacro reply Ok(MacroData{[0x01,0x02,0x03]})",
            encode_frame(Cmd::GetMacro, SEQ, &Ok::<MacroData, RynkError>(ex.macro_data.clone())),
        ),
        (
            "SetMacro request SetMacroRequest{1,2,[0x01,0x02,0x03]}",
            encode_frame(
                Cmd::SetMacro,
                SEQ,
                &SetMacroRequest {
                    index: 1,
                    offset: 2,
                    data: ex.macro_data.clone()
                },
            ),
        ),
        (
            "SetMacro reply Ok(())",
            encode_frame(Cmd::SetMacro, SEQ, &Ok::<(), RynkError>(()))
        ),
        // ── Combo (0x03xx) ──
        ("GetCombo request 3", encode_frame(Cmd::GetCombo, SEQ, &3u8)),
        (
            "GetCombo reply Ok(Combo{[Single(A)],Morse(1),L2})",
            encode_frame(Cmd::GetCombo, SEQ, &Ok::<Combo, RynkError>(ex.combo.clone())),
        ),
        (
            "SetCombo request SetComboRequest{3,combo}",
            encode_frame(
                Cmd::SetCombo,
                SEQ,
                &SetComboRequest {
                    index: 3,
                    config: ex.combo.clone()
                }
            ),
        ),
        (
            "SetCombo reply Ok(())",
            encode_frame(Cmd::SetCombo, SEQ, &Ok::<(), RynkError>(()))
        ),
        // ── Morse (0x04xx) ──
        ("GetMorse request 0", encode_frame(Cmd::GetMorse, SEQ, &0u8)),
        (
            "GetMorse reply Ok(Morse{TAP->Key(A)})",
            encode_frame(Cmd::GetMorse, SEQ, &Ok::<Morse, RynkError>(ex.morse.clone())),
        ),
        (
            "SetMorse request SetMorseRequest{0,morse}",
            encode_frame(
                Cmd::SetMorse,
                SEQ,
                &SetMorseRequest {
                    index: 0,
                    config: ex.morse.clone()
                }
            ),
        ),
        (
            "SetMorse reply Ok(())",
            encode_frame(Cmd::SetMorse, SEQ, &Ok::<(), RynkError>(()))
        ),
        // ── Fork (0x05xx) ──
        ("GetFork request 2", encode_frame(Cmd::GetFork, SEQ, &2u8)),
        (
            "GetFork reply Ok(Fork{Single(A),No,Morse(2)})",
            encode_frame(Cmd::GetFork, SEQ, &Ok::<Fork, RynkError>(ex.fork))
        ),
        (
            "SetFork request SetForkRequest{2,fork}",
            encode_frame(
                Cmd::SetFork,
                SEQ,
                &SetForkRequest {
                    index: 2,
                    config: ex.fork
                }
            ),
        ),
        (
            "SetFork reply Ok(())",
            encode_frame(Cmd::SetFork, SEQ, &Ok::<(), RynkError>(()))
        ),
        // ── Behavior (0x06xx) ──
        (
            "GetBehaviorConfig request ()",
            encode_frame(Cmd::GetBehaviorConfig, SEQ, &())
        ),
        (
            "GetBehaviorConfig reply Ok(BehaviorConfig{50,500,200,20})",
            encode_frame(
                Cmd::GetBehaviorConfig,
                SEQ,
                &Ok::<BehaviorConfig, RynkError>(ex.behavior)
            ),
        ),
        (
            "SetBehaviorConfig request BehaviorConfig{50,500,200,20}",
            encode_frame(Cmd::SetBehaviorConfig, SEQ, &ex.behavior)
        ),
        (
            "SetBehaviorConfig reply Ok(())",
            encode_frame(Cmd::SetBehaviorConfig, SEQ, &Ok::<(), RynkError>(())),
        ),
        // ── Connection (0x07xx) ──
        (
            "GetConnectionType request ()",
            encode_frame(Cmd::GetConnectionType, SEQ, &())
        ),
        (
            "GetConnectionType reply Ok(Ble)",
            encode_frame(
                Cmd::GetConnectionType,
                SEQ,
                &Ok::<ConnectionType, RynkError>(ConnectionType::Ble)
            ),
        ),
        (
            "GetConnectionStatus request ()",
            encode_frame(Cmd::GetConnectionStatus, SEQ, &())
        ),
        (
            "GetConnectionStatus reply Ok(ConnectionStatus{Configured,{1,Adv},Ble})",
            encode_frame(
                Cmd::GetConnectionStatus,
                SEQ,
                &Ok::<ConnectionStatus, RynkError>(ex.connection)
            ),
        ),
        // ── Status (0x08xx) ──
        (
            "GetCurrentLayer request ()",
            encode_frame(Cmd::GetCurrentLayer, SEQ, &())
        ),
        (
            "GetCurrentLayer reply Ok(1)",
            encode_frame(Cmd::GetCurrentLayer, SEQ, &Ok::<u8, RynkError>(1)),
        ),
        ("GetMatrixState request ()", encode_frame(Cmd::GetMatrixState, SEQ, &())),
        (
            "GetMatrixState reply Ok(MatrixState{[0x05,0x00,0x20]})",
            encode_frame(
                Cmd::GetMatrixState,
                SEQ,
                &Ok::<MatrixState, RynkError>(ex.matrix.clone())
            ),
        ),
        ("GetWpm request ()", encode_frame(Cmd::GetWpm, SEQ, &())),
        (
            "GetWpm reply Ok(42)",
            encode_frame(Cmd::GetWpm, SEQ, &Ok::<u16, RynkError>(42))
        ),
        ("GetSleepState request ()", encode_frame(Cmd::GetSleepState, SEQ, &())),
        (
            "GetSleepState reply Ok(true)",
            encode_frame(Cmd::GetSleepState, SEQ, &Ok::<bool, RynkError>(true)),
        ),
        (
            "GetLedIndicator request ()",
            encode_frame(Cmd::GetLedIndicator, SEQ, &())
        ),
        (
            "GetLedIndicator reply Ok(LedIndicator(Num|Scroll))",
            encode_frame(Cmd::GetLedIndicator, SEQ, &Ok::<LedIndicator, RynkError>(led)),
        ),
        // ── Topics (0x80xx, server→host push, SEQ 0) ──
        ("LayerChange topic 3", encode_frame(Cmd::LayerChange, 0, &3u8)),
        ("WpmUpdate topic 42", encode_frame(Cmd::WpmUpdate, 0, &42u16)),
        (
            "ConnectionChange topic ConnectionStatus{Configured,{1,Adv},Ble}",
            encode_frame(Cmd::ConnectionChange, 0, &ex.connection)
        ),
        ("SleepState topic true", encode_frame(Cmd::SleepState, 0, &true)),
        (
            "LedIndicatorChange topic LedIndicator(Num|Scroll)",
            encode_frame(Cmd::LedIndicatorChange, 0, &led)
        ),
    ];
    let view: alloc::vec::Vec<(&str, &[u8])> = entries.iter().map(|(l, b)| (*l, b.as_slice())).collect();

    let actual = snapshot::format_value_snapshot(
        "snapshots/wire_frames.snap",
        "Wire-format FRAME snapshot",
        "# Each entry is a full Rynk frame — 5-byte header (CMD u16 LE + SEQ u8 + LEN\n\
         # u16 LE) + postcard payload — one per protocol message; the label names the\n\
         # decoded payload (`()` = empty). A diff means the header, a CMD number, or a\n\
         # message frame changed. If intentional, bump ProtocolVersion::CURRENT and regenerate:",
        "wire_frames",
        &view,
    );
    snapshot::assert_snapshot("snapshots/wire_frames.snap", actual);
}
