//! Rynk host service — RMK-native protocol server.
//!
//! `RynkService` is the transport-agnostic core. It holds a
//! [`KeyboardContext`](super::context::KeyboardContext) and exposes:
//!
//! - [`dispatch`](RynkService::dispatch) — turn one inbound message into
//!   one outbound message, in-place on the same buffer.
//! - [`write_topic`](RynkService::write_topic) — fill a buffer with one
//!   topic message for the per-transport publisher task.
//!
//! Per-transport adapters (`UsbTransport`, `BleTransport`) live under
//! [`transport`] and call `dispatch` inside their own RX/TX loop.

mod handlers;
mod topics;
pub mod transport;
pub(crate) mod wire;

use rmk_types::protocol::rynk::{Cmd, RYNK_HEADER_SIZE, RYNK_MIN_BUFFER_SIZE, RynkError, RynkMessage};
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

    /// Process one inbound message in place: route on `cmd`, let the
    /// matching handler write the response payload into the same
    /// buffer, then patch the `LEN` field.
    ///
    /// `cmd` and `seq` are echoed verbatim from request to response —
    /// they're never re-written. The bytes after `RYNK_HEADER_SIZE +
    /// msg.payload_len()` are unspecified after this returns.
    ///
    /// On a malformed header or a topic CMD sent as a request, returns
    /// `Err(RynkError::InvalidRequest)` and the transport drops the
    /// message. Handler-level errors are encoded into the payload as
    /// `Err::<(), RynkError>(e)` and reported as `Ok(())` — every
    /// legitimate request gets a reply.
    ///
    /// Adding a `Cmd`:
    /// 1. Append the variant in `rmk-types/src/protocol/rynk/cmd.rs`.
    /// 2. Add the match arm here.
    /// 3. Add the matching `handle_xxx` method in the relevant
    ///    `handlers/*.rs` file.
    pub async fn dispatch(&self, msg: &mut [u8]) -> Result<(), RynkError> {
        let cmd = msg.cmd()?;

        let handler_result: Result<usize, RynkError> = match cmd {
            // ── System ──
            Cmd::GetVersion => self.handle_get_version(msg.payload_mut()?).await,
            Cmd::GetCapabilities => self.handle_get_capabilities(msg.payload_mut()?).await,
            Cmd::Reboot => self.handle_reboot(msg.payload_mut()?).await,
            Cmd::BootloaderJump => self.handle_bootloader_jump(msg.payload_mut()?).await,
            Cmd::StorageReset => self.handle_storage_reset(msg.payload_mut()?).await,

            // ── Keymap (incl. encoder) ──
            Cmd::GetKeyAction => self.handle_get_key_action(msg.payload_mut()?).await,
            Cmd::SetKeyAction => self.handle_set_key_action(msg.payload_mut()?).await,
            Cmd::GetDefaultLayer => self.handle_get_default_layer(msg.payload_mut()?).await,
            Cmd::SetDefaultLayer => self.handle_set_default_layer(msg.payload_mut()?).await,
            Cmd::GetEncoderAction => self.handle_get_encoder_action(msg.payload_mut()?).await,
            Cmd::SetEncoderAction => self.handle_set_encoder_action(msg.payload_mut()?).await,
            #[cfg(feature = "bulk_transfer")]
            Cmd::GetKeymapBulk => self.handle_get_keymap_bulk(msg.payload_mut()?).await,
            #[cfg(feature = "bulk_transfer")]
            Cmd::SetKeymapBulk => self.handle_set_keymap_bulk(msg.payload_mut()?).await,

            // ── Macro ──
            Cmd::GetMacro => self.handle_get_macro(msg.payload_mut()?).await,
            Cmd::SetMacro => self.handle_set_macro(msg.payload_mut()?).await,

            // ── Combo ──
            Cmd::GetCombo => self.handle_get_combo(msg.payload_mut()?).await,
            Cmd::SetCombo => self.handle_set_combo(msg.payload_mut()?).await,
            #[cfg(feature = "bulk_transfer")]
            Cmd::GetComboBulk => self.handle_get_combo_bulk(msg.payload_mut()?).await,
            #[cfg(feature = "bulk_transfer")]
            Cmd::SetComboBulk => self.handle_set_combo_bulk(msg.payload_mut()?).await,

            // ── Morse ──
            Cmd::GetMorse => self.handle_get_morse(msg.payload_mut()?).await,
            Cmd::SetMorse => self.handle_set_morse(msg.payload_mut()?).await,
            #[cfg(feature = "bulk_transfer")]
            Cmd::GetMorseBulk => self.handle_get_morse_bulk(msg.payload_mut()?).await,
            #[cfg(feature = "bulk_transfer")]
            Cmd::SetMorseBulk => self.handle_set_morse_bulk(msg.payload_mut()?).await,

            // ── Fork ──
            Cmd::GetFork => self.handle_get_fork(msg.payload_mut()?).await,
            Cmd::SetFork => self.handle_set_fork(msg.payload_mut()?).await,

            // ── Behavior ──
            Cmd::GetBehaviorConfig => self.handle_get_behavior_config(msg.payload_mut()?).await,
            Cmd::SetBehaviorConfig => self.handle_set_behavior_config(msg.payload_mut()?).await,

            // ── Connection ──
            Cmd::GetConnectionType => self.handle_get_connection_type(msg.payload_mut()?).await,
            #[cfg(feature = "_ble")]
            Cmd::GetBleStatus => self.handle_get_ble_status(msg.payload_mut()?).await,
            #[cfg(feature = "_ble")]
            Cmd::SwitchBleProfile => self.handle_switch_ble_profile(msg.payload_mut()?).await,
            #[cfg(feature = "_ble")]
            Cmd::ClearBleProfile => self.handle_clear_ble_profile(msg.payload_mut()?).await,

            // ── Status ──
            Cmd::GetCurrentLayer => self.handle_get_current_layer(msg.payload_mut()?).await,
            Cmd::GetMatrixState => self.handle_get_matrix_state(msg.payload_mut()?).await,
            #[cfg(feature = "_ble")]
            Cmd::GetBatteryStatus => self.handle_get_battery_status(msg.payload_mut()?).await,
            #[cfg(all(feature = "_ble", feature = "split"))]
            Cmd::GetPeripheralStatus => self.handle_get_peripheral_status(msg.payload_mut()?).await,
            Cmd::GetWpm => self.handle_get_wpm(msg.payload_mut()?).await,
            Cmd::GetSleepState => self.handle_get_sleep_state(msg.payload_mut()?).await,
            Cmd::GetLedIndicator => self.handle_get_led_indicator(msg.payload_mut()?).await,

            // Topic CMDs — host shouldn't be sending these.
            Cmd::LayerChange | Cmd::WpmUpdate | Cmd::ConnectionChange | Cmd::SleepState | Cmd::LedIndicator => {
                return Err(RynkError::InvalidRequest);
            }
            #[cfg(feature = "_ble")]
            Cmd::BatteryStatusTopic | Cmd::BleStatusChangeTopic => {
                return Err(RynkError::InvalidRequest);
            }
        };

        let payload_len = match handler_result {
            Ok(n) => n,
            Err(e) => {
                let envelope: Result<(), RynkError> = Err(e);
                postcard::to_slice(&envelope, msg.payload_mut()?)
                    .map(|s| s.len())
                    .unwrap_or(0)
            }
        };
        msg.set_payload_len(payload_len as u16)?;
        Ok(())
    }

    /// Build a topic message in `msg`: writes the header (cmd, seq=0,
    /// payload_len) and the postcard-encoded payload. The full message
    /// occupies `&msg[..RYNK_HEADER_SIZE + msg.payload_len()]`
    /// after this returns. Returns `Err(InvalidRequest)` if `msg` is
    /// shorter than `RYNK_HEADER_SIZE`; the debug assertion catches a
    /// non-topic `cmd` in dev builds.
    pub fn write_topic<T: serde::Serialize>(&self, cmd: Cmd, value: &T, msg: &mut [u8]) -> Result<(), RynkError> {
        debug_assert!(cmd.is_topic(), "write_topic called with non-topic cmd");
        msg.set_cmd(cmd)?;
        msg.set_seq(0)?;
        let n = postcard::to_slice(value, msg.payload_mut()?)
            .map(|s| s.len())
            .unwrap_or(0);
        msg.set_payload_len(n as u16)?;
        Ok(())
    }
}
