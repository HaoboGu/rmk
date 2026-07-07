//! End-to-end loopback integration test for the Rynk host service.
//!
//! Every case runs the production [`RynkService::run_session`] over an in-memory
//! embedded-io duplex (see [`common::rynk_link`]) and drives it with a host-side
//! `RynkClient`. Requests and responses cross the full framing/codec path —
//! parse → dispatch → handler → response-encode → framing — plus the topic-emit
//! and oversized-frame resync arms of the session loop. Nothing here touches a
//! hardware transport, so the whole Rynk stack is tested in isolation.
//!
//! Coverage is exhaustive: there is at least one case per dispatch arm in
//! `RynkService::handle` (happy path where the mock keymap allows it, the
//! `Invalid`/`Unimplemented` arm otherwise) and one case per topic push.

#![cfg(feature = "rynk")]

pub mod common;

use heapless::Vec as HVec;
use rmk::config::{BehaviorConfig, PositionalConfig, RmkConfig};
#[cfg(feature = "_ble")]
use rmk::event::BatteryStatusEvent;
use rmk::event::{
    ConnectionStatusChangeEvent, LayerChangeEvent, LedIndicatorEvent, SleepStateEvent, WpmUpdateEvent, publish_event,
};
use rmk::host::HostService as RynkService;
use rmk::types::action::{Action, EncoderAction, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};
#[cfg(feature = "_ble")]
use rmk_types::battery::{BatteryStatus, ChargeState};
#[cfg(feature = "_ble")]
use rmk_types::ble::BleStatus;
use rmk_types::combo::Combo as ComboConfig;
use rmk_types::connection::{ConnectionStatus, ConnectionType};
use rmk_types::constants::RYNK_BUFFER_SIZE;
use rmk_types::fork::Fork;
use rmk_types::led_indicator::LedIndicator;
use rmk_types::morse::{Morse, MorseMode, MorseProfile};
use rmk_types::protocol::rynk::{
    BehaviorConfig as WireBehaviorConfig, Cmd, DeviceCapabilities, DeviceInfo, GetEncoderRequest, GetMacroRequest,
    KeyPosition, LockStatus, MacroData, MatrixState, ProtocolVersion, RYNK_HEADER_SIZE, RynkError, SetComboRequest,
    SetEncoderRequest, SetForkRequest, SetKeyRequest, SetMacroRequest, SetMorseRequest, StorageResetMode,
};

use crate::common::rynk_link::{link_session, link_two_sessions};
use crate::common::{wrap_keymap, wrap_keymap_with_encoders};

/// Leak an `insecure` (always-unlocked) config so these loopback cases exercise
/// protocol mechanics without the lock gate intercepting `BootloaderJump` /
/// `StorageReset` / `GetMatrixState`. The gate itself is covered by the
/// `host::rynk` and `host::lock` unit tests.
fn insecure_config() -> &'static RmkConfig<'static> {
    let mut config = RmkConfig::default();
    config.lock_config.insecure = true;
    Box::leak(Box::new(config))
}

/// Build a `RynkService` over a tiny 1-layer 2-row 2-col keymap, so the tests
/// don't depend on the size of the helper module's default keyboard.
fn service() -> RynkService<'static> {
    let behavior: &'static mut BehaviorConfig = Box::leak(Box::new(BehaviorConfig::default()));
    let per_key: &'static PositionalConfig<2, 2> = Box::leak(Box::new(PositionalConfig::default()));
    let keymap = [[[KeyAction::No; 2]; 2]; 1];
    let km = wrap_keymap(keymap, per_key, behavior);
    RynkService::new(km, insecure_config())
}

/// A 2-layer variant, so SetDefaultLayer can move the default off layer 0 and
/// the readback can actually observe the write (the 1-layer keymap can't —
/// layer 0 is both the default and the only valid value).
fn service_2_layers() -> RynkService<'static> {
    let behavior: &'static mut BehaviorConfig = Box::leak(Box::new(BehaviorConfig::default()));
    let per_key: &'static PositionalConfig<2, 2> = Box::leak(Box::new(PositionalConfig::default()));
    let keymap = [[[KeyAction::No; 2]; 2]; 2];
    let km = wrap_keymap(keymap, per_key, behavior);
    RynkService::new(km, insecure_config())
}

/// A keymap with 2 encoders, so `GetCapabilities` can report a non-zero
/// `num_encoders` and the encoder endpoints become reachable.
fn service_with_encoders() -> RynkService<'static> {
    let behavior: &'static mut BehaviorConfig = Box::leak(Box::new(BehaviorConfig::default()));
    let per_key: &'static PositionalConfig<2, 2> = Box::leak(Box::new(PositionalConfig::default()));
    let keymap = [[[KeyAction::No; 2]; 2]; 1];
    let encoder_map = [[EncoderAction::default(); 2]; 1];
    let km = wrap_keymap_with_encoders(keymap, encoder_map, per_key, behavior);
    RynkService::new(km, insecure_config())
}

/// A single-layer 3-row 4-col keymap (12 keys) for the bulk endpoints: small
/// enough that runs hit the keymap's edge, and 4-key rows make a run wrap a row
/// boundary early. Larger runs use `service_3x4x4`.
#[cfg(feature = "bulk")]
fn service_3x4() -> RynkService<'static> {
    let behavior: &'static mut BehaviorConfig = Box::leak(Box::new(BehaviorConfig::default()));
    let per_key: &'static PositionalConfig<3, 4> = Box::leak(Box::new(PositionalConfig::default()));
    let keymap = [[[KeyAction::No; 4]; 3]; 1];
    let km = wrap_keymap(keymap, per_key, behavior);
    let config: &'static RmkConfig<'static> = Box::leak(Box::new(RmkConfig::default()));
    RynkService::new(km, config)
}

/// A 4-layer 3-row 4-col keymap: 48 keys total, more than one `BULK_KEYMAP_SIZE`
/// run, so the max-capacity and over-budget tests can exercise a full run (and a
/// run one past the budget) without the keymap's total size being the limiter.
#[cfg(feature = "bulk")]
fn service_3x4x4() -> RynkService<'static> {
    let behavior: &'static mut BehaviorConfig = Box::leak(Box::new(BehaviorConfig::default()));
    let per_key: &'static PositionalConfig<3, 4> = Box::leak(Box::new(PositionalConfig::default()));
    let keymap = [[[KeyAction::No; 4]; 3]; 4];
    let km = wrap_keymap(keymap, per_key, behavior);
    let config: &'static RmkConfig<'static> = Box::leak(Box::new(RmkConfig::default()));
    RynkService::new(km, config)
}

/// Distinct action per bulk slot, so a write landing in the wrong position
/// shows up as the wrong keycode rather than a lucky match. HID usage 4 is `A`,
/// so slots 0.. map to A, B, C, … — distinct across a full `BULK_KEYMAP_SIZE`
/// run (usages 4..=0x73 are all valid keys).
#[cfg(feature = "bulk")]
fn bulk_action(i: usize) -> KeyAction {
    KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::from(4 + i as u8))))
}

// ─────────────────────────────────────────────────────────────────────────
// System  (0x00xx)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn get_version() {
    let service = service();
    link_session(&service, async |client| {
        let version = client.request::<(), ProtocolVersion>(Cmd::GetVersion, 0x42, &()).await;
        assert_eq!(version, Ok(ProtocolVersion::CURRENT));
    });
}

#[test]
fn get_capabilities() {
    let service = service();
    link_session(&service, async |client| {
        let caps = client
            .request::<(), DeviceCapabilities>(Cmd::GetCapabilities, 0x07, &())
            .await
            .expect("Ok envelope");
        // Layout reflects service()'s tiny keymap: 1 layer × 2 rows × 2 cols.
        assert_eq!(caps.num_layers, 1);
        assert_eq!(caps.num_rows, 2);
        assert_eq!(caps.num_cols, 2);
        assert_eq!(caps.num_encoders, 0);
        // Concrete protocol limits (default config) — these pin real values, so
        // a handler reporting the wrong limit is caught, not just "decodes".
        assert_eq!(caps.max_combos, 8);
        assert_eq!(caps.max_morse, 8);
        assert_eq!(caps.max_forks, 8);
        assert_eq!(caps.macro_space_size, 256);
        assert_eq!(caps.macro_chunk_size, 64);
        // Feature flags must track the compiled feature set. The suite runs this
        // under both the all-on combo and a rynk-only combo (see test_all.sh),
        // so these are exercised in both the true and false states.
        assert_eq!(caps.storage_enabled, cfg!(feature = "storage"));
        assert_eq!(caps.ble_enabled, cfg!(feature = "_ble"));
        assert_eq!(caps.is_split, cfg!(feature = "split"));
        // Bulk advertisement tracks the compiled feature — the flag and both
        // per-message budgets move together. Keys and configs have separate
        // budgets (keys are far smaller, so they chunk in larger runs).
        assert_eq!(caps.bulk_transfer_supported, cfg!(feature = "bulk"));
        #[cfg(feature = "bulk")]
        {
            assert_eq!(caps.max_bulk_keys as usize, rmk_types::constants::BULK_KEYMAP_SIZE);
            assert_eq!(caps.max_bulk_configs as usize, rmk_types::constants::BULK_SIZE);
        }
        #[cfg(not(feature = "bulk"))]
        {
            assert_eq!(caps.max_bulk_keys, 0);
            assert_eq!(caps.max_bulk_configs, 0);
        }
    });
}

#[test]
fn get_device_info() {
    let service = service();
    link_session(&service, async |client| {
        let info = client
            .request::<(), DeviceInfo>(Cmd::GetDeviceInfo, 0x08, &())
            .await
            .expect("Ok envelope");
        // service() passes RmkConfig::default(), so the identity is the default
        // DeviceConfig, and rmk_version tracks this crate's own version.
        assert_eq!(info.vendor_id, 0x4c4b);
        assert_eq!(info.product_id, 0x4643);
        assert_eq!(info.manufacturer.as_str(), "RMK");
        assert_eq!(info.product_name.as_str(), "RMK Keyboard");
        assert_eq!(info.serial_number.as_str(), "vial:f64c2b3c:000001");
        assert_eq!(info.rmk_version.major, env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap());
        assert_eq!(info.rmk_version.minor, env!("CARGO_PKG_VERSION_MINOR").parse().unwrap());
        assert_eq!(info.rmk_version.patch, env!("CARGO_PKG_VERSION_PATCH").parse().unwrap());
    });
}

#[test]
fn reboot_acks_where_reset_is_a_no_op() {
    // On real hardware `reboot_keyboard()` never returns and no reply is sent
    // (the host's `reboot()` is send-only). On targets where the reset is a
    // no-op — like this test host — the handler falls through to the standard
    // `Ok(())` envelope (`request` asserts the cmd + seq echo).
    let service = service();
    link_session(&service, async |client| {
        let r = client.request::<(), ()>(Cmd::Reboot, 0x60, &()).await;
        assert_eq!(r, Ok(()));
    });
}

#[test]
fn bootloader_jump_acks_where_jump_is_a_no_op() {
    // Same contract as `reboot_acks_where_reset_is_a_no_op`.
    let service = service();
    link_session(&service, async |client| {
        let r = client.request::<(), ()>(Cmd::BootloaderJump, 0x61, &()).await;
        assert_eq!(r, Ok(()));
    });
}

#[test]
fn storage_reset_acks() {
    let service = service();
    link_session(&service, async |client| {
        let r = client
            .request::<StorageResetMode, ()>(Cmd::StorageReset, 0x62, &StorageResetMode::Full)
            .await;
        assert_eq!(r, Ok(()));
    });
}

#[test]
fn storage_reset_rejects_layout_only_until_implemented() {
    // `LayoutOnly` semantics aren't wired yet (`reset_storage` is always a
    // Full wipe, bonds included); the handler rejects the request instead of
    // silently over-wiping.
    let service = service();
    link_session(&service, async |client| {
        let r = client
            .request::<StorageResetMode, ()>(Cmd::StorageReset, 0x63, &StorageResetMode::LayoutOnly)
            .await;
        assert_eq!(r, Err(RynkError::Unimplemented));
    });
}

// ─────────────────────────────────────────────────────────────────────────
// Keymap + encoder  (0x01xx)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn get_set_key_action_round_trip() {
    let service = service();
    // Both requests share one session — Set is dispatched, then Get reads back
    // the value over the same wire, pipelined behind the Set response.
    link_session(&service, async |client| {
        let position = KeyPosition {
            layer: 0,
            row: 1,
            col: 0,
        };
        let action = KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A)));

        let set = SetKeyRequest { position, action };
        let r = client.request::<_, ()>(Cmd::SetKeyAction, 0x01, &set).await;
        assert_eq!(r, Ok(()), "SetKeyAction should accept an in-range write");

        let got = client.request::<_, KeyAction>(Cmd::GetKeyAction, 0x02, &position).await;
        assert_eq!(got, Ok(action), "GetKeyAction should read back what Set wrote");
    });
}

/// Two `run_session`s over one shared `RynkService` — the production shape when
/// a board runs the BLE-GATT and BLE-HID (`RynkHidService`) sessions
/// concurrently. Both build their own `TopicSubscribers` against the global
/// event channels and share one `KeyMap`; this must not panic (subscriber
/// overflow or RefCell double-borrow) and writes from one session must be
/// visible to the other.
#[test]
fn concurrent_sessions_share_one_service() {
    let service = service_2_layers();
    link_two_sessions(&service, async |a, b| {
        let position = KeyPosition {
            layer: 1,
            row: 0,
            col: 0,
        };
        let key_a = KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A)));
        let key_b = KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::B)));

        // Write from session A, then session B, over the shared KeyMap.
        let set_a = SetKeyRequest {
            position,
            action: key_a,
        };
        assert_eq!(a.request::<_, ()>(Cmd::SetKeyAction, 0x11, &set_a).await, Ok(()));
        let set_b = SetKeyRequest {
            position,
            action: key_b,
        };
        assert_eq!(b.request::<_, ()>(Cmd::SetKeyAction, 0x21, &set_b).await, Ok(()));

        // Both sessions read the same shared state — B's write (the last) wins.
        let from_a = a.request::<_, KeyAction>(Cmd::GetKeyAction, 0x12, &position).await;
        let from_b = b.request::<_, KeyAction>(Cmd::GetKeyAction, 0x22, &position).await;
        assert_eq!(
            from_a,
            Ok(key_b),
            "session A observes session B's write (shared KeyMap)"
        );
        assert_eq!(from_b, Ok(key_b), "session B reads back its own write");
    });
}

#[test]
fn get_key_action_rejects_out_of_range() {
    let service = service();
    link_session(&service, async |client| {
        let pos = KeyPosition {
            layer: 0,
            row: 9,
            col: 0,
        };
        let r = client.request::<_, KeyAction>(Cmd::GetKeyAction, 0x03, &pos).await;
        assert_eq!(r, Err(RynkError::Invalid));
    });
}

#[test]
fn set_key_action_rejects_out_of_range() {
    let service = service();
    link_session(&service, async |client| {
        // service()'s keymap is 1×2×2, so row 9 is out of range.
        let set = SetKeyRequest {
            position: KeyPosition {
                layer: 0,
                row: 9,
                col: 0,
            },
            action: KeyAction::No,
        };
        let r = client.request::<_, ()>(Cmd::SetKeyAction, 0x33, &set).await;
        assert_eq!(r, Err(RynkError::Invalid));
    });
}

#[test]
fn get_set_default_layer() {
    let service = service_2_layers();
    link_session(&service, async |client| {
        // Fresh keymap defaults to layer 0; moving it to 1 is an observable
        // change — a dropped Set would read back 0 and fail.
        let before = client.request::<(), u8>(Cmd::GetDefaultLayer, 0x10, &()).await;
        assert_eq!(before, Ok(0), "fresh keymap defaults to layer 0");
        let r = client.request::<u8, ()>(Cmd::SetDefaultLayer, 0x11, &1u8).await;
        assert_eq!(r, Ok(()));
        let after = client.request::<(), u8>(Cmd::GetDefaultLayer, 0x12, &()).await;
        assert_eq!(after, Ok(1), "SetDefaultLayer must persist the new default");
        // Layer 2 is out of range for a 2-layer keymap.
        let r = client.request::<u8, ()>(Cmd::SetDefaultLayer, 0x13, &2u8).await;
        assert_eq!(r, Err(RynkError::Invalid));
    });
}

#[test]
fn set_default_layer_rejects_truncated_payload() {
    let service = service_2_layers();
    link_session(&service, async |client| {
        let mut header = [0u8; RYNK_HEADER_SIZE];
        header[0..2].copy_from_slice(&Cmd::SetDefaultLayer.to_le_bytes());
        header[2] = 0x16;
        header[3..5].copy_from_slice(&0u16.to_le_bytes());
        client.send_raw(&header).await;

        let reply = client.recv_response(0x16).await;
        assert_eq!(reply.header.cmd, Cmd::SetDefaultLayer);
        assert_eq!(reply.envelope::<()>(), Err(RynkError::Malformed));

        let layer = client.request::<(), u8>(Cmd::GetDefaultLayer, 0x17, &()).await;
        assert_eq!(
            layer,
            Ok(0),
            "malformed SetDefaultLayer must not set layer 0 from scratch bytes"
        );
    });
}

#[test]
fn encoder_action_out_of_range() {
    // service()'s keymap has 0 encoders, so any encoder id is out of range —
    // this is the only reachable arm for the encoder Cmds with this mock keymap.
    let service = service();
    link_session(&service, async |client| {
        let get = GetEncoderRequest {
            encoder_id: 0,
            layer: 0,
        };
        let r = client
            .request::<_, EncoderAction>(Cmd::GetEncoderAction, 0x14, &get)
            .await;
        assert_eq!(r, Err(RynkError::Invalid));

        let set = SetEncoderRequest {
            encoder_id: 0,
            layer: 0,
            action: EncoderAction::default(),
        };
        let r = client.request::<_, ()>(Cmd::SetEncoderAction, 0x15, &set).await;
        assert_eq!(r, Err(RynkError::Invalid));
    });
}

#[test]
fn capabilities_report_configured_encoder_count() {
    // A keymap with encoders must advertise them (regression: capabilities
    // hardcoded `num_encoders: 0`, hiding fully-wired encoder endpoints from any
    // capability-respecting host). The endpoint is reachable once advertised.
    let service = service_with_encoders();
    link_session(&service, async |client| {
        let caps = client
            .request::<(), DeviceCapabilities>(Cmd::GetCapabilities, 0x07, &())
            .await
            .expect("Ok envelope");
        assert_eq!(
            caps.num_encoders, 2,
            "capabilities must reflect the configured encoder count"
        );

        // In-range encoder is now reachable (would be `Invalid` if num_encoders==0).
        let get = GetEncoderRequest {
            encoder_id: 1,
            layer: 0,
        };
        let action = client
            .request::<_, EncoderAction>(Cmd::GetEncoderAction, 0x08, &get)
            .await;
        assert_eq!(action, Ok(EncoderAction::default()));

        // Out-of-range encoder id (==count) is still rejected.
        let oor = GetEncoderRequest {
            encoder_id: 2,
            layer: 0,
        };
        let r = client
            .request::<_, EncoderAction>(Cmd::GetEncoderAction, 0x09, &oor)
            .await;
        assert_eq!(r, Err(RynkError::Invalid));
    });
}

#[test]
fn get_set_encoder_round_trip() {
    // SetEncoderAction writes both directions in one shot; the readback must
    // reflect the whole `EncoderAction`, not a half-applied pair.
    let service = service_with_encoders();
    link_session(&service, async |client| {
        let action = EncoderAction::new(
            KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A))),
            KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::B))),
        );
        let set = SetEncoderRequest {
            encoder_id: 1,
            layer: 0,
            action,
        };
        let r = client.request::<_, ()>(Cmd::SetEncoderAction, 0x0A, &set).await;
        assert_eq!(r, Ok(()));

        let get = GetEncoderRequest {
            encoder_id: 1,
            layer: 0,
        };
        let read = client
            .request::<_, EncoderAction>(Cmd::GetEncoderAction, 0x0B, &get)
            .await;
        assert_eq!(read, Ok(action));
    });
}

#[cfg(feature = "bulk")]
#[test]
fn keymap_bulk_round_trip_wraps_row_boundary() {
    use rmk_types::constants::BULK_KEYMAP_SIZE;
    use rmk_types::protocol::rynk::{GetKeymapBulkRequest, GetKeymapBulkResponse, SetKeymapBulkRequest};
    let service = service_3x4();
    link_session(&service, async |client| {
        // A 4-key run from (0,2) on 4-col rows covers (0,2) (0,3) (1,0) (1,1) —
        // the row-major order both bulk endpoints must agree on.
        let mut actions: HVec<KeyAction, BULK_KEYMAP_SIZE> = HVec::new();
        for i in 0..4 {
            actions.push(bulk_action(i)).unwrap();
        }
        let set = SetKeymapBulkRequest {
            layer: 0,
            start_row: 0,
            start_col: 2,
            actions: actions.clone(),
        };
        assert_eq!(client.request::<_, ()>(Cmd::SetKeymapBulk, 0x16, &set).await, Ok(()));

        let get = GetKeymapBulkRequest {
            layer: 0,
            start_row: 0,
            start_col: 2,
            count: 4,
        };
        let got = client
            .request::<_, GetKeymapBulkResponse>(Cmd::GetKeymapBulk, 0x17, &get)
            .await
            .expect("Ok envelope");
        assert_eq!(got.actions, actions, "bulk get reads back the bulk set, in order");

        // The single-key endpoint agrees on where the run wrapped: item 2
        // crossed the row boundary onto (1,0).
        let wrapped = KeyPosition {
            layer: 0,
            row: 1,
            col: 0,
        };
        let single = client.request::<_, KeyAction>(Cmd::GetKeyAction, 0x18, &wrapped).await;
        assert_eq!(single, Ok(bulk_action(2)));
    });
}

#[cfg(feature = "bulk")]
#[test]
fn keymap_bulk_round_trip_wraps_layer_boundary() {
    use rmk_types::constants::BULK_KEYMAP_SIZE;
    use rmk_types::protocol::rynk::{GetKeymapBulkRequest, GetKeymapBulkResponse, SetKeymapBulkRequest};
    // 2 layers × 2 rows × 2 cols = 8 keys. Starting on layer 0's last key
    // (offset 3) and running 2 keys steps across the layer-0→layer-1 boundary:
    // the flat keymap is contiguous across layers, so bulk never stops at one.
    let service = service_2_layers();
    link_session(&service, async |client| {
        let mut actions: HVec<KeyAction, BULK_KEYMAP_SIZE> = HVec::new();
        actions.push(bulk_action(0)).unwrap();
        actions.push(bulk_action(1)).unwrap();
        let set = SetKeymapBulkRequest {
            layer: 0,
            start_row: 1,
            start_col: 1,
            actions: actions.clone(),
        };
        assert_eq!(client.request::<_, ()>(Cmd::SetKeymapBulk, 0x30, &set).await, Ok(()));

        let get = GetKeymapBulkRequest {
            layer: 0,
            start_row: 1,
            start_col: 1,
            count: 2,
        };
        let got = client
            .request::<_, GetKeymapBulkResponse>(Cmd::GetKeymapBulk, 0x31, &get)
            .await
            .expect("Ok envelope");
        assert_eq!(
            got.actions, actions,
            "bulk run reads back across the layer boundary, in order"
        );

        // The single-key endpoint agrees on where the run crossed: item 1
        // landed on layer 1's first key (1,0,0), item 0 on layer 0's last (0,1,1).
        let first_of_layer1 = KeyPosition {
            layer: 1,
            row: 0,
            col: 0,
        };
        assert_eq!(
            client
                .request::<_, KeyAction>(Cmd::GetKeyAction, 0x32, &first_of_layer1)
                .await,
            Ok(bulk_action(1))
        );
        let last_of_layer0 = KeyPosition {
            layer: 0,
            row: 1,
            col: 1,
        };
        assert_eq!(
            client
                .request::<_, KeyAction>(Cmd::GetKeyAction, 0x33, &last_of_layer0)
                .await,
            Ok(bulk_action(0))
        );
    });
}

#[cfg(feature = "bulk")]
#[test]
fn keymap_bulk_max_capacity_round_trip() {
    use rmk_types::constants::BULK_KEYMAP_SIZE;
    use rmk_types::protocol::rynk::{GetKeymapBulkRequest, GetKeymapBulkResponse, SetKeymapBulkRequest};
    // The largest legal run is BULK_KEYMAP_SIZE keys; the 48-key keymap holds it
    // and the run spans several layers (12 keys each).
    let service = service_3x4x4();
    link_session(&service, async |client| {
        let mut actions: HVec<KeyAction, BULK_KEYMAP_SIZE> = HVec::new();
        for i in 0..BULK_KEYMAP_SIZE {
            actions.push(bulk_action(i)).unwrap();
        }
        let set = SetKeymapBulkRequest {
            layer: 0,
            start_row: 0,
            start_col: 0,
            actions: actions.clone(),
        };
        assert_eq!(client.request::<_, ()>(Cmd::SetKeymapBulk, 0x19, &set).await, Ok(()));

        let get = GetKeymapBulkRequest {
            layer: 0,
            start_row: 0,
            start_col: 0,
            count: BULK_KEYMAP_SIZE as u8,
        };
        let got = client
            .request::<_, GetKeymapBulkResponse>(Cmd::GetKeymapBulk, 0x1A, &get)
            .await
            .expect("Ok envelope");
        assert_eq!(got.actions, actions);
    });
}

#[cfg(feature = "bulk")]
#[test]
fn keymap_bulk_rejects_invalid_runs() {
    use rmk_types::constants::BULK_KEYMAP_SIZE;
    use rmk_types::protocol::rynk::{GetKeymapBulkRequest, GetKeymapBulkResponse, SetKeymapBulkRequest};
    let service = service_3x4();
    link_session(&service, async |client| {
        // A zero-item run is a host bug in either direction.
        let get = GetKeymapBulkRequest {
            layer: 0,
            start_row: 0,
            start_col: 0,
            count: 0,
        };
        let r = client
            .request::<_, GetKeymapBulkResponse>(Cmd::GetKeymapBulk, 0x1B, &get)
            .await;
        assert_eq!(r, Err(RynkError::Invalid));
        let set = SetKeymapBulkRequest {
            layer: 0,
            start_row: 0,
            start_col: 0,
            actions: HVec::new(),
        };
        assert_eq!(
            client.request::<_, ()>(Cmd::SetKeymapBulk, 0x1C, &set).await,
            Err(RynkError::Invalid)
        );

        // A run past the keymap's last key: start (2,2) is offset 10 of 12, so
        // 3 keys would run off the end of the (single-layer) keymap — rejected.
        let get = GetKeymapBulkRequest {
            layer: 0,
            start_row: 2,
            start_col: 2,
            count: 3,
        };
        let r = client
            .request::<_, GetKeymapBulkResponse>(Cmd::GetKeymapBulk, 0x1D, &get)
            .await;
        assert_eq!(r, Err(RynkError::Invalid));

        // The Set twin is validated before any write: the whole run is
        // rejected and the in-range prefix stays untouched.
        let mut actions: HVec<KeyAction, BULK_KEYMAP_SIZE> = HVec::new();
        for i in 0..3 {
            actions.push(bulk_action(i)).unwrap();
        }
        let set = SetKeymapBulkRequest {
            layer: 0,
            start_row: 2,
            start_col: 2,
            actions,
        };
        assert_eq!(
            client.request::<_, ()>(Cmd::SetKeymapBulk, 0x1E, &set).await,
            Err(RynkError::Invalid)
        );
        let start = KeyPosition {
            layer: 0,
            row: 2,
            col: 2,
        };
        let untouched = client.request::<_, KeyAction>(Cmd::GetKeyAction, 0x1F, &start).await;
        assert_eq!(untouched, Ok(KeyAction::No), "rejected set must not write its prefix");

        // A start position outside the keymap geometry (layer 1 of 1).
        let get = GetKeymapBulkRequest {
            layer: 1,
            start_row: 0,
            start_col: 0,
            count: 1,
        };
        let r = client
            .request::<_, GetKeymapBulkResponse>(Cmd::GetKeymapBulk, 0x22, &get)
            .await;
        assert_eq!(r, Err(RynkError::Invalid));
    });
}

#[cfg(feature = "bulk")]
#[test]
fn keymap_bulk_rejects_over_budget_run() {
    use rmk_types::constants::BULK_KEYMAP_SIZE;
    use rmk_types::protocol::rynk::{GetKeymapBulkRequest, GetKeymapBulkResponse};
    // The 48-key keymap has room for BULK_KEYMAP_SIZE + 1 keys, so a run that
    // long is rejected by the per-message budget, not by the keymap's span.
    assert!(BULK_KEYMAP_SIZE < 48, "fixture must hold more than one full run");
    let service = service_3x4x4();
    link_session(&service, async |client| {
        let get = GetKeymapBulkRequest {
            layer: 0,
            start_row: 0,
            start_col: 0,
            count: (BULK_KEYMAP_SIZE + 1) as u8,
        };
        let r = client
            .request::<_, GetKeymapBulkResponse>(Cmd::GetKeymapBulk, 0x21, &get)
            .await;
        assert_eq!(r, Err(RynkError::Invalid));
    });
}

// ─────────────────────────────────────────────────────────────────────────
// Macro  (0x02xx)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn get_set_macro_round_trip() {
    let service = service();
    link_session(&service, async |client| {
        let mut data: HVec<u8, 64> = HVec::new();
        data.extend_from_slice(&[0xAA, 0xBB, 0xCC]).unwrap();
        let set = SetMacroRequest {
            index: 0,
            offset: 0,
            data: MacroData { data },
        };
        let r = client.request::<_, ()>(Cmd::SetMacro, 0x20, &set).await;
        assert_eq!(r, Ok(()));

        let get = GetMacroRequest { index: 0, offset: 0 };
        let got = client
            .request::<_, MacroData>(Cmd::GetMacro, 0x21, &get)
            .await
            .expect("Ok envelope");
        // The read is zero-filled up to MACRO_DATA_SIZE; the prefix is our write.
        assert_eq!(&got.data[..3], &[0xAA, 0xBB, 0xCC]);
        assert!(
            got.data[3..].iter().all(|&b| b == 0),
            "rest of the chunk is zero-filled"
        );
    });
}

// ─────────────────────────────────────────────────────────────────────────
// Combo  (0x03xx)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn get_set_combo_round_trip() {
    let service = service();
    link_session(&service, async |client| {
        // An untouched slot reads back as the empty combo.
        let empty = client
            .request::<u8, ComboConfig>(Cmd::GetCombo, 0x30, &0u8)
            .await
            .expect("Ok envelope");
        assert_eq!(empty, ComboConfig::empty());

        // Write a non-empty combo (empty configs are stored as "no combo").
        let combo = ComboConfig::new(
            [KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A)))],
            KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::B))),
            None,
        );
        let set = SetComboRequest {
            index: 0,
            config: combo.clone(),
        };
        let r = client.request::<_, ()>(Cmd::SetCombo, 0x31, &set).await;
        assert_eq!(r, Ok(()));

        let got = client.request::<u8, ComboConfig>(Cmd::GetCombo, 0x32, &0u8).await;
        assert_eq!(got, Ok(combo));
    });
}

#[test]
fn combo_rejects_out_of_range() {
    let service = service();
    link_session(&service, async |client| {
        let r = client.request::<u8, ComboConfig>(Cmd::GetCombo, 0x35, &250u8).await;
        assert_eq!(r, Err(RynkError::Invalid));
        let set = SetComboRequest {
            index: 250,
            config: ComboConfig::empty(),
        };
        let r = client.request::<_, ()>(Cmd::SetCombo, 0x36, &set).await;
        assert_eq!(r, Err(RynkError::Invalid));
    });
}

#[cfg(feature = "bulk")]
#[test]
fn combo_bulk_round_trip_with_empty_slots() {
    use rmk_types::constants::BULK_SIZE;
    use rmk_types::protocol::rynk::{GetComboBulkRequest, GetComboBulkResponse, SetComboBulkRequest};
    let service = service();
    link_session(&service, async |client| {
        // Two distinct combos written at slots 3 and 4.
        let combo = |out| {
            ComboConfig::new(
                [KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A)))],
                KeyAction::Single(Action::Key(KeyCode::Hid(out))),
                None,
            )
        };
        let mut configs: HVec<ComboConfig, BULK_SIZE> = HVec::new();
        configs.push(combo(HidKeyCode::B)).unwrap();
        configs.push(combo(HidKeyCode::C)).unwrap();
        let set = SetComboBulkRequest {
            start_index: 3,
            configs: configs.clone(),
        };
        assert_eq!(client.request::<_, ()>(Cmd::SetComboBulk, 0x37, &set).await, Ok(()));

        // A window starting one slot before the write: the untouched slot
        // reads back as the empty config, matching the single Get's mapping.
        let get = GetComboBulkRequest {
            start_index: 2,
            count: 3,
        };
        let got = client
            .request::<_, GetComboBulkResponse>(Cmd::GetComboBulk, 0x38, &get)
            .await
            .expect("Ok envelope");
        assert_eq!(got.configs.len(), 3);
        assert_eq!(got.configs[0], ComboConfig::empty());
        assert_eq!(got.configs[1], configs[0]);
        assert_eq!(got.configs[2], configs[1]);

        // The single-item endpoint sees the bulk write at slot 4.
        let single = client.request::<u8, ComboConfig>(Cmd::GetCombo, 0x39, &4u8).await;
        assert_eq!(single, Ok(configs[1].clone()));
    });
}

#[cfg(feature = "bulk")]
#[test]
fn combo_bulk_rejects_invalid_runs() {
    use rmk_types::constants::BULK_SIZE;
    use rmk_types::protocol::rynk::{GetComboBulkRequest, GetComboBulkResponse, SetComboBulkRequest};
    let service = service();
    link_session(&service, async |client| {
        // A zero-item run is a host bug in either direction.
        let get = GetComboBulkRequest {
            start_index: 0,
            count: 0,
        };
        let r = client
            .request::<_, GetComboBulkResponse>(Cmd::GetComboBulk, 0x3A, &get)
            .await;
        assert_eq!(r, Err(RynkError::Invalid));
        let set = SetComboBulkRequest {
            start_index: 0,
            configs: HVec::new(),
        };
        assert_eq!(
            client.request::<_, ()>(Cmd::SetComboBulk, 0x3B, &set).await,
            Err(RynkError::Invalid)
        );

        // A run past the last slot (8 combos): 7 + 2 > 8, on both endpoints.
        let get = GetComboBulkRequest {
            start_index: 7,
            count: 2,
        };
        let r = client
            .request::<_, GetComboBulkResponse>(Cmd::GetComboBulk, 0x3C, &get)
            .await;
        assert_eq!(r, Err(RynkError::Invalid));
        let mut configs: HVec<ComboConfig, BULK_SIZE> = HVec::new();
        let combo = ComboConfig::new(
            [KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A)))],
            KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::B))),
            None,
        );
        configs.push(combo.clone()).unwrap();
        configs.push(combo).unwrap();
        let set = SetComboBulkRequest {
            start_index: 7,
            configs,
        };
        assert_eq!(
            client.request::<_, ()>(Cmd::SetComboBulk, 0x3D, &set).await,
            Err(RynkError::Invalid)
        );
        // Validated before any write: the in-range slot 7 stays untouched.
        let single = client.request::<u8, ComboConfig>(Cmd::GetCombo, 0x3E, &7u8).await;
        assert_eq!(
            single,
            Ok(ComboConfig::empty()),
            "rejected set must not write its prefix"
        );
    });
}

// ─────────────────────────────────────────────────────────────────────────
// Morse  (0x04xx)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn get_set_morse_round_trip() {
    let service = service();
    // `Morse` has no trivial constructor, so read slot 0 and flip its profile to
    // a clearly distinct value before writing it back — a no-op Set would still
    // return the default and fail the readback. (`Morse::eq` compares `profile`.)
    link_session(&service, async |client| {
        let mut morse = client
            .request::<u8, Morse>(Cmd::GetMorse, 0x40, &0u8)
            .await
            .expect("slot 0 exists (morses filled to capacity)");
        let original = morse.clone();
        morse.profile = MorseProfile::new(Some(true), Some(MorseMode::PermissiveHold), Some(321), Some(123));
        assert_ne!(morse, original, "the constructed morse must differ from the default");

        let set = SetMorseRequest {
            index: 0,
            config: morse.clone(),
        };
        let r = client.request::<_, ()>(Cmd::SetMorse, 0x41, &set).await;
        assert_eq!(r, Ok(()));
        let got = client.request::<u8, Morse>(Cmd::GetMorse, 0x42, &0u8).await;
        assert_eq!(got, Ok(morse), "SetMorse must persist the distinct value");
    });
}

#[test]
fn morse_rejects_out_of_range() {
    let service = service();
    link_session(&service, async |client| {
        let r = client.request::<u8, Morse>(Cmd::GetMorse, 0x45, &250u8).await;
        assert_eq!(r, Err(RynkError::Invalid));
        // SetMorse needs a payload; reuse slot 0's value but target an OOR index.
        let morse = client
            .request::<u8, Morse>(Cmd::GetMorse, 0x46, &0u8)
            .await
            .expect("slot 0");
        let set = SetMorseRequest {
            index: 250,
            config: morse,
        };
        let r = client.request::<_, ()>(Cmd::SetMorse, 0x47, &set).await;
        assert_eq!(r, Err(RynkError::Invalid));
    });
}

#[cfg(feature = "bulk")]
#[test]
fn morse_bulk_round_trip() {
    use rmk_types::protocol::rynk::{GetMorseBulkRequest, GetMorseBulkResponse, SetMorseBulkRequest};
    let service = service();
    // `Morse` has no trivial constructor (see `get_set_morse_round_trip`), so
    // bulk-read two slots, give each a distinct profile, and bulk-write them back.
    link_session(&service, async |client| {
        let get = GetMorseBulkRequest {
            start_index: 1,
            count: 2,
        };
        let mut fetched = client
            .request::<_, GetMorseBulkResponse>(Cmd::GetMorseBulk, 0x48, &get)
            .await
            .expect("slots 1..3 exist (morses filled to capacity)");
        fetched.configs[0].profile =
            MorseProfile::new(Some(true), Some(MorseMode::PermissiveHold), Some(111), Some(11));
        fetched.configs[1].profile =
            MorseProfile::new(Some(false), Some(MorseMode::HoldOnOtherPress), Some(222), Some(22));

        let set = SetMorseBulkRequest {
            start_index: 1,
            configs: fetched.configs.clone(),
        };
        assert_eq!(client.request::<_, ()>(Cmd::SetMorseBulk, 0x49, &set).await, Ok(()));

        let read = client
            .request::<_, GetMorseBulkResponse>(Cmd::GetMorseBulk, 0x4A, &get)
            .await
            .expect("Ok envelope");
        assert_eq!(
            read.configs, fetched.configs,
            "bulk get reads back the bulk set, in order"
        );

        // The single-item endpoint sees the second bulk write at slot 2.
        let single = client.request::<u8, Morse>(Cmd::GetMorse, 0x4B, &2u8).await;
        assert_eq!(single, Ok(fetched.configs[1].clone()));
    });
}

#[cfg(feature = "bulk")]
#[test]
fn morse_bulk_rejects_invalid_runs() {
    use rmk_types::protocol::rynk::{GetMorseBulkRequest, GetMorseBulkResponse, SetMorseBulkRequest};
    let service = service();
    link_session(&service, async |client| {
        // A zero-item run is a host bug in either direction.
        let get = GetMorseBulkRequest {
            start_index: 0,
            count: 0,
        };
        let r = client
            .request::<_, GetMorseBulkResponse>(Cmd::GetMorseBulk, 0x4C, &get)
            .await;
        assert_eq!(r, Err(RynkError::Invalid));
        let set = SetMorseBulkRequest {
            start_index: 0,
            configs: HVec::new(),
        };
        assert_eq!(
            client.request::<_, ()>(Cmd::SetMorseBulk, 0x4D, &set).await,
            Err(RynkError::Invalid)
        );

        // A run past the last slot (8 morses): 7 + 2 > 8, on both endpoints.
        let get = GetMorseBulkRequest {
            start_index: 7,
            count: 2,
        };
        let r = client
            .request::<_, GetMorseBulkResponse>(Cmd::GetMorseBulk, 0x4E, &get)
            .await;
        assert_eq!(r, Err(RynkError::Invalid));
        let fetch = GetMorseBulkRequest {
            start_index: 0,
            count: 2,
        };
        let fetched = client
            .request::<_, GetMorseBulkResponse>(Cmd::GetMorseBulk, 0x4F, &fetch)
            .await
            .expect("Ok envelope");
        let set = SetMorseBulkRequest {
            start_index: 7,
            configs: fetched.configs,
        };
        assert_eq!(
            client.request::<_, ()>(Cmd::SetMorseBulk, 0x51, &set).await,
            Err(RynkError::Invalid)
        );
    });
}

// ─────────────────────────────────────────────────────────────────────────
// Fork  (0x05xx)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn get_set_fork_round_trip() {
    let service = service();
    // Forks are filled to capacity by KeyMap::new, so slot 0 exists. Read the
    // default, flip a field so the value is provably distinct, then write it —
    // a no-op Set would still return the default and fail the readback.
    link_session(&service, async |client| {
        let mut fork = client
            .request::<u8, Fork>(Cmd::GetFork, 0x50, &0u8)
            .await
            .expect("slot 0 exists (forks filled to capacity)");
        let original = fork;
        fork.trigger = KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A)));
        assert_ne!(fork, original, "the constructed fork must differ from the default");

        let set = SetForkRequest { index: 0, config: fork };
        let r = client.request::<_, ()>(Cmd::SetFork, 0x51, &set).await;
        assert_eq!(r, Ok(()));
        let got = client.request::<u8, Fork>(Cmd::GetFork, 0x52, &0u8).await;
        assert_eq!(got, Ok(fork), "SetFork must persist the distinct value");
    });
}

#[test]
fn fork_rejects_out_of_range() {
    let service = service();
    link_session(&service, async |client| {
        let r = client.request::<u8, Fork>(Cmd::GetFork, 0x55, &250u8).await;
        assert_eq!(r, Err(RynkError::Invalid));
        let fork = client
            .request::<u8, Fork>(Cmd::GetFork, 0x56, &0u8)
            .await
            .expect("slot 0");
        let set = SetForkRequest {
            index: 250,
            config: fork,
        };
        let r = client.request::<_, ()>(Cmd::SetFork, 0x57, &set).await;
        assert_eq!(r, Err(RynkError::Invalid));
    });
}

// ─────────────────────────────────────────────────────────────────────────
// Behavior config  (0x06xx)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn get_set_behavior_config_round_trip() {
    let service = service();
    // All four values are non-default (defaults are 50ms / 1000ms / 20 / 20),
    // so a dropped Set is observable. The harness drains FLASH_CHANNEL, so the
    // four persistence writes never block regardless of channel capacity.
    link_session(&service, async |client| {
        let cfg = WireBehaviorConfig {
            combo_timeout_ms: 123,
            oneshot_timeout_ms: 456,
            tap_interval_ms: 78,
            tap_capslock_interval_ms: 90,
        };
        let r = client.request::<_, ()>(Cmd::SetBehaviorConfig, 0x60, &cfg).await;
        assert_eq!(r, Ok(()));

        let got = client
            .request::<(), WireBehaviorConfig>(Cmd::GetBehaviorConfig, 0x61, &())
            .await;
        assert_eq!(got, Ok(cfg));
    });
}

// ─────────────────────────────────────────────────────────────────────────
// Connection  (0x07xx)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn get_connection_type() {
    let service = service();
    link_session(&service, async |client| {
        let t = client
            .request::<(), ConnectionType>(Cmd::GetConnectionType, 0x70, &())
            .await;
        // Default global connection status prefers USB.
        assert_eq!(t, Ok(ConnectionType::Usb));
    });
}

#[test]
fn get_connection_status_matches_connection_type() {
    let service = service();
    link_session(&service, async |client| {
        let status = client
            .request::<(), ConnectionStatus>(Cmd::GetConnectionStatus, 0x74, &())
            .await
            .expect("Ok envelope decodes into ConnectionStatus");
        // The full snapshot and the derived single-transport view agree.
        let t = client
            .request::<(), ConnectionType>(Cmd::GetConnectionType, 0x75, &())
            .await;
        assert_eq!(t, Ok(status.preferred));
    });
}

#[cfg(feature = "_ble")]
#[test]
fn get_ble_status() {
    let service = service();
    link_session(&service, async |client| {
        let r = client.request::<(), BleStatus>(Cmd::GetBleStatus, 0x71, &()).await;
        let _ = r.expect("Ok envelope decodes into BleStatus");
    });
}

#[cfg(feature = "_ble")]
#[test]
fn switch_ble_profile() {
    let service = service();
    link_session(&service, async |client| {
        // Valid slot 0 enqueues a profile switch (BLE_PROFILE_CHANNEL cap 1).
        let r = client.request::<u8, ()>(Cmd::SwitchBleProfile, 0x72, &0u8).await;
        assert_eq!(r, Ok(()));
    });
}

#[cfg(feature = "_ble")]
#[test]
fn switch_ble_profile_rejects_out_of_range() {
    let service = service();
    link_session(&service, async |client| {
        let r = client.request::<u8, ()>(Cmd::SwitchBleProfile, 0x73, &250u8).await;
        assert_eq!(r, Err(RynkError::Invalid));
    });
}

#[cfg(feature = "_ble")]
#[test]
fn clear_ble_profile() {
    let service = service();
    link_session(&service, async |client| {
        let r = client.request::<u8, ()>(Cmd::ClearBleProfile, 0x74, &0u8).await;
        assert_eq!(r, Ok(()));
    });
}

// ─────────────────────────────────────────────────────────────────────────
// Status  (0x08xx)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn get_current_layer() {
    let service = service();
    link_session(&service, async |client| {
        let layer = client.request::<(), u8>(Cmd::GetCurrentLayer, 0x05, &()).await;
        assert_eq!(layer, Ok(0));
    });
}

#[test]
fn get_matrix_state() {
    let service = service();
    link_session(&service, async |client| {
        let state = client
            .request::<(), MatrixState>(Cmd::GetMatrixState, 0x80, &())
            .await
            .expect("Ok envelope");
        // The insecure harness is unlocked, so the gate passes and the real
        // bitmap is returned — all-zero here since the test presses no keys.
        assert!(state.pressed_bitmap.iter().all(|&b| b == 0));
    });
}

#[cfg(feature = "_ble")]
#[test]
fn get_battery_status() {
    let service = service();
    link_session(&service, async |client| {
        let r = client
            .request::<(), BatteryStatus>(Cmd::GetBatteryStatus, 0x81, &())
            .await;
        let _ = r.expect("Ok envelope decodes into BatteryStatus");
    });
}

#[cfg(all(feature = "_ble", feature = "split"))]
#[test]
fn get_peripheral_status() {
    let service = service();
    link_session(&service, async |client| {
        use rmk_types::protocol::rynk::PeripheralStatus;
        // A valid slot reads back the default snapshot: disconnected, no battery.
        let status = client
            .request::<u8, PeripheralStatus>(Cmd::GetPeripheralStatus, 0x82, &0u8)
            .await
            .expect("Ok envelope");
        assert!(!status.connected);
        // An out-of-range peripheral id is rejected.
        let r = client
            .request::<u8, PeripheralStatus>(Cmd::GetPeripheralStatus, 0x83, &250u8)
            .await;
        assert_eq!(r, Err(RynkError::Invalid));
    });
}

#[test]
fn get_wpm_returns_snapshot() {
    let service = service();
    link_session(&service, async |client| {
        let wpm = client.request::<(), u16>(Cmd::GetWpm, 0x09, &()).await;
        assert_eq!(wpm, Ok(0));
    });
}

#[test]
fn get_sleep_state_returns_snapshot() {
    let service = service();
    link_session(&service, async |client| {
        let sleep = client.request::<(), bool>(Cmd::GetSleepState, 0x0A, &()).await;
        assert_eq!(sleep, Ok(false));
    });
}

#[test]
fn get_led_indicator_returns_snapshot() {
    let service = service();
    link_session(&service, async |client| {
        let led = client
            .request::<(), LedIndicator>(Cmd::GetLedIndicator, 0x0B, &())
            .await;
        assert_eq!(led, Ok(LedIndicator::from_bits(0)));
    });
}

// ─────────────────────────────────────────────────────────────────────────
// Protocol / framing
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn drops_topic_cmd_from_host() {
    let service = service();
    link_session(&service, async |client| {
        // Topic CMDs are server→host push only. The session drops such
        // frames without replying — an error reply would echo a high-bit
        // CMD that the host's reassembly queues as a phantom topic.
        client.send(Cmd::LayerChange, 0x13, &0u8).await;
        // The next request succeeds with no stray reply in between
        // (`recv_response` panics on any non-topic frame with another seq).
        let version = client.request::<(), ProtocolVersion>(Cmd::GetVersion, 0x14, &()).await;
        assert_eq!(version, Ok(ProtocolVersion::CURRENT), "session in sync after drop");
    });
}

#[cfg(feature = "_ble")]
#[test]
fn drops_battery_topic_cmd_from_host() {
    // `BatteryStatusChange` is the one `_ble`-gated topic; cover it explicitly.
    let service = service();
    link_session(&service, async |client| {
        client
            .send(Cmd::BatteryStatusChange, 0x19, &BatteryStatus::Unavailable)
            .await;
        let version = client.request::<(), ProtocolVersion>(Cmd::GetVersion, 0x1A, &()).await;
        assert_eq!(version, Ok(ProtocolVersion::CURRENT), "session in sync after drop");
    });
}

#[test]
fn unknown_cmd_over_the_wire_gets_unknown_cmd_reply() {
    let service = service();
    // Cmd tags this build does not handle, one per range. 0x00FF (an
    // unassigned System-range slot, standing in for a feature-gated-out or
    // newer peer's command): dispatch answers UnknownCmd, not Malformed,
    // because the frame itself was sound. 0xFFFF (topic range): dropped without
    // a reply, like every topic-range request. Neither desyncs the session.
    link_session(&service, async |client| {
        let mut header = [0u8; RYNK_HEADER_SIZE];
        header[0..2].copy_from_slice(&0x00FFu16.to_le_bytes());
        header[2] = 0x21; // seq — echoed on the error reply
        // payload_len stays 0
        client.send_raw(&header).await;
        let reply = client.recv_response(0x21).await;
        assert_eq!(
            reply.header.cmd,
            Cmd::from_raw(0x00FF),
            "error reply echoes the unknown cmd bytes"
        );
        assert_eq!(reply.envelope::<()>(), Err(RynkError::UnknownCmd));

        let mut header = [0u8; RYNK_HEADER_SIZE];
        header[0..2].copy_from_slice(&0xFFFFu16.to_le_bytes());
        header[2] = 0x22;
        client.send_raw(&header).await;

        // The session is still in sync afterwards, with no stray reply.
        let version = client.request::<(), ProtocolVersion>(Cmd::GetVersion, 0x23, &()).await;
        assert_eq!(version, Ok(ProtocolVersion::CURRENT));
    });
}

#[test]
fn lock_endpoints_dispatch() {
    // The loopback harness runs `insecure`, so the device is always unlocked and
    // the three lock arms are reachable over the wire. The gate and the
    // physical-unlock ceremony are covered by the `host::rynk` / `host::lock`
    // unit tests, which can drive the matrix and the mock clock.
    let service = service();
    link_session(&service, async |client| {
        // GetLockStatus: insecure ⇒ never locked, no challenge configured.
        let status = client
            .request::<(), LockStatus>(Cmd::GetLockStatus, 0x01, &())
            .await
            .expect("lock status");
        assert!(!status.locked);
        assert!(status.key_positions.is_empty());

        // A wire `Lock` is a no-op on an insecure device (is_unlocked stays true).
        assert_eq!(client.request::<(), ()>(Cmd::Lock, 0x02, &()).await, Ok(()));
        let status = client
            .request::<(), LockStatus>(Cmd::GetLockStatus, 0x03, &())
            .await
            .expect("lock status");
        assert!(!status.locked, "insecure device ignores wire Lock");

        // UnlockPoll with no keys configured warns and refuses — nothing to hold.
        let polled = client
            .request::<(), LockStatus>(Cmd::UnlockPoll, 0x04, &())
            .await
            .expect("unlock poll");
        assert!(!polled.unlocking);
        assert_eq!(polled.remaining_keys, 0);
    });
}

#[test]
fn pipelines_multiple_requests_in_one_session() {
    let service = service();
    // Three distinct commands on one session, each correlated by its own seq.
    link_session(&service, async |client| {
        let version = client.request::<(), ProtocolVersion>(Cmd::GetVersion, 0x31, &()).await;
        assert_eq!(version, Ok(ProtocolVersion::CURRENT));

        let caps = client
            .request::<(), DeviceCapabilities>(Cmd::GetCapabilities, 0x32, &())
            .await
            .expect("Ok envelope");
        assert_eq!(caps.num_layers, 1);

        let layer = client.request::<(), u8>(Cmd::GetCurrentLayer, 0x33, &()).await;
        assert_eq!(layer, Ok(0));
    });
}

#[test]
fn oversized_frame_is_rejected_then_stream_resyncs() {
    let service = service();
    // A header that declares more payload than the device buffer can hold. The
    // session must reply Malformed, drain the declared bytes off the wire, and
    // resync so a well-formed request right after still gets a correct reply.
    let recovered = link_session(&service, async |client| {
        let payload_n = (RYNK_BUFFER_SIZE - RYNK_HEADER_SIZE + 1) as u16;
        let mut bad = [0u8; RYNK_HEADER_SIZE];
        bad[0..2].copy_from_slice(&Cmd::GetVersion.to_le_bytes());
        bad[2] = 0x55; // seq — echoed on the error reply
        bad[3..5].copy_from_slice(&payload_n.to_le_bytes());
        client.send_raw(&bad).await;
        // The declared-but-bogus payload the session drains to resync.
        client.send_raw(&vec![0xAB; payload_n as usize]).await;

        let err = client.recv_response(0x55).await;
        assert_eq!(
            err.envelope::<()>(),
            Err(RynkError::Malformed),
            "oversized frame → Malformed"
        );

        // A clean request after the resync still round-trips.
        client.request::<(), ProtocolVersion>(Cmd::GetVersion, 0x56, &()).await
    });
    assert_eq!(recovered, Ok(ProtocolVersion::CURRENT));
}

// ─────────────────────────────────────────────────────────────────────────
// Topics  (0x80xx, server → host push)
//
// `run_session` subscribes to every topic before the script runs, so an event
// published from the script is forwarded as a topic frame on the next session
// turn — exercising the topic-emit arm of the session loop and the wire encoder.
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn topic_layer_change() {
    let service = service();
    let v = link_session(&service, async |client| {
        publish_event(LayerChangeEvent::new(3));
        let frame = client.recv_topic().await;
        assert_eq!(frame.header.cmd, Cmd::LayerChange);
        frame.raw::<u8>()
    });
    assert_eq!(v, 3);
}

#[test]
fn topic_wpm_update() {
    let service = service();
    let v = link_session(&service, async |client| {
        publish_event(WpmUpdateEvent::new(42));
        let frame = client.recv_topic().await;
        assert_eq!(frame.header.cmd, Cmd::WpmUpdate);
        frame.raw::<u16>()
    });
    assert_eq!(v, 42);
}

#[test]
fn topic_sleep_state() {
    let service = service();
    let v = link_session(&service, async |client| {
        publish_event(SleepStateEvent::new(true));
        let frame = client.recv_topic().await;
        assert_eq!(frame.header.cmd, Cmd::SleepState);
        frame.raw::<bool>()
    });
    assert!(v);
}

#[test]
fn topic_led_indicator() {
    let service = service();
    let v = link_session(&service, async |client| {
        publish_event(LedIndicatorEvent::new(LedIndicator::from_bits(0b0000_0101)));
        let frame = client.recv_topic().await;
        assert_eq!(frame.header.cmd, Cmd::LedIndicatorChange);
        frame.raw::<LedIndicator>()
    });
    assert_eq!(v, LedIndicator::from_bits(0b0000_0101));
}

#[test]
fn topic_connection_change() {
    let service = service();
    let v = link_session(&service, async |client| {
        // Publish a non-default status (default prefers USB) so the readback
        // proves the published value crossed the wire, not a stale snapshot.
        let status = ConnectionStatus {
            preferred: ConnectionType::Ble,
            ..ConnectionStatus::default()
        };
        publish_event(ConnectionStatusChangeEvent(status));
        let frame = client.recv_topic().await;
        assert_eq!(frame.header.cmd, Cmd::ConnectionChange);
        frame.raw::<ConnectionStatus>()
    });
    assert_eq!(v.preferred, ConnectionType::Ble);
}

#[cfg(feature = "_ble")]
#[test]
fn topic_battery_status() {
    let service = service();
    let expected = BatteryStatus::Available {
        charge_state: ChargeState::Discharging,
        level: Some(77),
    };
    let v = link_session(&service, async |client| {
        publish_event(BatteryStatusEvent(expected));
        let frame = client.recv_topic().await;
        assert_eq!(frame.header.cmd, Cmd::BatteryStatusChange);
        frame.raw::<BatteryStatus>()
    });
    assert_eq!(v, expected);
}
