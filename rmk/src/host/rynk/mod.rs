//! Rynk host service — RMK-native protocol server.
//!
//! `RynkService` is the transport-agnostic core. It holds a
//! [`KeyboardContext`](super::context::KeyboardContext) and exposes:
//!
//! - [`dispatch`](RynkService::dispatch) — process inbound message in-place.
//! - [`run_session`](RynkService::run_session) — drive one rynk session
//!   against a wire transport until it closes; emits topic frames between
//!   request/response turns.
//!
//! Topic handling lives in [`topics::TopicSubscribers`] which the session
//! drains; cache-only events (peripheral status) are mirrored into `ctx`
//! and not surfaced on the wire.

mod handlers;
pub(crate) mod topics;
// Shared error for RMK-authored adapters; only BLE hand-writes one today.
#[cfg(feature = "_ble")]
pub mod transport;
pub mod uart;

use embassy_futures::select::{Either, select};
use embedded_io_async::{Read, Write};
use rmk_types::constants::RYNK_BUFFER_SIZE;
use rmk_types::protocol::rynk::{Cmd, RYNK_HEADER_SIZE, RYNK_MIN_BUFFER_SIZE, RynkError, RynkMessage};
#[allow(unused_imports)] // re-exported at `crate::host` for downstream users
pub use uart::run_rynk_uart;

use super::context::KeyboardContext;
use crate::config::RmkConfig;
use crate::keymap::KeyMap;

const _: () = assert!(
    rmk_types::constants::RYNK_BUFFER_SIZE >= RYNK_MIN_BUFFER_SIZE,
    "rynk_buffer_size is smaller than RYNK_MIN_BUFFER_SIZE — set [rmk] \
     rynk_buffer_size in keyboard.toml, or disable features to shrink the \
     floor",
);

/// Maximum BLE chunk size that fits in a single GATT write
#[cfg(feature = "_ble")]
pub const RYNK_BLE_CHUNK_SIZE: usize = 244;

/// Transport-agnostic Rynk service.
pub struct RynkService<'a> {
    pub(super) ctx: KeyboardContext<'a>,
}

impl<'a> RynkService<'a> {
    pub fn new(keymap: &'a KeyMap<'a>, _config: &RmkConfig<'static>) -> Self {
        Self {
            ctx: KeyboardContext::new(keymap),
        }
    }

    /// Process one inbound message in place. Always writes a response
    /// envelope (Ok or Err) into `msg`; `cmd` and `seq` are echoed verbatim.
    pub async fn dispatch(&self, msg: &mut RynkMessage<'_>) {
        let payload_len: usize = match self.handle(msg).await {
            Ok(n) => n,
            // Postcard's `Err` encoding doesn't reference `T`, so
            // `Err::<(), RynkError>(e)` is byte-identical on the wire to
            // any `Result<T, RynkError>::Err(e)` the host expects.
            Err(e) => postcard::to_slice(&Err::<(), RynkError>(e), msg.payload_mut())
                .map(|s| s.len())
                .unwrap_or(0),
        };
        msg.set_payload_len(payload_len as u16);
    }

    async fn handle(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        let cmd = msg.cmd();
        let payload = msg.payload_mut();
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
                return Err(RynkError::Invalid);
            }
            #[cfg(feature = "_ble")]
            Cmd::BatteryStatusTopic => return Err(RynkError::Invalid),
        };
        Ok(payload_len)
    }

    /// Encode `value` as the `Ok` arm of a `Result<T, RynkError>` envelope.
    pub(crate) fn write_response<T: serde::Serialize>(value: &T, payload: &mut [u8]) -> Result<usize, RynkError> {
        postcard::to_slice(&Ok::<&T, RynkError>(value), payload)
            .map(|s| s.len())
            .map_err(|_| RynkError::Internal)
    }
}

impl RynkService<'_> {
    /// Drive one rynk session against `rx`/`tx`. Returns when either half
    /// reports an error or EOF (USB endpoint disabled, BLE disconnect,
    /// UART read failure).
    ///
    /// Owns the reassembly/dispatch buffer, parses frames as `5 + LEN`
    /// headers, dispatches each frame in place via
    /// [`dispatch`](RynkService::dispatch), and emits topic frames between
    /// request/response turns. Transport-specific setup (USB
    /// `wait_connection`, BLE per-connection channel reset) and reconnect
    /// both stay in the caller — this returns on close rather than looping.
    pub async fn run_session<R: Read, T: Write>(&self, rx: &mut R, tx: &mut T) {
        let mut buf = [0u8; RYNK_BUFFER_SIZE];
        let mut topics = topics::TopicSubscribers::new();
        let mut rx_used = 0usize;

        loop {
            let recv_result = if rx_used == 0 {
                // Idle: race the wire against topic events. Topic emission
                // is only safe before frame accumulation starts — once
                // `rx_used > 0` the buffer is committed to that frame.
                match select(rx.read(&mut buf), topics.next_event(&self.ctx)).await {
                    Either::First(r) => r,
                    Either::Second(event) => {
                        match event.encode(&mut buf) {
                            Ok(msg) => {
                                let total = msg.frame_len();
                                if tx.write_all(&buf[..total]).await.is_err() {
                                    return;
                                }
                            }
                            Err(e) => warn!("Rynk topic encode failed: {:?}", e),
                        }
                        continue;
                    }
                }
            } else {
                let tail = &mut buf[rx_used..];
                if tail.is_empty() {
                    warn!("Rynk RX overflow; resyncing");
                    rx_used = 0;
                    continue;
                }
                rx.read(tail).await
            };

            match recv_result {
                Ok(0) => return, // EOF
                Ok(n) => rx_used += n,
                Err(_) => return,
            }

            if rx_used >= RYNK_HEADER_SIZE {
                let payload_n = u16::from_le_bytes([buf[3], buf[4]]) as usize;
                let frame_len = RYNK_HEADER_SIZE + payload_n;
                if rx_used < frame_len {
                    continue;
                }
                if rx_used > frame_len {
                    // Trailing bytes get clobbered by the in-place response
                    // below — rynk callers don't pipeline, but warn so a
                    // misbehaving host is visible.
                    warn!(
                        "Rynk: discarding {} trailing byte(s) past frame end",
                        rx_used - frame_len
                    );
                }
                let resp_len = match RynkMessage::try_from(&mut buf[..frame_len]) {
                    Ok(mut msg) => {
                        self.dispatch(&mut msg).await;
                        msg.frame_len()
                    }
                    Err(e) => {
                        warn!("Rynk: invalid frame: {:?}", e);
                        rx_used = 0;
                        continue;
                    }
                };
                if tx.write_all(&buf[..resp_len]).await.is_err() {
                    return;
                }
                rx_used = 0;
            }
        }
    }
}
