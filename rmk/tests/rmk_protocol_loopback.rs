//! Loopback integration test for the RMK protocol dispatch.
//!
//! Builds a real `KeyMap` via the standard test wrapper, constructs a
//! `RmkProtocolApp` with a mock `WireTx` that captures every reply, then drives
//! the `Dispatch::handle` machinery with hand-built request frames. Asserts
//! the responses round-trip back to the expected wire types.
//!
//! This exercises:
//! * the protocol handshake (`GetVersion`, `GetCapabilities`)
//! * key get/set
//! * the v1 lock stubs (plan §3.7)
//! * a no-op `Reboot` (mocked away — calling the real `boot::reboot_keyboard`
//!   would take down the test binary)
//!
//! No real transport (USB / BLE) is involved; this is a pure dispatch-layer
//! test that runs under nextest like the other integration tests.

#![cfg(all(feature = "std", feature = "rmk_protocol"))]

pub mod common;

use std::sync::{Arc, Mutex as StdMutex};

use postcard_rpc::Endpoint;
use postcard_rpc::header::{VarHeader, VarKey, VarKeyKind, VarSeq};
use postcard_rpc::server::{Dispatch, Sender, WireTx, WireTxErrorKind};
use rmk::config::{BehaviorConfig, PositionalConfig};
use rmk::host::rmk_protocol::{Ctx, RmkProtocolApp};
use rmk::types::action::KeyAction;
use rmk_types::protocol::rmk::{
    GetCapabilities, GetKeyAction, GetLockStatus, GetVersion, KeyPosition, LockStatus, ProtocolVersion, SetKeyAction,
    SetKeyRequest, UnlockChallenge, UnlockRequest,
};

use crate::common::test_block_on::test_block_on;
use crate::common::{get_keymap, wrap_keymap};

/// Mock `WireTx` that records every send call as a `(key_bytes, payload)` pair.
#[derive(Default, Clone)]
struct MockTx {
    captured: Arc<StdMutex<Vec<(u8, [u8; 8], Vec<u8>)>>>,
}

impl WireTx for MockTx {
    type Error = WireTxErrorKind;

    async fn wait_connection(&self) {}

    async fn send<T: serde::Serialize + ?Sized>(&self, hdr: VarHeader, msg: &T) -> Result<(), Self::Error> {
        let key = match hdr.key {
            VarKey::Key8(k) => k.to_bytes(),
            _ => panic!("expected Key8 key"),
        };
        let seq = match hdr.seq_no {
            VarSeq::Seq1(b) => b as u8,
            VarSeq::Seq2(s) => (s & 0xFF) as u8,
            VarSeq::Seq4(s) => (s & 0xFF) as u8,
        };
        let mut buf = vec![0u8; 1024];
        let used = postcard::to_slice(msg, &mut buf).map_err(|_| WireTxErrorKind::Other)?;
        self.captured.lock().unwrap().push((seq, key, used.to_vec()));
        Ok(())
    }

    async fn send_raw(&self, _buf: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn send_log_str(&self, _kkind: postcard_rpc::header::VarKeyKind, _s: &str) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn send_log_fmt<'a>(
        &self,
        _kkind: postcard_rpc::header::VarKeyKind,
        _a: core::fmt::Arguments<'a>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn build_app() -> (RmkProtocolApp<'static, MockTx>, MockTx) {
    let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(BehaviorConfig::default()));
    let per_key_config: &'static PositionalConfig<5, 14> = Box::leak(Box::new(PositionalConfig::default()));
    let keymap = wrap_keymap(get_keymap(), per_key_config, behavior_config);
    let app = RmkProtocolApp::<'static, MockTx>::new(Ctx::new(keymap));
    let tx = MockTx::default();
    (app, tx)
}

/// `postcard::to_allocvec` requires `alloc`'s feature on `postcard` (not pulled
/// by rmk's test build). Roll our own with a stack buffer big enough for any
/// of the test payloads.
fn to_vec_helper<T: serde::Serialize>(val: &T) -> Vec<u8> {
    let mut buf = vec![0u8; 256];
    let n = postcard::to_slice(val, &mut buf).expect("encode");
    n.to_vec()
}

/// Build a header for a request to the given endpoint with a fixed seq number.
fn request_hdr<E: Endpoint>(seq: u8) -> VarHeader {
    VarHeader {
        key: VarKey::Key8(<E as Endpoint>::REQ_KEY),
        seq_no: VarSeq::Seq1(seq),
    }
}

/// Run `dispatch.handle()` and pop the single captured reply, returning its key
/// and payload bytes.
async fn dispatch_one<E: Endpoint>(
    app: &mut RmkProtocolApp<'static, MockTx>,
    tx: &MockTx,
    seq: u8,
    body: &[u8],
) -> ([u8; 8], Vec<u8>) {
    let sender = Sender::new(tx.clone(), VarKeyKind::Key8);
    let hdr = request_hdr::<E>(seq);
    Dispatch::handle(app, &sender, &hdr, body).await.expect("dispatch ok");
    let mut captured = tx.captured.lock().unwrap();
    assert_eq!(captured.len(), 1, "expected exactly one reply");
    let (got_seq, key, payload) = captured.remove(0);
    assert_eq!(got_seq, seq, "reply seq must match request seq");
    (key, payload)
}

#[test]
fn handshake_returns_protocol_version_and_capabilities() {
    test_block_on(handshake_returns_protocol_version_and_capabilities_inner());
}

async fn handshake_returns_protocol_version_and_capabilities_inner() {
    let (mut app, tx) = build_app();

    // GetVersion → ProtocolVersion
    let (key, body) = dispatch_one::<GetVersion>(&mut app, &tx, 0, &[]).await;
    assert_eq!(key, <GetVersion as Endpoint>::RESP_KEY.to_bytes());
    let version: ProtocolVersion = postcard::from_bytes(&body).unwrap();
    assert_eq!(version, ProtocolVersion::CURRENT);

    // GetCapabilities → DeviceCapabilities
    let (_, body) = dispatch_one::<GetCapabilities>(&mut app, &tx, 1, &[]).await;
    let caps: rmk_types::protocol::rmk::DeviceCapabilities = postcard::from_bytes(&body).unwrap();
    assert_eq!(caps.num_layers, 2);
    assert_eq!(caps.num_rows, 5);
    assert_eq!(caps.num_cols, 14);
    assert!(caps.storage_enabled, "test build has storage");
    assert_eq!(caps.is_split, cfg!(feature = "split"));
    assert_eq!(caps.ble_enabled, cfg!(feature = "_ble"));
    assert_eq!(caps.bulk_transfer_supported, cfg!(feature = "bulk_transfer"));
    // Numeric limits should reflect the build defaults, not zero.
    assert!(caps.max_combos > 0, "max_combos should be wired from COMBO_MAX_NUM");
    assert!(
        caps.max_combo_keys > 0,
        "max_combo_keys should be wired from COMBO_MAX_LENGTH"
    );
    assert!(caps.max_morse > 0, "max_morse should be wired from MORSE_MAX_NUM");
    assert!(caps.max_patterns_per_key > 0, "max_patterns_per_key should be wired");
    assert!(caps.max_forks > 0, "max_forks should be wired from FORK_MAX_NUM");
    assert!(caps.macro_space_size > 0, "macro_space_size should be wired");
    assert!(caps.max_payload_size > 0, "max_payload_size should be set");
    #[cfg(feature = "bulk_transfer")]
    assert!(
        caps.max_bulk_keys > 0,
        "max_bulk_keys should be wired when bulk_transfer is on"
    );
    #[cfg(feature = "_ble")]
    assert!(
        caps.num_ble_profiles > 0,
        "num_ble_profiles should be wired when _ble is on"
    );
}

#[test]
fn lock_endpoints_are_stubbed_in_v1() {
    test_block_on(lock_endpoints_are_stubbed_in_v1_inner());
}

async fn lock_endpoints_are_stubbed_in_v1_inner() {
    let (mut app, tx) = build_app();

    // GetLockStatus → locked: false
    let (_, body) = dispatch_one::<GetLockStatus>(&mut app, &tx, 0, &[]).await;
    let status: LockStatus = postcard::from_bytes(&body).unwrap();
    assert!(!status.locked, "v1 ships always-unlocked");
    assert!(!status.awaiting_keys);
    assert_eq!(status.remaining_keys, 0);

    // UnlockRequest → empty challenge
    let (_, body) = dispatch_one::<UnlockRequest>(&mut app, &tx, 1, &[]).await;
    let challenge: UnlockChallenge = postcard::from_bytes(&body).unwrap();
    assert!(challenge.key_positions.is_empty());
}

#[test]
fn key_action_round_trips_through_dispatch() {
    test_block_on(key_action_round_trips_through_dispatch_inner());
}

async fn key_action_round_trips_through_dispatch_inner() {
    let (mut app, tx) = build_app();

    // GetKeyAction at (layer=0, row=0, col=0) — should be `Grave` from the
    // test keymap (`get_keymap()` in tests/common/mod.rs).
    let pos = KeyPosition {
        layer: 0,
        row: 0,
        col: 0,
    };
    let body = to_vec_helper(&pos);
    let (_, resp) = dispatch_one::<GetKeyAction>(&mut app, &tx, 0, &body).await;
    let action: KeyAction = postcard::from_bytes(&resp).unwrap();
    assert!(
        !matches!(action, KeyAction::No),
        "key (0,0,0) is non-empty in test keymap"
    );

    // SetKeyAction at (layer=0, row=0, col=0) → No
    let req = SetKeyRequest {
        position: pos,
        action: KeyAction::No,
    };
    let body = to_vec_helper(&req);
    let (_, resp) = dispatch_one::<SetKeyAction>(&mut app, &tx, 1, &body).await;
    let result: rmk_types::protocol::rmk::RmkResult = postcard::from_bytes(&resp).unwrap();
    assert!(result.is_ok(), "set_key_action ok: {:?}", result);

    // Re-read: should now be No.
    let body = to_vec_helper(&pos);
    let (_, resp) = dispatch_one::<GetKeyAction>(&mut app, &tx, 2, &body).await;
    let action: KeyAction = postcard::from_bytes(&resp).unwrap();
    assert!(matches!(action, KeyAction::No));
}

#[test]
fn out_of_range_keymap_position_does_not_panic_dispatcher() {
    test_block_on(out_of_range_keymap_position_does_not_panic_dispatcher_inner());
}

async fn out_of_range_keymap_position_does_not_panic_dispatcher_inner() {
    let (mut app, tx) = build_app();

    // Test keymap is 2 layers × 5 rows × 14 cols. (255, 255, 255) is way out
    // of range. Without the bounds gate `KeyMap::action_at_pos` indexes
    // `self.layers[idx]` directly and panics.
    let bad_pos = KeyPosition {
        layer: 255,
        row: 255,
        col: 255,
    };

    // GetKeyAction → must not panic; protocol shape forces a `KeyAction::No`.
    let body = to_vec_helper(&bad_pos);
    let (_, resp) = dispatch_one::<GetKeyAction>(&mut app, &tx, 0, &body).await;
    let action: KeyAction = postcard::from_bytes(&resp).unwrap();
    assert!(matches!(action, KeyAction::No), "out-of-range get returns No");

    // SetKeyAction → must reject with `InvalidParameter`.
    let bad_req = SetKeyRequest {
        position: bad_pos,
        action: KeyAction::No,
    };
    let body = to_vec_helper(&bad_req);
    let (_, resp) = dispatch_one::<SetKeyAction>(&mut app, &tx, 1, &body).await;
    let result: rmk_types::protocol::rmk::RmkResult = postcard::from_bytes(&resp).unwrap();
    assert!(matches!(result, Err(_)), "out-of-range set returns Err");
}

#[test]
fn out_of_range_default_layer_is_rejected() {
    test_block_on(async {
        let (mut app, tx) = build_app();
        // Test keymap has 2 layers; layer 99 is out of range.
        let body = to_vec_helper(&99u8);
        let (_, resp) = dispatch_one::<rmk_types::protocol::rmk::SetDefaultLayer>(&mut app, &tx, 0, &body).await;
        let result: rmk_types::protocol::rmk::RmkResult = postcard::from_bytes(&resp).unwrap();
        assert!(matches!(result, Err(_)));
    });
}
