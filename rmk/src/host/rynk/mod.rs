//! Rynk host service — RMK-native protocol server.
//!
//! `RynkService` is the transport-agnostic core. It holds a
//! [`KeyboardContext`](super::context::KeyboardContext) and exposes:
//!
//! - [`dispatch`](RynkService::dispatch) — process inbound message in-place
//! - [`write_topic`](RynkService::write_topic) — fill a buffer with one
//!   topic message for the per-transport publisher task.

mod handlers;
mod topics;
pub mod transport;
pub(crate) mod wire;

use rmk_types::protocol::rynk::{Cmd, RYNK_MIN_BUFFER_SIZE, RynkError, RynkMessage};
// TODO: implement and remove this comment
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

/// Max packet size to use for the Rynk USB BULK IN/OUT endpoints.
#[cfg(not(feature = "_no_usb"))]
pub const RYNK_USB_MAX_PACKET_SIZE: u16 = 64;

/// Maximum BLE chunk size that fits in a single GATT write
#[cfg(feature = "_ble")]
pub const RYNK_BLE_CHUNK_SIZE: usize = 244;

/// Transport-agnostic Rynk service.
pub struct RynkService<'a> {
    pub(super) ctx: KeyboardContext<'a>,
}

impl<'a> RynkService<'a> {
    pub fn new(keymap: &'a KeyMap<'a>) -> Self {
        Self {
            ctx: KeyboardContext::new(keymap),
        }
    }

    /// Process one inbound message in place. Always writes a response
    /// envelope (Ok or Err) into `msg`; `cmd` and `seq` are echoed verbatim.
    pub async fn dispatch(&self, msg: &mut [u8]) {
        let payload_len: usize = match self.handle(msg).await {
            Ok(n) => n,
            Err(e) => msg
                .payload_mut()
                .and_then(|p| Self::write_error_response(p, e))
                .unwrap_or(0),
        };
        if let Err(e) = msg.set_payload_len(payload_len as u16) {
            error!("Rynk dispatch failed to write payload_len: {:?}", e);
        }
    }

    async fn handle(&self, msg: &mut [u8]) -> Result<usize, RynkError> {
        let cmd = msg.cmd()?;
        let payload = msg.payload_mut()?;
        let payload_len = match cmd {
            // ── System ──
            Cmd::GetVersion => self.handle_get_version(payload).await?,
            Cmd::GetCapabilities => self.handle_get_capabilities(payload).await?,
            Cmd::Reboot => self.handle_reboot(payload).await?,
            Cmd::BootloaderJump => self.handle_bootloader_jump(payload).await?,
            Cmd::StorageReset => self.handle_storage_reset(payload).await?,

            // ── Keymap (incl. encoder) ──
            Cmd::GetKeyAction => self.handle_get_key_action(payload).await?,
            Cmd::SetKeyAction => self.handle_set_key_action(payload).await?,
            Cmd::GetDefaultLayer => self.handle_get_default_layer(payload).await?,
            Cmd::SetDefaultLayer => self.handle_set_default_layer(payload).await?,
            Cmd::GetEncoderAction => self.handle_get_encoder_action(payload).await?,
            Cmd::SetEncoderAction => self.handle_set_encoder_action(payload).await?,
            #[cfg(feature = "bulk_transfer")]
            Cmd::GetKeymapBulk => self.handle_get_keymap_bulk(payload).await?,
            #[cfg(feature = "bulk_transfer")]
            Cmd::SetKeymapBulk => self.handle_set_keymap_bulk(payload).await?,

            // ── Macro ──
            Cmd::GetMacro => self.handle_get_macro(payload).await?,
            Cmd::SetMacro => self.handle_set_macro(payload).await?,

            // ── Combo ──
            Cmd::GetCombo => self.handle_get_combo(payload).await?,
            Cmd::SetCombo => self.handle_set_combo(payload).await?,
            #[cfg(feature = "bulk_transfer")]
            Cmd::GetComboBulk => self.handle_get_combo_bulk(payload).await?,
            #[cfg(feature = "bulk_transfer")]
            Cmd::SetComboBulk => self.handle_set_combo_bulk(payload).await?,

            // ── Morse ──
            Cmd::GetMorse => self.handle_get_morse(payload).await?,
            Cmd::SetMorse => self.handle_set_morse(payload).await?,
            #[cfg(feature = "bulk_transfer")]
            Cmd::GetMorseBulk => self.handle_get_morse_bulk(payload).await?,
            #[cfg(feature = "bulk_transfer")]
            Cmd::SetMorseBulk => self.handle_set_morse_bulk(payload).await?,

            // ── Fork ──
            Cmd::GetFork => self.handle_get_fork(payload).await?,
            Cmd::SetFork => self.handle_set_fork(payload).await?,

            // ── Behavior ──
            Cmd::GetBehaviorConfig => self.handle_get_behavior_config(payload).await?,
            Cmd::SetBehaviorConfig => self.handle_set_behavior_config(payload).await?,

            // ── Connection ──
            Cmd::GetConnectionType => self.handle_get_connection_type(payload).await?,
            #[cfg(feature = "_ble")]
            Cmd::GetBleStatus => self.handle_get_ble_status(payload).await?,
            #[cfg(feature = "_ble")]
            Cmd::SwitchBleProfile => self.handle_switch_ble_profile(payload).await?,
            #[cfg(feature = "_ble")]
            Cmd::ClearBleProfile => self.handle_clear_ble_profile(payload).await?,

            // ── Status ──
            Cmd::GetCurrentLayer => self.handle_get_current_layer(payload).await?,
            Cmd::GetMatrixState => self.handle_get_matrix_state(payload).await?,
            #[cfg(feature = "_ble")]
            Cmd::GetBatteryStatus => self.handle_get_battery_status(payload).await?,
            #[cfg(all(feature = "_ble", feature = "split"))]
            Cmd::GetPeripheralStatus => self.handle_get_peripheral_status(payload).await?,
            Cmd::GetWpm => self.handle_get_wpm(payload).await?,
            Cmd::GetSleepState => self.handle_get_sleep_state(payload).await?,
            Cmd::GetLedIndicator => self.handle_get_led_indicator(payload).await?,

            // Topic CMDs — host shouldn't be sending these as requests.
            Cmd::LayerChange | Cmd::WpmUpdate | Cmd::ConnectionChange | Cmd::SleepState | Cmd::LedIndicator => {
                return Err(RynkError::InvalidRequest);
            }
            #[cfg(feature = "_ble")]
            Cmd::BatteryStatusTopic | Cmd::BleStatusChangeTopic => return Err(RynkError::InvalidRequest),
        };
        Ok(payload_len)
    }

    /// Encode `value` as the `Ok` arm of a `Result<T, RynkError>` envelope.
    pub(crate) fn write_response<T: serde::Serialize>(value: &T, payload: &mut [u8]) -> Result<usize, RynkError> {
        postcard::to_slice(&Ok::<&T, RynkError>(value), payload)
            .map(|s| s.len())
            .map_err(|_| RynkError::Internal)
    }

    /// Encode `err` as the `Err` arm of a `Result<T, RynkError>` envelope.
    ///
    /// Postcard's `Err` encoding doesn't reference `T`, so
    /// `Err::<(), RynkError>(err)` is byte-identical on the wire to any
    /// `Result<T, RynkError>::Err(err)` the host expects.
    pub(crate) fn write_error_response(payload: &mut [u8], err: RynkError) -> Result<usize, RynkError> {
        postcard::to_slice(&Err::<(), RynkError>(err), payload)
            .map(|s| s.len())
            .map_err(|_| RynkError::Internal)
    }

    /// Build a topic message in `msg`: header (cmd, seq=0, payload_len) and
    /// postcard-encoded payload. The full message occupies
    /// `&msg[..RYNK_HEADER_SIZE + msg.payload_len()]` after this returns.
    pub fn write_topic<T: serde::Serialize>(&self, cmd: Cmd, value: &T, msg: &mut [u8]) -> Result<(), RynkError> {
        debug_assert!(cmd.is_topic(), "write_topic called with non-topic cmd");
        msg.set_cmd(cmd)?;
        msg.set_seq(0)?;
        let n = postcard::to_slice(value, msg.payload_mut()?)
            .map(|s| s.len())
            .map_err(|_| RynkError::Internal)?;
        msg.set_payload_len(n as u16)?;
        Ok(())
    }
}
