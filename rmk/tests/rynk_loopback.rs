//! Loopback integration test for the Rynk dispatch core.
//!
//! Round-trips a representative sample of Cmds through
//! `RynkService::dispatch`, asserting both the response header (echoed
//! SEQ, correct CMD, correct LEN) and the decoded payload. Verifies the
//! Phase 2 service core without depending on any transport adapter.

#![cfg(feature = "rynk")]

pub mod common;

use rmk::config::{BehaviorConfig, PositionalConfig};
use rmk::host::RynkService;
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};
use rmk_types::constants::RYNK_BUFFER_SIZE;
use rmk_types::led_indicator::LedIndicator;
use rmk_types::protocol::rynk::{
    BehaviorConfig as WireBehaviorConfig, Cmd, DeviceCapabilities, KeyPosition, ProtocolVersion, RynkError, RynkMessage,
    SetKeyRequest,
};

use crate::common::test_block_on::test_block_on;
use crate::common::wrap_keymap;

/// Build a tiny 1-layer 2-row 2-col keymap so the test doesn't depend
/// on the size of the helper module's default keyboard.
fn tiny_keymap() -> &'static rmk::keymap::KeyMap<'static> {
    let behavior: &'static mut BehaviorConfig = Box::leak(Box::new(BehaviorConfig::default()));
    let per_key: &'static PositionalConfig<2, 2> = Box::leak(Box::new(PositionalConfig::default()));
    let keymap = [[[KeyAction::No; 2]; 2]; 1];
    wrap_keymap(keymap, per_key, behavior)
}

/// Build a single-buffer request message ready for in-place `dispatch`.
/// Writes the header fields, postcard-encodes the payload into the
/// buffer's payload region, and patches LEN. Panics on any header op
/// error — the buffer is sized at `RYNK_BUFFER_SIZE` so those are
/// unreachable in practice.
fn make_msg<T: serde::Serialize>(cmd: Cmd, seq: u8, payload: &T) -> Vec<u8> {
    let mut buf = vec![0u8; RYNK_BUFFER_SIZE];
    buf.as_mut_slice().set_cmd(cmd).expect("buffer ≥ RYNK_HEADER_SIZE");
    buf.as_mut_slice().set_seq(seq).expect("buffer ≥ RYNK_HEADER_SIZE");
    let n = postcard::to_slice(
        payload,
        buf.as_mut_slice().payload_mut().expect("buffer ≥ RYNK_HEADER_SIZE"),
    )
    .map(|s| s.len())
    .unwrap_or(0);
    buf.as_mut_slice()
        .set_payload_len(n as u16)
        .expect("buffer ≥ RYNK_HEADER_SIZE");
    buf
}

/// Borrow the response payload slice for decoding. Panics on any
/// header op error — see `make_msg`.
fn response(buf: &[u8]) -> &[u8] {
    let len = buf.payload_len().expect("buffer ≥ RYNK_HEADER_SIZE") as usize;
    &buf.payload().expect("buffer ≥ RYNK_HEADER_SIZE")[..len]
}

#[test]
fn dispatch_round_trips_get_version() {
    let service = RynkService::new(tiny_keymap());
    let mut msg = make_msg(Cmd::GetVersion, 0x42, &());
    test_block_on(service.dispatch(&mut msg));
    assert_eq!(msg.as_slice().cmd().unwrap(), Cmd::GetVersion);
    assert_eq!(msg.as_slice().seq().unwrap(), 0x42, "SEQ should be echoed");
    let payload = response(msg.as_slice());
    let version: Result<ProtocolVersion, RynkError> = postcard::from_bytes(payload).expect("decode envelope");
    assert_eq!(version, Ok(ProtocolVersion::CURRENT));
}

#[test]
fn dispatch_round_trips_get_capabilities() {
    let service = RynkService::new(tiny_keymap());
    let mut msg = make_msg(Cmd::GetCapabilities, 0x07, &());
    test_block_on(service.dispatch(&mut msg));
    assert_eq!(msg.as_slice().cmd().unwrap(), Cmd::GetCapabilities);
    assert_eq!(msg.as_slice().seq().unwrap(), 0x07);
    let payload = response(msg.as_slice());
    let caps: Result<DeviceCapabilities, RynkError> = postcard::from_bytes(payload).expect("decode envelope");
    let caps = caps.expect("Ok envelope");
    // Layout reflects our tiny_keymap: 1 layer × 2 rows × 2 cols.
    assert_eq!(caps.num_layers, 1);
    assert_eq!(caps.num_rows, 2);
    assert_eq!(caps.num_cols, 2);
    assert_eq!(caps.storage_enabled, cfg!(feature = "storage"));
}

#[test]
fn dispatch_rejects_topic_cmd_from_host() {
    let service = RynkService::new(tiny_keymap());
    let mut msg = make_msg(Cmd::LayerChange, 0, &0u8);
    test_block_on(service.dispatch(&mut msg));
    let payload = response(msg.as_slice());
    let r: Result<(), RynkError> = postcard::from_bytes(payload).expect("decode envelope");
    assert_eq!(r, Err(RynkError::InvalidRequest), "topic CMD from host should be rejected");
}

#[test]
fn dispatch_rejects_unknown_cmd() {
    let service = RynkService::new(tiny_keymap());
    // RYNK_BUFFER_SIZE buffer with an unknown CMD (0xFFFF reserved as sentinel).
    let mut bad = vec![0u8; RYNK_BUFFER_SIZE];
    bad[0] = 0xFF;
    bad[1] = 0xFF;
    test_block_on(service.dispatch(bad.as_mut_slice()));
    let payload = response(bad.as_slice());
    let r: Result<(), RynkError> = postcard::from_bytes(payload).expect("decode envelope");
    assert_eq!(r, Err(RynkError::InvalidRequest), "unknown CMD should be rejected");
}

#[test]
fn dispatch_responds_to_get_default_layer() {
    // Real handler: a fresh keymap defaults to layer 0. The Ok envelope
    // adds 1 tag byte; the layer u8 adds 1; total 2 bytes.
    let service = RynkService::new(tiny_keymap());
    let mut msg = make_msg(Cmd::GetDefaultLayer, 0x11, &());
    test_block_on(service.dispatch(&mut msg));
    assert_eq!(msg.as_slice().cmd().unwrap(), Cmd::GetDefaultLayer);
    assert_eq!(msg.as_slice().seq().unwrap(), 0x11);
    let payload = response(msg.as_slice());
    let layer: Result<u8, RynkError> = postcard::from_bytes(payload).expect("decode envelope");
    assert_eq!(layer, Ok(0));
}

#[test]
fn dispatch_get_set_key_action_round_trip() {
    let service = RynkService::new(tiny_keymap());

    // SetKeyAction at (layer=0, row=1, col=0) → KC_A.
    let action = KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A)));
    let set = SetKeyRequest {
        position: KeyPosition {
            layer: 0,
            row: 1,
            col: 0,
        },
        action,
    };
    let mut msg = make_msg(Cmd::SetKeyAction, 0x01, &set);
    test_block_on(service.dispatch(&mut msg));
    assert_eq!(msg.as_slice().cmd().unwrap(), Cmd::SetKeyAction);
    let payload = response(msg.as_slice());
    let r: Result<(), RynkError> = postcard::from_bytes(payload).expect("decode envelope");
    assert_eq!(r, Ok(()), "SetKeyAction should accept in-range write");

    // GetKeyAction reads back the value just written. dispatch overwrote
    // the buffer in place, so rebuild the request from scratch.
    let mut msg = make_msg(
        Cmd::GetKeyAction,
        0x02,
        &KeyPosition {
            layer: 0,
            row: 1,
            col: 0,
        },
    );
    test_block_on(service.dispatch(&mut msg));
    assert_eq!(msg.as_slice().cmd().unwrap(), Cmd::GetKeyAction);
    let payload = response(msg.as_slice());
    let got: Result<KeyAction, RynkError> = postcard::from_bytes(payload).expect("decode envelope");
    assert_eq!(got, Ok(action));
}

#[test]
fn dispatch_set_key_action_rejects_out_of_range() {
    let service = RynkService::new(tiny_keymap());
    // tiny_keymap is 1×2×2, so row 9 is out of range.
    let set = SetKeyRequest {
        position: KeyPosition {
            layer: 0,
            row: 9,
            col: 0,
        },
        action: KeyAction::No,
    };
    let mut msg = make_msg(Cmd::SetKeyAction, 0x33, &set);
    // dispatch writes the error envelope into the buffer.
    test_block_on(service.dispatch(&mut msg));
    let payload = response(msg.as_slice());
    let r: Result<(), RynkError> = postcard::from_bytes(payload).expect("decode envelope");
    assert_eq!(r, Err(RynkError::InvalidRequest));
}

#[test]
fn dispatch_get_behavior_config_returns_defaults() {
    let service = RynkService::new(tiny_keymap());
    let mut msg = make_msg(Cmd::GetBehaviorConfig, 0x77, &());
    test_block_on(service.dispatch(&mut msg));
    assert_eq!(msg.as_slice().cmd().unwrap(), Cmd::GetBehaviorConfig);
    // Just verify it round-trips into the BehaviorConfig wire shape — we
    // don't pin specific timeout values here, since they come from the
    // BehaviorConfig defaults wired into the keymap.
    let payload = response(msg.as_slice());
    let r: Result<WireBehaviorConfig, RynkError> = postcard::from_bytes(payload).expect("decode envelope");
    let _ = r.expect("Ok envelope");
}

#[test]
fn dispatch_get_current_layer_returns_active() {
    let service = RynkService::new(tiny_keymap());
    let mut msg = make_msg(Cmd::GetCurrentLayer, 0x05, &());
    test_block_on(service.dispatch(&mut msg));
    let payload = response(msg.as_slice());
    let layer: Result<u8, RynkError> = postcard::from_bytes(payload).expect("decode envelope");
    assert_eq!(layer, Ok(0));
}

// The topic-cache atomics start at default (0 / false / empty LedIndicator)
// and `run_topic_snapshot` is never spawned in these tests, so the values
// stay at default. The tests pin both wire shape and default payload.

#[test]
fn dispatch_get_wpm_returns_snapshot() {
    let service = RynkService::new(tiny_keymap());
    let mut msg = make_msg(Cmd::GetWpm, 0x09, &());
    test_block_on(service.dispatch(&mut msg));
    assert_eq!(msg.as_slice().cmd().unwrap(), Cmd::GetWpm);
    let payload = response(msg.as_slice());
    let wpm: Result<u16, RynkError> = postcard::from_bytes(payload).expect("decode envelope");
    assert_eq!(wpm, Ok(0));
}

#[test]
fn dispatch_get_sleep_state_returns_snapshot() {
    let service = RynkService::new(tiny_keymap());
    let mut msg = make_msg(Cmd::GetSleepState, 0x0A, &());
    test_block_on(service.dispatch(&mut msg));
    assert_eq!(msg.as_slice().cmd().unwrap(), Cmd::GetSleepState);
    let payload = response(msg.as_slice());
    let sleep: Result<bool, RynkError> = postcard::from_bytes(payload).expect("decode envelope");
    assert_eq!(sleep, Ok(false));
}

#[test]
fn dispatch_get_led_indicator_returns_snapshot() {
    let service = RynkService::new(tiny_keymap());
    let mut msg = make_msg(Cmd::GetLedIndicator, 0x0B, &());
    test_block_on(service.dispatch(&mut msg));
    assert_eq!(msg.as_slice().cmd().unwrap(), Cmd::GetLedIndicator);
    let payload = response(msg.as_slice());
    let led: Result<LedIndicator, RynkError> = postcard::from_bytes(payload).expect("decode envelope");
    assert_eq!(led, Ok(LedIndicator::from_bits(0)));
}
