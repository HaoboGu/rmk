//! End-to-end loopback integration test for the Rynk host service.
//!
//! Cases run the production [`RynkService::run_session`] over an in-memory
//! embedded-io duplex and drive it with a host-side `RynkClient`.
//!
//! Coverage includes each dispatch arm and topic push.

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

use crate::common::rynk_link::{RynkHostClient, link_session, link_two_sessions};
use crate::common::{wrap_keymap, wrap_keymap_with_encoders};

/// Leak an always-unlocked config so these cases exercise protocol mechanics.
fn insecure_config() -> &'static RmkConfig<'static> {
    let mut config = RmkConfig::default();
    config.lock_config.insecure = true;
    Box::leak(Box::new(config))
}

/// Build a tiny 1-layer 2x2 service.
fn service() -> RynkService<'static> {
    let behavior: &'static mut BehaviorConfig = Box::leak(Box::new(BehaviorConfig::default()));
    let per_key: &'static PositionalConfig<2, 2> = Box::leak(Box::new(PositionalConfig::default()));
    let keymap = [[[KeyAction::No; 2]; 2]; 1];
    let km = wrap_keymap(keymap, per_key, behavior);
    RynkService::new(km, insecure_config())
}

/// 2-layer service so SetDefaultLayer can make an observable change.
fn service_2_layers() -> RynkService<'static> {
    let behavior: &'static mut BehaviorConfig = Box::leak(Box::new(BehaviorConfig::default()));
    let per_key: &'static PositionalConfig<2, 2> = Box::leak(Box::new(PositionalConfig::default()));
    let keymap = [[[KeyAction::No; 2]; 2]; 2];
    let km = wrap_keymap(keymap, per_key, behavior);
    RynkService::new(km, insecure_config())
}

/// Service with two encoders, making encoder endpoints reachable.
fn service_with_encoders() -> RynkService<'static> {
    let behavior: &'static mut BehaviorConfig = Box::leak(Box::new(BehaviorConfig::default()));
    let per_key: &'static PositionalConfig<2, 2> = Box::leak(Box::new(PositionalConfig::default()));
    let keymap = [[[KeyAction::No; 2]; 2]; 1];
    let encoder_map = [[EncoderAction::default(); 2]; 1];
    let km = wrap_keymap_with_encoders(keymap, encoder_map, per_key, behavior);
    RynkService::new(km, insecure_config())
}

/// 3x4 service for bulk edge and row-wrap cases.
fn service_3x4() -> RynkService<'static> {
    let behavior: &'static mut BehaviorConfig = Box::leak(Box::new(BehaviorConfig::default()));
    let per_key: &'static PositionalConfig<3, 4> = Box::leak(Box::new(PositionalConfig::default()));
    let keymap = [[[KeyAction::No; 4]; 3]; 1];
    let km = wrap_keymap(keymap, per_key, behavior);
    let config: &'static RmkConfig<'static> = Box::leak(Box::new(RmkConfig::default()));
    RynkService::new(km, config)
}

/// 256-key service for full and over-budget bulk runs.
fn service_4x8x8() -> RynkService<'static> {
    let behavior: &'static mut BehaviorConfig = Box::leak(Box::new(BehaviorConfig::default()));
    let per_key: &'static PositionalConfig<8, 4> = Box::leak(Box::new(PositionalConfig::default()));
    let keymap = [[[KeyAction::No; 4]; 8]; 8];
    let km = wrap_keymap(keymap, per_key, behavior);
    let config: &'static RmkConfig<'static> = Box::leak(Box::new(RmkConfig::default()));
    RynkService::new(km, config)
}

/// Distinct action per bulk slot catches wrong-position writes.
fn bulk_action(i: usize) -> KeyAction {
    KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::from(4 + i as u8))))
}

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
        // Pin real protocol limits, not just decodability.
        assert_eq!(caps.max_combos, 8);
        assert_eq!(caps.max_morse, 8);
        assert_eq!(caps.max_forks, 8);
        assert_eq!(caps.macro_space_size, 256);
        assert_eq!(caps.macro_chunk_size, 64);
        // This suite runs with all-on and rynk-only feature sets.
        assert_eq!(caps.storage_enabled, cfg!(feature = "storage"));
        assert_eq!(caps.ble_enabled, cfg!(feature = "_ble"));
        assert_eq!(caps.is_split, cfg!(feature = "split"));
        assert!(caps.bulk_transfer_supported);
        assert_eq!(caps.max_bulk_keys as usize, rmk_types::constants::BULK_KEYMAP_SIZE);
        assert_eq!(caps.max_bulk_configs as usize, rmk_types::constants::BULK_SIZE);
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
        // service() uses the default identity and this crate version.
        assert_eq!(info.vendor_id, 0x4c4b);
        assert_eq!(info.product_id, 0x4643);
        assert_eq!(info.manufacturer.as_str(), "RMK");
        assert_eq!(info.product_name.as_str(), "RMK Keyboard");
        assert_eq!(info.serial_number.as_str(), rmk::config::RMK_BUILD_INFO);
        assert_eq!(info.rmk_version.major, env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap());
        assert_eq!(info.rmk_version.minor, env!("CARGO_PKG_VERSION_MINOR").parse().unwrap());
        assert_eq!(info.rmk_version.patch, env!("CARGO_PKG_VERSION_PATCH").parse().unwrap());
    });
}

#[test]
fn reboot_acks_where_reset_is_a_no_op() {
    // Test targets may no-op reset and return the standard `Ok(())` envelope.
    let service = service();
    link_session(&service, async |client| {
        let r = client.request::<(), ()>(Cmd::Reboot, 0x60, &()).await;
        assert_eq!(r, Ok(()));
    });
}

#[test]
fn bootloader_jump_acks_where_jump_is_a_no_op() {
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
    // Reject LayoutOnly until reset_storage can preserve bonds.
    let service = service();
    link_session(&service, async |client| {
        let r = client
            .request::<StorageResetMode, ()>(Cmd::StorageReset, 0x63, &StorageResetMode::LayoutOnly)
            .await;
        assert_eq!(r, Err(RynkError::Unimplemented));
    });
}

#[test]
fn get_set_key_action_round_trip() {
    let service = service();
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

/// Concurrent sessions over one service must share state without panicking.
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

        // Last writer over the shared KeyMap wins.
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

        // Both sessions read the shared final state.
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
        // Row 9 is out of range for service().
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
        // Moving default layer to 1 makes dropped Set observable.
        let before = client.request::<(), u8>(Cmd::GetDefaultLayer, 0x10, &()).await;
        assert_eq!(before, Ok(0), "fresh keymap defaults to layer 0");
        let r = client.request::<u8, ()>(Cmd::SetDefaultLayer, 0x11, &1u8).await;
        assert_eq!(r, Ok(()));
        let after = client.request::<(), u8>(Cmd::GetDefaultLayer, 0x12, &()).await;
        assert_eq!(after, Ok(1), "SetDefaultLayer must persist the new default");
        // Layer 2 is out of range.
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
    // service() has no encoders, so only the OOR path is reachable.
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
    // Capabilities must advertise encoder endpoints.
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

        // In-range encoder is reachable once advertised.
        let get = GetEncoderRequest {
            encoder_id: 1,
            layer: 0,
        };
        let action = client
            .request::<_, EncoderAction>(Cmd::GetEncoderAction, 0x08, &get)
            .await;
        assert_eq!(action, Ok(EncoderAction::default()));

        // id == count is still out of range.
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
    // Readback must reflect both encoder directions.
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

#[test]
fn keymap_bulk_round_trip_wraps_row_boundary() {
    use rmk_types::constants::BULK_KEYMAP_SIZE;
    use rmk_types::protocol::rynk::{GetKeymapBulkRequest, GetKeymapBulkResponse, SetKeymapBulkRequest};
    let service = service_3x4();
    link_session(&service, async |client| {
        // Row-major run wraps from (0,3) to (1,0).
        let mut actions: HVec<KeyAction, BULK_KEYMAP_SIZE> = HVec::new();
        for i in 0..4 {
            actions.push(bulk_action(i)).unwrap();
        }
        // Flat index 2 is key (0,0,2).
        let set = SetKeymapBulkRequest {
            layer: 0,
            start_row: 0,
            start_col: 2,
            actions: actions.clone(),
        };
        assert_eq!(client.request::<_, ()>(Cmd::SetKeymapBulk, 0x16, &set).await, Ok(()));

        // GET clamps to the written 4-item page.
        let got = client
            .request::<_, GetKeymapBulkResponse>(
                Cmd::GetKeymapBulk,
                0x17,
                &GetKeymapBulkRequest {
                    layer: 0,
                    start_row: 0,
                    start_col: 2,
                },
            )
            .await
            .expect("Ok envelope");
        assert_eq!(
            &got.actions[..4],
            actions.as_slice(),
            "bulk get reads back the bulk set, in order"
        );

        // Single-key read confirms row wrap.
        let wrapped = KeyPosition {
            layer: 0,
            row: 1,
            col: 0,
        };
        let single = client.request::<_, KeyAction>(Cmd::GetKeyAction, 0x18, &wrapped).await;
        assert_eq!(single, Ok(bulk_action(2)));
    });
}

#[test]
fn keymap_bulk_round_trip_wraps_layer_boundary() {
    use rmk_types::constants::BULK_KEYMAP_SIZE;
    use rmk_types::protocol::rynk::{GetKeymapBulkRequest, GetKeymapBulkResponse, SetKeymapBulkRequest};
    // A 2-key run from flat index 3 crosses the layer boundary.
    let service = service_2_layers();
    link_session(&service, async |client| {
        let mut actions: HVec<KeyAction, BULK_KEYMAP_SIZE> = HVec::new();
        actions.push(bulk_action(0)).unwrap();
        actions.push(bulk_action(1)).unwrap();
        // Flat index 3 is layer 0's last key.
        let set = SetKeymapBulkRequest {
            layer: 0,
            start_row: 1,
            start_col: 1,
            actions: actions.clone(),
        };
        assert_eq!(client.request::<_, ()>(Cmd::SetKeymapBulk, 0x30, &set).await, Ok(()));

        let got = client
            .request::<_, GetKeymapBulkResponse>(
                Cmd::GetKeymapBulk,
                0x31,
                &GetKeymapBulkRequest {
                    layer: 0,
                    start_row: 1,
                    start_col: 1,
                },
            )
            .await
            .expect("Ok envelope");
        assert_eq!(
            &got.actions[..2],
            actions.as_slice(),
            "bulk run reads back across the layer boundary, in order"
        );

        // Single-key reads confirm the layer-boundary wrap.
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

#[test]
fn keymap_bulk_max_capacity_round_trip() {
    use rmk_types::constants::BULK_KEYMAP_SIZE;
    use rmk_types::protocol::rynk::{GetKeymapBulkRequest, GetKeymapBulkResponse, SetKeymapBulkRequest};
    // The 48-key keymap holds a full multi-layer bulk run.
    let service = service_4x8x8();
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

        // GET at 0 returns one full capped page.
        let got = client
            .request::<_, GetKeymapBulkResponse>(
                Cmd::GetKeymapBulk,
                0x1A,
                &GetKeymapBulkRequest {
                    layer: 0,
                    start_row: 0,
                    start_col: 0,
                },
            )
            .await
            .expect("Ok envelope");
        assert_eq!(got.actions, actions);
    });
}

#[test]
fn keymap_bulk_clamps_and_rejects() {
    use rmk_types::constants::BULK_KEYMAP_SIZE;
    use rmk_types::protocol::rynk::{GetKeymapBulkRequest, GetKeymapBulkResponse, SetKeymapBulkRequest};
    let service = service_3x4();
    link_session(&service, async |client| {
        // Valid tail starts clamp to a short page.
        let got = client
            .request::<_, GetKeymapBulkResponse>(
                Cmd::GetKeymapBulk,
                0x1B,
                &GetKeymapBulkRequest {
                    layer: 0,
                    start_row: 2,
                    start_col: 2,
                },
            )
            .await
            .expect("Ok envelope");
        assert_eq!(got.actions.len(), 2, "short page is the keymap tail");

        // Out-of-geometry starts reject instead of returning an empty page.
        let r = client
            .request::<_, GetKeymapBulkResponse>(
                Cmd::GetKeymapBulk,
                0x1C,
                &GetKeymapBulkRequest {
                    layer: 1,
                    start_row: 0,
                    start_col: 0,
                },
            )
            .await;
        assert_eq!(r, Err(RynkError::Invalid), "out-of-geometry start is rejected");

        // Empty SET runs are host bugs.
        let set = SetKeymapBulkRequest {
            layer: 0,
            start_row: 0,
            start_col: 0,
            actions: HVec::new(),
        };
        assert_eq!(
            client.request::<_, ()>(Cmd::SetKeymapBulk, 0x1D, &set).await,
            Err(RynkError::Invalid)
        );

        // Over-tail SET rejects without touching the in-range prefix.
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
    });
}

#[test]
fn keymap_bulk_get_caps_page_at_budget() {
    use rmk_types::constants::BULK_KEYMAP_SIZE;
    use rmk_types::protocol::rynk::{GetKeymapBulkRequest, GetKeymapBulkResponse};
    // GET from 0 is capped by message budget, not keymap span.
    assert!(BULK_KEYMAP_SIZE < 256, "fixture must hold more than one full run");
    let service = service_4x8x8();
    link_session(&service, async |client| {
        let got = client
            .request::<_, GetKeymapBulkResponse>(
                Cmd::GetKeymapBulk,
                0x21,
                &GetKeymapBulkRequest {
                    layer: 0,
                    start_row: 0,
                    start_col: 0,
                },
            )
            .await
            .expect("Ok envelope");
        assert_eq!(
            got.actions.len(),
            BULK_KEYMAP_SIZE,
            "page capped at the per-message budget even though 48 keys remain"
        );
    });
}

#[test]
fn keymap_bulk_set_malformed_element_aborts_whole_write() {
    let service = service_4x8x8();
    link_session(&service, async |client| {
        let good = KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A)));
        let mut scratch = [0u8; 16];
        let good_bytes = postcard::to_slice(&good, &mut scratch).unwrap();

        // 0x7F is not a valid `KeyAction` discriminant.
        let mut payload = vec![0u8, 0, 0, 2];
        payload.extend_from_slice(good_bytes);
        payload.push(0x7F);

        let mut frame = vec![0u8; RYNK_HEADER_SIZE];
        frame[0..2].copy_from_slice(&Cmd::SetKeymapBulk.to_le_bytes());
        frame[2] = 0x40;
        frame[3..5].copy_from_slice(&(payload.len() as u16).to_le_bytes());
        frame.extend_from_slice(&payload);
        client.send_raw(&frame).await;

        let reply = client.recv_response(0x40).await;
        assert_eq!(
            reply.envelope::<()>(),
            Err(RynkError::Malformed),
            "a malformed element rejects the whole write"
        );

        let first = client
            .request::<_, KeyAction>(
                Cmd::GetKeyAction,
                0x41,
                &KeyPosition {
                    layer: 0,
                    row: 0,
                    col: 0,
                },
            )
            .await;
        assert_eq!(
            first,
            Ok(KeyAction::No),
            "all-or-nothing: no element is written when a later one is malformed"
        );
    });
}

#[test]
fn get_set_macro_round_trip() {
    let service = service();
    link_session(&service, async |client| {
        let mut data: HVec<u8, 64> = HVec::new();
        data.extend_from_slice(&[0xAA, 0xBB, 0xCC]).unwrap();
        let set = SetMacroRequest {
            offset: 0,
            data: MacroData { data },
        };
        let r = client.request::<_, ()>(Cmd::SetMacro, 0x20, &set).await;
        assert_eq!(r, Ok(()));

        let get = GetMacroRequest { offset: 0 };
        let got = client
            .request::<_, MacroData>(Cmd::GetMacro, 0x21, &get)
            .await
            .expect("Ok envelope");
        // Reads zero-fill after the written prefix.
        assert_eq!(&got.data[..3], &[0xAA, 0xBB, 0xCC]);
        assert!(
            got.data[3..].iter().all(|&b| b == 0),
            "rest of the chunk is zero-filled"
        );
    });
}

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

        // Empty configs are stored as "no combo".
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

#[test]
fn combo_bulk_round_trip_with_empty_slots() {
    use rmk_types::constants::BULK_SIZE;
    use rmk_types::protocol::rynk::{GetComboBulkRequest, GetComboBulkResponse, SetComboBulkRequest};
    let service = service();
    link_session(&service, async |client| {
        // Slots 3 and 4 are distinct writes.
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

        // Page includes one untouched slot and one written slot.
        let got = client
            .request::<_, GetComboBulkResponse>(Cmd::GetComboBulk, 0x38, &GetComboBulkRequest { start_index: 2 })
            .await
            .expect("Ok envelope");
        assert!(got.configs.len() >= 3);
        assert_eq!(got.configs[0], ComboConfig::empty());
        assert_eq!(got.configs[1], configs[0]);
        assert_eq!(got.configs[2], configs[1]);

        // Single-item read sees the bulk write.
        let single = client.request::<u8, ComboConfig>(Cmd::GetCombo, 0x39, &4u8).await;
        assert_eq!(single, Ok(configs[1].clone()));
    });
}

#[test]
fn combo_bulk_clamps_and_rejects() {
    use rmk_types::constants::BULK_SIZE;
    use rmk_types::protocol::rynk::{GetComboBulkRequest, GetComboBulkResponse, SetComboBulkRequest};
    let service = service();
    link_session(&service, async |client| {
        // Empty SET runs are host bugs.
        let set = SetComboBulkRequest {
            start_index: 0,
            configs: HVec::new(),
        };
        assert_eq!(
            client.request::<_, ()>(Cmd::SetComboBulk, 0x3A, &set).await,
            Err(RynkError::Invalid)
        );

        // GET clamps to short and final-empty pages.
        let got = client
            .request::<_, GetComboBulkResponse>(Cmd::GetComboBulk, 0x3B, &GetComboBulkRequest { start_index: 7 })
            .await
            .expect("Ok envelope");
        assert_eq!(got.configs.len(), 1, "short page is the last slot");
        let got = client
            .request::<_, GetComboBulkResponse>(Cmd::GetComboBulk, 0x3C, &GetComboBulkRequest { start_index: 8 })
            .await
            .expect("Ok envelope");
        assert!(got.configs.is_empty(), "start at the slot count yields an empty page");

        // Over-tail SET rejects without touching slot 7.
        let combo = ComboConfig::new(
            [KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A)))],
            KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::B))),
            None,
        );
        let mut configs: HVec<ComboConfig, BULK_SIZE> = HVec::new();
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
        let single = client.request::<u8, ComboConfig>(Cmd::GetCombo, 0x3E, &7u8).await;
        assert_eq!(
            single,
            Ok(ComboConfig::empty()),
            "rejected set must not write its prefix"
        );
    });
}

#[test]
fn get_set_morse_round_trip() {
    let service = service();
    // Flip an existing Morse profile so the write is distinguishable.
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
        // Reuse slot 0's payload with an OOR target.
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

#[test]
fn morse_bulk_round_trip() {
    use rmk_types::constants::BULK_SIZE;
    use rmk_types::protocol::rynk::{GetMorseBulkRequest, GetMorseBulkResponse, SetMorseBulkRequest};
    let service = service();
    // Mutate a read page because Morse has no trivial constructor.
    link_session(&service, async |client| {
        let page = client
            .request::<_, GetMorseBulkResponse>(Cmd::GetMorseBulk, 0x48, &GetMorseBulkRequest { start_index: 1 })
            .await
            .expect("slots 1.. exist (morses filled to capacity)");
        assert!(page.configs.len() >= 2);
        let mut configs: HVec<Morse, BULK_SIZE> = HVec::new();
        configs.push(page.configs[0].clone()).unwrap();
        configs.push(page.configs[1].clone()).unwrap();
        configs[0].profile = MorseProfile::new(Some(true), Some(MorseMode::PermissiveHold), Some(111), Some(11));
        configs[1].profile = MorseProfile::new(Some(false), Some(MorseMode::HoldOnOtherPress), Some(222), Some(22));

        let set = SetMorseBulkRequest {
            start_index: 1,
            configs: configs.clone(),
        };
        assert_eq!(client.request::<_, ()>(Cmd::SetMorseBulk, 0x49, &set).await, Ok(()));

        let read = client
            .request::<_, GetMorseBulkResponse>(Cmd::GetMorseBulk, 0x4A, &GetMorseBulkRequest { start_index: 1 })
            .await
            .expect("Ok envelope");
        assert_eq!(
            &read.configs[..2],
            configs.as_slice(),
            "bulk get reads back the bulk set, in order"
        );

        // Single-item read sees the second bulk write.
        let single = client.request::<u8, Morse>(Cmd::GetMorse, 0x4B, &2u8).await;
        assert_eq!(single, Ok(configs[1].clone()));
    });
}

#[test]
fn morse_bulk_clamps_and_rejects() {
    use rmk_types::constants::BULK_SIZE;
    use rmk_types::protocol::rynk::{GetMorseBulkRequest, GetMorseBulkResponse, SetMorseBulkRequest};
    let service = service();
    link_session(&service, async |client| {
        // Empty SET runs are host bugs.
        let set = SetMorseBulkRequest {
            start_index: 0,
            configs: HVec::new(),
        };
        assert_eq!(
            client.request::<_, ()>(Cmd::SetMorseBulk, 0x4C, &set).await,
            Err(RynkError::Invalid)
        );

        // GET clamps to short and final-empty pages.
        let got = client
            .request::<_, GetMorseBulkResponse>(Cmd::GetMorseBulk, 0x4D, &GetMorseBulkRequest { start_index: 7 })
            .await
            .expect("Ok envelope");
        assert_eq!(got.configs.len(), 1, "short page is the last slot");
        let got = client
            .request::<_, GetMorseBulkResponse>(Cmd::GetMorseBulk, 0x4E, &GetMorseBulkRequest { start_index: 8 })
            .await
            .expect("Ok envelope");
        assert!(got.configs.is_empty(), "start at the slot count yields an empty page");

        // Over-tail SET rejects without touching slot 7.
        let before = client
            .request::<u8, Morse>(Cmd::GetMorse, 0x4F, &7u8)
            .await
            .expect("slot 7");
        let mut modified = before.clone();
        modified.profile = MorseProfile::new(Some(true), Some(MorseMode::PermissiveHold), Some(321), Some(123));
        let mut configs: HVec<Morse, BULK_SIZE> = HVec::new();
        configs.push(modified.clone()).unwrap();
        configs.push(modified).unwrap();
        let set = SetMorseBulkRequest {
            start_index: 7,
            configs,
        };
        assert_eq!(
            client.request::<_, ()>(Cmd::SetMorseBulk, 0x51, &set).await,
            Err(RynkError::Invalid)
        );
        let after = client.request::<u8, Morse>(Cmd::GetMorse, 0x52, &7u8).await;
        assert_eq!(after, Ok(before), "rejected set must not write slot 7");
    });
}

#[test]
fn get_set_fork_round_trip() {
    let service = service();
    // Flip a default fork field so the write is distinguishable.
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

#[test]
fn get_set_behavior_config_round_trip() {
    let service = service();
    // Non-default values make a dropped Set observable.
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

#[test]
fn get_connection_type() {
    let service = service();
    link_session(&service, async |client| {
        let t = client
            .request::<(), ConnectionType>(Cmd::GetConnectionType, 0x70, &())
            .await;
        // Default global status prefers USB.
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
        // Full snapshot and derived view agree.
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
        // Slot 0 enqueues a profile switch.
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
        // Insecure harness allows the real all-zero matrix bitmap.
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
        // Valid slot reads back the default snapshot.
        let status = client
            .request::<u8, PeripheralStatus>(Cmd::GetPeripheralStatus, 0x82, &0u8)
            .await
            .expect("Ok envelope");
        assert!(!status.connected);
        // OOR peripheral id is rejected.
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

#[test]
fn drops_topic_cmd_from_host() {
    let service = service();
    link_session(&service, async |client| {
        // Topic CMD requests must not create phantom topic replies.
        client.send(Cmd::LayerChange, 0x13, &0u8).await;
        // Next request proves there is no stray reply.
        let version = client.request::<(), ProtocolVersion>(Cmd::GetVersion, 0x14, &()).await;
        assert_eq!(version, Ok(ProtocolVersion::CURRENT), "session in sync after drop");
    });
}

#[cfg(feature = "_ble")]
#[test]
fn drops_battery_topic_cmd_from_host() {
    // Cover the one `_ble`-gated topic explicitly.
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
    // Unknown request tags reply UnknownCmd; unknown topic tags are dropped.
    link_session(&service, async |client| {
        let mut header = [0u8; RYNK_HEADER_SIZE];
        header[0..2].copy_from_slice(&0x00FFu16.to_le_bytes());
        header[2] = 0x21; // seq — echoed on the error reply
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

        // Session stays in sync afterwards.
        let version = client.request::<(), ProtocolVersion>(Cmd::GetVersion, 0x23, &()).await;
        assert_eq!(version, Ok(ProtocolVersion::CURRENT));
    });
}

#[test]
fn lock_endpoints_dispatch() {
    // `insecure` keeps lock endpoints reachable over the wire.
    let service = service();
    link_session(&service, async |client| {
        // Insecure means unlocked with no challenge.
        let status = client
            .request::<(), LockStatus>(Cmd::GetLockStatus, 0x01, &())
            .await
            .expect("lock status");
        assert!(!status.locked);
        assert!(status.key_positions.is_empty());

        // Lock is a no-op on insecure config.
        assert_eq!(client.request::<(), ()>(Cmd::Lock, 0x02, &()).await, Ok(()));
        let status = client
            .request::<(), LockStatus>(Cmd::GetLockStatus, 0x03, &())
            .await
            .expect("lock status");
        assert!(!status.locked, "insecure device ignores wire Lock");

        // No configured keys means nothing to hold.
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
    // One session, three distinct seq correlations.
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
    // Oversized payload declarations must drain and resync.
    let recovered = link_session(&service, async |client| {
        let payload_n = (RYNK_BUFFER_SIZE - RYNK_HEADER_SIZE + 1) as u16;
        let mut bad = [0u8; RYNK_HEADER_SIZE];
        bad[0..2].copy_from_slice(&Cmd::GetVersion.to_le_bytes());
        bad[2] = 0x55; // seq — echoed on the error reply
        bad[3..5].copy_from_slice(&payload_n.to_le_bytes());
        client.send_raw(&bad).await;
        // Bogus payload to drain.
        client.send_raw(&vec![0xAB; payload_n as usize]).await;

        let err = client.recv_response(0x55).await;
        assert_eq!(
            err.envelope::<()>(),
            Err(RynkError::Malformed),
            "oversized frame → Malformed"
        );

        // Clean request after resync still round-trips.
        client.request::<(), ProtocolVersion>(Cmd::GetVersion, 0x56, &()).await
    });
    assert_eq!(recovered, Ok(ProtocolVersion::CURRENT));
}

#[test]
fn topic_layer_change() {
    let service = service();
    let v = link_session(&service, async |client| {
        client.handshake().await;
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
        client.handshake().await;
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
        client.handshake().await;
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
        client.handshake().await;
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
        client.handshake().await;
        // Non-default status proves the published value crossed the wire.
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
        client.handshake().await;
        publish_event(BatteryStatusEvent(expected));
        let frame = client.recv_topic().await;
        assert_eq!(frame.header.cmd, Cmd::BatteryStatusChange);
        frame.raw::<BatteryStatus>()
    });
    assert_eq!(v, expected);
}
