//! Rynk host service — RMK-native protocol server.
//!
//! `RynkService` is the transport-agnostic core. It holds a
//! [`KeyboardContext`](super::context::KeyboardContext) and exposes:
//!
//! - [`dispatch`](RynkService::dispatch) — turn one inbound frame into
//!   one outbound frame.
//! - [`encode_topic`](RynkService::encode_topic) — build a topic frame
//!   for the per-transport publisher task (Phase 5).
//!
//! Per-transport adapters (`UsbTransport`, `BleTransport`) live under
//! [`transport`] and call `dispatch` inside their own RX/TX loop.

pub(crate) mod codec;
mod handlers;
mod snapshot;
mod topics;
pub mod transport;

pub use snapshot::run_topic_snapshot;

use rmk_types::protocol::rynk::header::HEADER_SIZE;
use rmk_types::protocol::rynk::{Cmd, Header, RYNK_MIN_BUFFER_SIZE};
// Re-exports used by macro-generated entry code (Phase 6). The `unused`
// lint can flip on for feature combos where the macro path isn't compiled —
// keep them at the module surface so manually-driven examples still see
// them. `RynkBleTransport` is crate-internal; the user-facing handle is
// `BleTransport::with_rynk_service`.
#[cfg(feature = "_ble")]
#[allow(unused_imports)]
pub(crate) use transport::RynkBleTransport;
#[cfg(not(feature = "_no_usb"))]
pub use transport::RynkUsbTransport;

use super::context::KeyboardContext;
use crate::keymap::KeyMap;

const _: () = assert!(
    rmk_types::constants::RYNK_BUFFER_SIZE >= RYNK_MIN_BUFFER_SIZE,
    "rynk_buffer_size is smaller than RYNK_MIN_BUFFER_SIZE — set [rmk] \
     rynk_buffer_size in keyboard.toml, or disable features to shrink the \
     floor",
);

/// Max packet size to use for the Rynk USB BULK IN/OUT endpoints. 64 B is
/// the full-speed maximum and works on every embassy-usb driver; HS-only
/// devices can override at the call site for higher throughput.
#[cfg(not(feature = "_no_usb"))]
pub const RYNK_USB_MAX_PACKET_SIZE: u16 = 64;

/// Maximum BLE chunk size that fits in a single GATT write — matches the
/// `output_data` characteristic's value-array length in `ble_server.rs`
/// (≈ MTU − 3 for the typical 247-byte negotiated MTU).
#[cfg(feature = "_ble")]
pub const RYNK_BLE_CHUNK_SIZE: usize = 244;

/// Transport-agnostic Rynk dispatch core.
///
/// Construct via [`RynkService::new`] with a borrowed `KeyMap`. Hand it
/// to one or both transports' `run` futures and join the futures into
/// the existing `::rmk::run_all!(…)` chain (same pattern Vial uses for
/// `host_service.run()`).
pub struct RynkService<'a> {
    pub(super) ctx: KeyboardContext<'a>,
}

impl<'a> RynkService<'a> {
    /// Build a service over a `&KeyMap`. The keymap outlives every
    /// transport future, so the borrow is sound.
    pub fn new(keymap: &'a KeyMap<'a>) -> Self {
        Self {
            ctx: KeyboardContext::new(keymap),
        }
    }

    /// Decode one inbound frame, run the matching handler, and write the
    /// response frame (header + payload) into `out`. Returns the total
    /// response byte count, or `0` if the frame should produce no reply
    /// (malformed, topic-from-host, or one of the reset commands).
    pub async fn dispatch(&self, frame_in: &[u8], out: &mut [u8]) -> usize {
        let (header, payload) = match Header::decode(frame_in) {
            Ok(x) => x,
            Err(_) => return 0,
        };
        if header.cmd.is_topic() {
            return 0;
        }

        // Reserve the first 5 bytes for the response header so handlers
        // see only the payload buffer.
        if out.len() < HEADER_SIZE {
            return 0;
        }
        let (header_slot, payload_out) = out.split_at_mut(HEADER_SIZE);
        let payload_len = self.handle(header.cmd, payload, payload_out).await;

        Header {
            cmd: header.cmd,
            seq: header.seq,
            len: payload_len as u16,
        }
        .encode_into(header_slot);

        HEADER_SIZE + payload_len
    }

    /// Build a topic frame (header + payload) into `out`. Used by the
    /// per-transport publisher task in Phase 5.
    pub fn encode_topic<T: serde::Serialize>(&self, cmd: Cmd, value: &T, out: &mut [u8]) -> usize {
        debug_assert!(cmd.is_topic(), "encode_topic called with non-topic cmd");
        if out.len() < HEADER_SIZE {
            return 0;
        }
        let (header_slot, payload_out) = out.split_at_mut(HEADER_SIZE);
        let payload_len = postcard::to_slice(value, payload_out).map(|s| s.len()).unwrap_or(0);
        Header {
            cmd,
            seq: 0,
            len: payload_len as u16,
        }
        .encode_into(header_slot);
        HEADER_SIZE + payload_len
    }

    /// Central dispatch table — one match arm per `Cmd` variant.
    ///
    /// Adding a Cmd:
    /// 1. Append the variant in `rmk-types/src/protocol/rynk/cmd.rs`.
    /// 2. Add the match arm here.
    /// 3. Add the matching `handle_xxx` method in the relevant
    ///    `handlers/*.rs` file.
    ///
    /// All three steps must agree or `cargo build` fails.
    async fn handle(&self, cmd: Cmd, req: &[u8], out: &mut [u8]) -> usize {
        match cmd {
            // ── System ──
            Cmd::GetVersion => self.handle_get_version(req, out).await,
            Cmd::GetCapabilities => self.handle_get_capabilities(req, out).await,
            Cmd::Reboot => self.handle_reboot(req, out).await,
            Cmd::BootloaderJump => self.handle_bootloader_jump(req, out).await,
            Cmd::StorageReset => self.handle_storage_reset(req, out).await,

            // ── Keymap (incl. encoder) ──
            Cmd::GetKeyAction => self.handle_get_key_action(req, out).await,
            Cmd::SetKeyAction => self.handle_set_key_action(req, out).await,
            Cmd::GetDefaultLayer => self.handle_get_default_layer(req, out).await,
            Cmd::SetDefaultLayer => self.handle_set_default_layer(req, out).await,
            Cmd::GetEncoderAction => self.handle_get_encoder_action(req, out).await,
            Cmd::SetEncoderAction => self.handle_set_encoder_action(req, out).await,
            #[cfg(feature = "bulk_transfer")]
            Cmd::GetKeymapBulk => self.handle_get_keymap_bulk(req, out).await,
            #[cfg(feature = "bulk_transfer")]
            Cmd::SetKeymapBulk => self.handle_set_keymap_bulk(req, out).await,

            // ── Macro ──
            Cmd::GetMacro => self.handle_get_macro(req, out).await,
            Cmd::SetMacro => self.handle_set_macro(req, out).await,

            // ── Combo ──
            Cmd::GetCombo => self.handle_get_combo(req, out).await,
            Cmd::SetCombo => self.handle_set_combo(req, out).await,
            #[cfg(feature = "bulk_transfer")]
            Cmd::GetComboBulk => self.handle_get_combo_bulk(req, out).await,
            #[cfg(feature = "bulk_transfer")]
            Cmd::SetComboBulk => self.handle_set_combo_bulk(req, out).await,

            // ── Morse ──
            Cmd::GetMorse => self.handle_get_morse(req, out).await,
            Cmd::SetMorse => self.handle_set_morse(req, out).await,
            #[cfg(feature = "bulk_transfer")]
            Cmd::GetMorseBulk => self.handle_get_morse_bulk(req, out).await,
            #[cfg(feature = "bulk_transfer")]
            Cmd::SetMorseBulk => self.handle_set_morse_bulk(req, out).await,

            // ── Fork ──
            Cmd::GetFork => self.handle_get_fork(req, out).await,
            Cmd::SetFork => self.handle_set_fork(req, out).await,

            // ── Behavior ──
            Cmd::GetBehaviorConfig => self.handle_get_behavior_config(req, out).await,
            Cmd::SetBehaviorConfig => self.handle_set_behavior_config(req, out).await,

            // ── Connection ──
            Cmd::GetConnectionType => self.handle_get_connection_type(req, out).await,
            #[cfg(feature = "_ble")]
            Cmd::GetBleStatus => self.handle_get_ble_status(req, out).await,
            #[cfg(feature = "_ble")]
            Cmd::SwitchBleProfile => self.handle_switch_ble_profile(req, out).await,
            #[cfg(feature = "_ble")]
            Cmd::ClearBleProfile => self.handle_clear_ble_profile(req, out).await,

            // ── Status ──
            Cmd::GetCurrentLayer => self.handle_get_current_layer(req, out).await,
            Cmd::GetMatrixState => self.handle_get_matrix_state(req, out).await,
            #[cfg(feature = "_ble")]
            Cmd::GetBatteryStatus => self.handle_get_battery_status(req, out).await,
            #[cfg(all(feature = "_ble", feature = "split"))]
            Cmd::GetPeripheralStatus => self.handle_get_peripheral_status(req, out).await,
            Cmd::GetWpm => self.handle_get_wpm(req, out).await,
            Cmd::GetSleepState => self.handle_get_sleep_state(req, out).await,
            Cmd::GetLedIndicator => self.handle_get_led_indicator(req, out).await,

            // Topics never reach here (checked above) but the compiler can't
            // prove that — exhaustively match them with an unreachable arm.
            Cmd::LayerChange | Cmd::WpmUpdate | Cmd::ConnectionChange | Cmd::SleepState | Cmd::LedIndicator => 0,
            #[cfg(feature = "_ble")]
            Cmd::BatteryStatusTopic | Cmd::BleStatusChangeTopic => 0,
        }
    }
}
