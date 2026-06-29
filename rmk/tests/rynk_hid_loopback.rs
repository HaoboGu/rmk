//! HID-framed loopback: the same production [`RynkService::run_session`] as
//! `rynk_loopback.rs`, but every exchange crosses the fixed 32-byte HID report
//! framing (firmware `RynkHidService`; de-framed at the `ble::rynk` seam via
//! `drop_report_padding`, reply-framed by `ble::rynk::RynkBleTx`). Proves the
//! framing round-trips through the real dispatcher: single-report frames,
//! multi-report reassembly (`GetCapabilities` > 32 B), a pipelined two-request
//! session, and a server→host topic push.

#![cfg(feature = "rynk")]

pub mod common;

use rmk::config::{BehaviorConfig, PositionalConfig, RmkConfig};
use rmk::event::{WpmUpdateEvent, publish_event};
use rmk::host::HostService as RynkService;
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};
use rmk_types::protocol::rynk::{Cmd, DeviceCapabilities, KeyPosition, ProtocolVersion, SetKeyRequest};

use crate::common::rynk_hid_link::link_session_hid;
use crate::common::wrap_keymap;

/// A 2-layer 2×2 `RynkService`, leaked to `'static` (see `rynk_loopback.rs`).
fn service() -> RynkService<'static> {
    let behavior: &'static mut BehaviorConfig = Box::leak(Box::new(BehaviorConfig::default()));
    let per_key: &'static PositionalConfig<2, 2> = Box::leak(Box::new(PositionalConfig::default()));
    let keymap = [[[KeyAction::No; 2]; 2]; 1];
    let km = wrap_keymap(keymap, per_key, behavior);
    let config: &'static RmkConfig<'static> = Box::leak(Box::new(RmkConfig::default()));
    RynkService::new(km, config)
}

/// Smallest exchange: an empty request and an 8-byte response each fit one report.
#[test]
fn get_version_over_hid_framing() {
    let service = service();
    link_session_hid(&service, async |client| {
        let version = client.request::<(), ProtocolVersion>(Cmd::GetVersion, 0x42, &()).await;
        assert_eq!(version, Ok(ProtocolVersion::CURRENT));
    });
}

/// `DeviceCapabilities` exceeds one 32-byte report, so its reply spans several —
/// this is the case that actually exercises multi-report Tx framing + the
/// client's reassembly.
#[test]
fn get_capabilities_spans_multiple_reports() {
    let service = service();
    link_session_hid(&service, async |client| {
        let caps = client
            .request::<(), DeviceCapabilities>(Cmd::GetCapabilities, 0x07, &())
            .await
            .expect("Ok envelope");
        assert_eq!(caps.num_layers, 1);
        assert_eq!(caps.num_rows, 2);
        assert_eq!(caps.num_cols, 2);
        assert_eq!(caps.storage_enabled, cfg!(feature = "storage"));
        assert_eq!(caps.ble_enabled, cfg!(feature = "_ble"));
    });
}

/// Two requests pipelined over one session: the stream stays alive across the
/// Set response (no spurious EOF), and the Get reads back the mutated state.
#[test]
fn set_then_get_key_action_round_trip() {
    let service = service();
    link_session_hid(&service, async |client| {
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

/// Server→host push: a topic frame emitted between turns must also survive the
/// HID framing (here a single small report).
#[test]
fn topic_push_over_hid_framing() {
    let service = service();
    let v = link_session_hid(&service, async |client| {
        publish_event(WpmUpdateEvent::new(42));
        let frame = client.recv_topic().await;
        assert_eq!(frame.header.cmd, Cmd::WpmUpdate);
        frame.raw::<u16>()
    });
    assert_eq!(v, 42);
}
