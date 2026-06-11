//! Typed endpoint methods and topic decoding — the version-specific API
//! surface over the driver core in `driver.rs`.

use embedded_io_async::{Read, Write};
use rmk_types::action::{EncoderAction, KeyAction};
use rmk_types::battery::BatteryStatus;
use rmk_types::ble::BleStatus;
use rmk_types::combo::Combo;
use rmk_types::connection::{ConnectionStatus, ConnectionType};
use rmk_types::fork::Fork;
use rmk_types::led_indicator::LedIndicator;
use rmk_types::morse::Morse;
use rmk_types::protocol::rynk::{
    BehaviorConfig, Cmd, DeviceCapabilities, GetComboBulkRequest, GetComboBulkResponse, GetEncoderRequest,
    GetKeymapBulkRequest, GetKeymapBulkResponse, GetMacroRequest, GetMorseBulkRequest, GetMorseBulkResponse,
    KeyPosition, MacroData, MatrixState, PeripheralStatus, ProtocolVersion, SetComboBulkRequest, SetComboRequest,
    SetEncoderRequest, SetForkRequest, SetKeyRequest, SetKeymapBulkRequest, SetMacroRequest, SetMorseBulkRequest,
    SetMorseRequest, StorageResetMode, TopicEvent, command,
};

use crate::driver::{Client, RequestError, TopicFrame, TransportError};

/// A firmware topic push (server → host), delivered by [`Client::next_event`].
///
/// A recognized topic decodes into [`IncomingTopic::Topic`], carrying the shared
/// [`TopicEvent`] generated from the protocol's topic table.
///
/// Topics are **best-effort**: the link can drop a push (a full in-client queue
/// — see [`Client::events_dropped`] — or, on BLE, an OS-level notification drop
/// the client cannot observe).
#[derive(Debug, Clone)]
pub enum IncomingTopic {
    /// A recognized topic, decoded by the shared topic table.
    Topic(TopicEvent),
    /// A topic this build doesn't recognize, or one whose payload failed to decode.
    Unknown(TopicFrame),
}

impl<T: Read + Write> Client<T> {
    /// Read the next topic push, decoded into a typed [`IncomingTopic`].
    /// Queued topics are returned first. Cancel-safe.
    pub async fn next_event(&mut self) -> Result<IncomingTopic, TransportError> {
        let frame = self.next_topic_frame().await?;
        Ok(match TopicEvent::decode(frame.cmd, &frame.payload) {
            Some(event) => IncomingTopic::Topic(event),
            None => IncomingTopic::Unknown(frame),
        })
    }

    // Gating is structural only (whole feature families); the firmware is the
    // authoritative validator of numeric limits, rejecting out-of-range or
    // over-capacity requests, so the host does not pre-check them.

    /// Reject a bulk command locally when the cached capabilities say bulk
    /// transfer is absent, before touching the wire.
    fn require_bulk_transfer(&self, cmd: Cmd) -> Result<(), RequestError> {
        if self.capabilities().bulk_transfer_supported {
            Ok(())
        } else {
            Err(RequestError::Unsupported(cmd, "bulk transfer not supported"))
        }
    }

    /// Reject a BLE-only command locally when the cached capabilities say BLE
    /// is absent, before touching the wire.
    fn require_ble(&self, cmd: Cmd) -> Result<(), RequestError> {
        if self.capabilities().ble_enabled {
            Ok(())
        } else {
            Err(RequestError::Unsupported(cmd, "BLE not enabled"))
        }
    }

    // ── system ──

    /// Read the firmware's protocol version.
    pub async fn get_version(&mut self) -> Result<ProtocolVersion, RequestError> {
        self.request::<command::GetVersion>(&()).await
    }

    /// Re-read the firmware's capability set. Prefer the cached
    /// [`Client::capabilities`] for the snapshot taken at connect time.
    pub async fn get_capabilities(&mut self) -> Result<DeviceCapabilities, RequestError> {
        self.request::<command::GetCapabilities>(&()).await
    }

    /// Reboot the device — fire-and-forget: the firmware resets before its
    /// session loop can reply, so `Ok(())` only means the request frame was
    /// handed to the link.
    pub async fn reboot(&mut self) -> Result<(), RequestError> {
        self.send_no_reply::<command::Reboot>(&()).await
    }

    /// Jump to the bootloader (DFU mode) — fire-and-forget, same contract as
    /// [`reboot`](Self::reboot).
    pub async fn bootloader_jump(&mut self) -> Result<(), RequestError> {
        self.send_no_reply::<command::BootloaderJump>(&()).await
    }

    /// Reset persistent storage. Rejected locally when storage is disabled
    /// ([`DeviceCapabilities::storage_enabled`]), where the wipe would be a silent
    /// no-op.
    pub async fn storage_reset(&mut self, mode: StorageResetMode) -> Result<(), RequestError> {
        if !self.capabilities().storage_enabled {
            return Err(RequestError::Unsupported(Cmd::StorageReset, "storage not enabled"));
        }
        self.request::<command::StorageReset>(&mode).await
    }

    // ── keymap ──

    /// Read one key's action.
    pub async fn get_key(&mut self, layer: u8, row: u8, col: u8) -> Result<KeyAction, RequestError> {
        self.request::<command::GetKeyAction>(&KeyPosition { layer, row, col })
            .await
    }

    /// Write one key's action.
    pub async fn set_key(&mut self, layer: u8, row: u8, col: u8, action: KeyAction) -> Result<(), RequestError> {
        let req = SetKeyRequest {
            position: KeyPosition { layer, row, col },
            action,
        };
        self.request::<command::SetKeyAction>(&req).await
    }

    /// Read the currently selected default layer index.
    pub async fn get_default_layer(&mut self) -> Result<u8, RequestError> {
        self.request::<command::GetDefaultLayer>(&()).await
    }

    /// Set the default layer.
    pub async fn set_default_layer(&mut self, layer: u8) -> Result<(), RequestError> {
        self.request::<command::SetDefaultLayer>(&layer).await
    }

    /// Read both rotation actions for one encoder on one layer.
    pub async fn get_encoder(&mut self, encoder_id: u8, layer: u8) -> Result<EncoderAction, RequestError> {
        self.request::<command::GetEncoderAction>(&GetEncoderRequest { encoder_id, layer })
            .await
    }

    /// Set both rotation actions for one encoder on one layer.
    pub async fn set_encoder(&mut self, encoder_id: u8, layer: u8, action: EncoderAction) -> Result<(), RequestError> {
        let req = SetEncoderRequest {
            encoder_id,
            layer,
            action,
        };
        self.request::<command::SetEncoderAction>(&req).await
    }

    /// Read multiple key actions starting from one key position. Bulk firmware
    /// only ([`DeviceCapabilities::bulk_transfer_supported`]); returns
    /// [`RequestError::Unsupported`] otherwise, without touching the wire.
    pub async fn get_keymap_bulk(
        &mut self,
        layer: u8,
        start_row: u8,
        start_col: u8,
        count: u8,
    ) -> Result<GetKeymapBulkResponse, RequestError> {
        self.require_bulk_transfer(Cmd::GetKeymapBulk)?;
        self.request::<command::GetKeymapBulk>(&GetKeymapBulkRequest {
            layer,
            start_row,
            start_col,
            count,
        })
        .await
    }

    /// Write multiple key actions starting from one key position. Bulk firmware
    /// only ([`DeviceCapabilities::bulk_transfer_supported`]); returns
    /// [`RequestError::Unsupported`] otherwise, without touching the wire.
    pub async fn set_keymap_bulk(&mut self, request: SetKeymapBulkRequest) -> Result<(), RequestError> {
        self.require_bulk_transfer(Cmd::SetKeymapBulk)?;
        self.request::<command::SetKeymapBulk>(&request).await
    }

    // ── combos / forks / morse / macros ──

    /// Read one combo entry by index.
    pub async fn get_combo(&mut self, index: u8) -> Result<Combo, RequestError> {
        self.request::<command::GetCombo>(&index).await
    }

    /// Write one combo entry by index.
    pub async fn set_combo(&mut self, index: u8, config: Combo) -> Result<(), RequestError> {
        self.request::<command::SetCombo>(&SetComboRequest { index, config })
            .await
    }

    /// Read multiple combo entries starting at `start_index`. Bulk firmware
    /// only ([`DeviceCapabilities::bulk_transfer_supported`]); returns
    /// [`RequestError::Unsupported`] otherwise, without touching the wire.
    pub async fn get_combo_bulk(&mut self, start_index: u8, count: u8) -> Result<GetComboBulkResponse, RequestError> {
        self.require_bulk_transfer(Cmd::GetComboBulk)?;
        self.request::<command::GetComboBulk>(&GetComboBulkRequest { start_index, count })
            .await
    }

    /// Write multiple combo entries starting at `request.start_index`. Bulk
    /// firmware only ([`DeviceCapabilities::bulk_transfer_supported`]); returns
    /// [`RequestError::Unsupported`] otherwise, without touching the wire.
    pub async fn set_combo_bulk(&mut self, request: SetComboBulkRequest) -> Result<(), RequestError> {
        self.require_bulk_transfer(Cmd::SetComboBulk)?;
        self.request::<command::SetComboBulk>(&request).await
    }

    /// Read one fork entry by index.
    pub async fn get_fork(&mut self, index: u8) -> Result<Fork, RequestError> {
        self.request::<command::GetFork>(&index).await
    }

    /// Write one fork entry by index.
    pub async fn set_fork(&mut self, index: u8, config: Fork) -> Result<(), RequestError> {
        self.request::<command::SetFork>(&SetForkRequest { index, config })
            .await
    }

    /// Read one morse entry by index.
    pub async fn get_morse(&mut self, index: u8) -> Result<Morse, RequestError> {
        self.request::<command::GetMorse>(&index).await
    }

    /// Write one morse entry by index.
    pub async fn set_morse(&mut self, index: u8, config: Morse) -> Result<(), RequestError> {
        self.request::<command::SetMorse>(&SetMorseRequest { index, config })
            .await
    }

    /// Read multiple morse entries starting at `start_index`. Bulk firmware
    /// only ([`DeviceCapabilities::bulk_transfer_supported`]); returns
    /// [`RequestError::Unsupported`] otherwise, without touching the wire.
    pub async fn get_morse_bulk(&mut self, start_index: u8, count: u8) -> Result<GetMorseBulkResponse, RequestError> {
        self.require_bulk_transfer(Cmd::GetMorseBulk)?;
        self.request::<command::GetMorseBulk>(&GetMorseBulkRequest { start_index, count })
            .await
    }

    /// Write multiple morse entries starting at `request.start_index`. Bulk
    /// firmware only ([`DeviceCapabilities::bulk_transfer_supported`]); returns
    /// [`RequestError::Unsupported`] otherwise, without touching the wire.
    pub async fn set_morse_bulk(&mut self, request: SetMorseBulkRequest) -> Result<(), RequestError> {
        self.require_bulk_transfer(Cmd::SetMorseBulk)?;
        self.request::<command::SetMorseBulk>(&request).await
    }

    /// Read one chunk of macro data starting at byte `offset`. The firmware
    /// always replies with exactly its build-time chunk size, zero-filling
    /// past the end of its macro space — a short chunk is **not** an
    /// end-of-data signal; parse the macro encoding itself for termination.
    pub async fn get_macro(&mut self, index: u8, offset: u16) -> Result<MacroData, RequestError> {
        self.request::<command::GetMacro>(&GetMacroRequest { index, offset })
            .await
    }

    /// Write one chunk of macro data starting at byte `offset`. Writes past
    /// the end of the device's macro space are truncated by the firmware.
    pub async fn set_macro(&mut self, index: u8, offset: u16, data: MacroData) -> Result<(), RequestError> {
        self.request::<command::SetMacro>(&SetMacroRequest { index, offset, data })
            .await
    }

    // ── behavior ──

    /// Read the global behavior config.
    pub async fn get_behavior(&mut self) -> Result<BehaviorConfig, RequestError> {
        self.request::<command::GetBehaviorConfig>(&()).await
    }

    /// Write the global behavior config.
    pub async fn set_behavior(&mut self, config: BehaviorConfig) -> Result<(), RequestError> {
        self.request::<command::SetBehaviorConfig>(&config).await
    }

    // ── status ──

    /// Read the currently active layer.
    pub async fn get_current_layer(&mut self) -> Result<u8, RequestError> {
        self.request::<command::GetCurrentLayer>(&()).await
    }

    /// Read the matrix scan bitmap.
    pub async fn get_matrix_state(&mut self) -> Result<MatrixState, RequestError> {
        self.request::<command::GetMatrixState>(&()).await
    }

    /// Read battery status. BLE firmware only ([`DeviceCapabilities::ble_enabled`]);
    /// returns [`RequestError::Unsupported`] otherwise, without touching the wire.
    pub async fn get_battery_status(&mut self) -> Result<BatteryStatus, RequestError> {
        self.require_ble(Cmd::GetBatteryStatus)?;
        self.request::<command::GetBatteryStatus>(&()).await
    }

    /// Read one split peripheral's status by slot. Split BLE keyboards only
    /// ([`DeviceCapabilities::is_split`] and `ble_enabled`); returns
    /// [`RequestError::Unsupported`] otherwise, without touching the wire.
    pub async fn get_peripheral_status(&mut self, slot: u8) -> Result<PeripheralStatus, RequestError> {
        if !(self.capabilities().is_split && self.capabilities().ble_enabled) {
            return Err(RequestError::Unsupported(
                Cmd::GetPeripheralStatus,
                "not a split BLE keyboard",
            ));
        }
        self.request::<command::GetPeripheralStatus>(&slot).await
    }

    /// Read the current words-per-minute estimate.
    pub async fn get_wpm(&mut self) -> Result<u16, RequestError> {
        self.request::<command::GetWpm>(&()).await
    }

    /// Read the firmware's sleep state.
    pub async fn get_sleep_state(&mut self) -> Result<bool, RequestError> {
        self.request::<command::GetSleepState>(&()).await
    }

    /// Read the host LED indicator state (caps/num/scroll lock, etc.).
    pub async fn get_led_indicator(&mut self) -> Result<LedIndicator, RequestError> {
        self.request::<command::GetLedIndicator>(&()).await
    }

    // ── connection ──

    /// Read the active connection type (USB / BLE).
    pub async fn get_connection_type(&mut self) -> Result<ConnectionType, RequestError> {
        self.request::<command::GetConnectionType>(&()).await
    }

    /// Read the full connection status — the same payload the `ConnectionChange`
    /// topic pushes, for recovering a missed push.
    pub async fn get_connection_status(&mut self) -> Result<ConnectionStatus, RequestError> {
        self.request::<command::GetConnectionStatus>(&()).await
    }

    /// Read BLE status (active profile, connection state). BLE firmware only
    /// ([`DeviceCapabilities::ble_enabled`]); returns [`RequestError::Unsupported`]
    /// otherwise, without touching the wire.
    pub async fn get_ble_status(&mut self) -> Result<BleStatus, RequestError> {
        self.require_ble(Cmd::GetBleStatus)?;
        self.request::<command::GetBleStatus>(&()).await
    }

    /// Switch to a BLE profile by slot. BLE firmware only; returns
    /// [`RequestError::Unsupported`] otherwise, without touching the wire.
    pub async fn switch_ble_profile(&mut self, slot: u8) -> Result<(), RequestError> {
        self.require_ble(Cmd::SwitchBleProfile)?;
        self.request::<command::SwitchBleProfile>(&slot).await
    }

    /// Clear (unbond) a BLE profile by slot. Tears down the active link if it
    /// targets the connected profile. BLE firmware only; returns
    /// [`RequestError::Unsupported`] otherwise, without touching the wire.
    pub async fn clear_ble_profile(&mut self, slot: u8) -> Result<(), RequestError> {
        self.require_ble(Cmd::ClearBleProfile)?;
        self.request::<command::ClearBleProfile>(&slot).await
    }
}
