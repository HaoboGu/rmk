//! Loopback integration test for the Rynk dispatch core.
//!
//! Round-trips `GetVersion` and `GetCapabilities` through
//! `RynkService::dispatch`, asserting both the response header (echoed
//! SEQ, correct CMD, correct LEN) and the decoded payload. Verifies the
//! Phase 2 service core without depending on any transport adapter.

#![cfg(feature = "rynk")]

pub mod common;

use rmk::config::{BehaviorConfig, PositionalConfig};
use rmk::host::RynkService;
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};
use rmk_types::protocol::rynk::header::HEADER_SIZE;
use rmk_types::led_indicator::LedIndicator;
use rmk_types::protocol::rynk::{
    BehaviorConfig as WireBehaviorConfig, Cmd, DeviceCapabilities, Header, KeyPosition, ProtocolVersion,
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

/// Build a request frame: header (5 B) + postcard-encoded payload.
fn build_frame<T: serde::Serialize>(cmd: Cmd, seq: u8, payload: &T) -> Vec<u8> {
    let mut payload_buf = [0u8; 256];
    let n = postcard::to_slice(payload, &mut payload_buf).expect("encode req");
    let header = Header {
        cmd,
        seq,
        len: n.len() as u16,
    };
    let mut out = vec![0u8; HEADER_SIZE + n.len()];
    header.encode_into(&mut out[..HEADER_SIZE]);
    out[HEADER_SIZE..].copy_from_slice(n);
    out
}

/// Parse a response frame: returns header + payload slice.
fn parse_frame(buf: &[u8]) -> (Header, &[u8]) {
    Header::decode(buf).expect("decode response")
}

#[test]
fn dispatch_round_trips_get_version() {
    let service = RynkService::new(tiny_keymap());
    let req = build_frame(Cmd::GetVersion, 0x42, &());
    let mut out = vec![0u8; rmk::host::RYNK_BUFFER_SIZE];
    let n = test_block_on(service.dispatch(&req, &mut out));
    assert!(n > 0, "dispatch returned 0 for GetVersion");
    let (header, payload) = parse_frame(&out[..n]);
    assert_eq!(header.cmd, Cmd::GetVersion);
    assert_eq!(header.seq, 0x42, "SEQ should be echoed");
    let version: ProtocolVersion = postcard::from_bytes(payload).expect("decode ProtocolVersion");
    assert_eq!(version, ProtocolVersion::CURRENT);
}

#[test]
fn dispatch_round_trips_get_capabilities() {
    let service = RynkService::new(tiny_keymap());
    let req = build_frame(Cmd::GetCapabilities, 0x07, &());
    let mut out = vec![0u8; rmk::host::RYNK_BUFFER_SIZE];
    let n = test_block_on(service.dispatch(&req, &mut out));
    assert!(n > 0, "dispatch returned 0 for GetCapabilities");
    let (header, payload) = parse_frame(&out[..n]);
    assert_eq!(header.cmd, Cmd::GetCapabilities);
    assert_eq!(header.seq, 0x07);
    let caps: DeviceCapabilities = postcard::from_bytes(payload).expect("decode DeviceCapabilities");
    // Layout reflects our tiny_keymap: 1 layer × 2 rows × 2 cols.
    assert_eq!(caps.num_layers, 1);
    assert_eq!(caps.num_rows, 2);
    assert_eq!(caps.num_cols, 2);
    assert_eq!(caps.storage_enabled, cfg!(feature = "storage"));
}

#[test]
fn dispatch_rejects_topic_cmd_from_host() {
    let service = RynkService::new(tiny_keymap());
    let req = build_frame(Cmd::LayerChange, 0, &0u8);
    let mut out = vec![0u8; rmk::host::RYNK_BUFFER_SIZE];
    let n = test_block_on(service.dispatch(&req, &mut out));
    assert_eq!(n, 0, "topic CMD from host should produce no response");
}

#[test]
fn dispatch_silently_drops_unknown_frames() {
    let service = RynkService::new(tiny_keymap());
    // 5-byte header with an unknown CMD (0xFFFF reserved as sentinel).
    let bad = [0xFFu8, 0xFFu8, 0x00, 0x00, 0x00];
    let mut out = vec![0u8; rmk::host::RYNK_BUFFER_SIZE];
    let n = test_block_on(service.dispatch(&bad, &mut out));
    assert_eq!(n, 0, "unknown CMD should produce no response");
}

#[test]
fn dispatch_responds_to_get_default_layer() {
    // Real handler: a fresh keymap defaults to layer 0 (postcard-encodes
    // to a single 0x00 byte). Verifies the dispatch path delivers the
    // header plus the payload, with SEQ echoed.
    let service = RynkService::new(tiny_keymap());
    let req = build_frame(Cmd::GetDefaultLayer, 0x11, &());
    let mut out = vec![0u8; rmk::host::RYNK_BUFFER_SIZE];
    let n = test_block_on(service.dispatch(&req, &mut out));
    assert_eq!(n, HEADER_SIZE + 1, "expected 5-byte header + 1-byte u8 payload");
    let (header, payload) = parse_frame(&out[..n]);
    assert_eq!(header.cmd, Cmd::GetDefaultLayer);
    assert_eq!(header.seq, 0x11);
    assert_eq!(header.len, 1);
    let layer: u8 = postcard::from_bytes(payload).expect("decode default layer");
    assert_eq!(layer, 0);
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
    let mut out = vec![0u8; rmk::host::RYNK_BUFFER_SIZE];
    let req = build_frame(Cmd::SetKeyAction, 0x01, &set);
    let n = test_block_on(service.dispatch(&req, &mut out));
    let (hdr, payload) = parse_frame(&out[..n]);
    assert_eq!(hdr.cmd, Cmd::SetKeyAction);
    let r: rmk_types::protocol::rynk::RynkResult = postcard::from_bytes(payload).expect("decode RynkResult");
    assert_eq!(r, Ok(()), "SetKeyAction should accept in-range write");

    // GetKeyAction reads back the value just written.
    let req = build_frame(
        Cmd::GetKeyAction,
        0x02,
        &KeyPosition {
            layer: 0,
            row: 1,
            col: 0,
        },
    );
    let n = test_block_on(service.dispatch(&req, &mut out));
    let (hdr, payload) = parse_frame(&out[..n]);
    assert_eq!(hdr.cmd, Cmd::GetKeyAction);
    let got: KeyAction = postcard::from_bytes(payload).expect("decode KeyAction");
    assert_eq!(got, action);
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
    let req = build_frame(Cmd::SetKeyAction, 0x33, &set);
    let mut out = vec![0u8; rmk::host::RYNK_BUFFER_SIZE];
    let n = test_block_on(service.dispatch(&req, &mut out));
    let (_, payload) = parse_frame(&out[..n]);
    let r: rmk_types::protocol::rynk::RynkResult = postcard::from_bytes(payload).expect("decode RynkResult");
    assert_eq!(r, Err(rmk_types::protocol::rynk::RynkError::InvalidParameter));
}

#[test]
fn dispatch_get_behavior_config_returns_defaults() {
    let service = RynkService::new(tiny_keymap());
    let req = build_frame(Cmd::GetBehaviorConfig, 0x77, &());
    let mut out = vec![0u8; rmk::host::RYNK_BUFFER_SIZE];
    let n = test_block_on(service.dispatch(&req, &mut out));
    let (hdr, payload) = parse_frame(&out[..n]);
    assert_eq!(hdr.cmd, Cmd::GetBehaviorConfig);
    // Just verify it round-trips into the BehaviorConfig wire shape — we
    // don't pin specific timeout values here, since they come from the
    // BehaviorConfig defaults wired into the keymap.
    let _: WireBehaviorConfig = postcard::from_bytes(payload).expect("decode BehaviorConfig");
}

#[test]
fn dispatch_get_current_layer_returns_active() {
    let service = RynkService::new(tiny_keymap());
    let req = build_frame(Cmd::GetCurrentLayer, 0x05, &());
    let mut out = vec![0u8; rmk::host::RYNK_BUFFER_SIZE];
    let n = test_block_on(service.dispatch(&req, &mut out));
    let (_, payload) = parse_frame(&out[..n]);
    let layer: u8 = postcard::from_bytes(payload).expect("decode layer");
    assert_eq!(layer, 0);
}

// The snapshot atomics start at default (0 / false / empty LedIndicator)
// and `run_topic_snapshot` is never spawned in these tests, so the values
// stay at default. The tests pin both wire shape and default payload.

#[test]
fn dispatch_get_wpm_returns_snapshot() {
    let service = RynkService::new(tiny_keymap());
    let req = build_frame(Cmd::GetWpm, 0x09, &());
    let mut out = vec![0u8; rmk::host::RYNK_BUFFER_SIZE];
    let n = test_block_on(service.dispatch(&req, &mut out));
    let (hdr, payload) = parse_frame(&out[..n]);
    assert_eq!(hdr.cmd, Cmd::GetWpm);
    let wpm: u16 = postcard::from_bytes(payload).expect("decode wpm");
    assert_eq!(wpm, 0);
}

#[test]
fn dispatch_get_sleep_state_returns_snapshot() {
    let service = RynkService::new(tiny_keymap());
    let req = build_frame(Cmd::GetSleepState, 0x0A, &());
    let mut out = vec![0u8; rmk::host::RYNK_BUFFER_SIZE];
    let n = test_block_on(service.dispatch(&req, &mut out));
    let (hdr, payload) = parse_frame(&out[..n]);
    assert_eq!(hdr.cmd, Cmd::GetSleepState);
    let sleeping: bool = postcard::from_bytes(payload).expect("decode sleep");
    assert!(!sleeping);
}

#[test]
fn dispatch_get_led_indicator_returns_snapshot() {
    let service = RynkService::new(tiny_keymap());
    let req = build_frame(Cmd::GetLedIndicator, 0x0B, &());
    let mut out = vec![0u8; rmk::host::RYNK_BUFFER_SIZE];
    let n = test_block_on(service.dispatch(&req, &mut out));
    let (hdr, payload) = parse_frame(&out[..n]);
    assert_eq!(hdr.cmd, Cmd::GetLedIndicator);
    let led: LedIndicator = postcard::from_bytes(payload).expect("decode led");
    assert_eq!(led, LedIndicator::from_bits(0));
}
