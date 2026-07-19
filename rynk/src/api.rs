//! Typed endpoint methods — the version-specific API surface over the driver
//! core in `driver.rs`.

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use rmk_types::action::{EncoderAction, KeyAction};
use rmk_types::battery::BatteryStatus;
use rmk_types::ble::BleStatus;
use rmk_types::combo::Combo;
use rmk_types::connection::{ConnectionStatus, ConnectionType};
use rmk_types::fork::Fork;
use rmk_types::led_indicator::LedIndicator;
use rmk_types::morse::Morse;
use rmk_types::protocol::rynk::{
    BehaviorConfig, Cmd, DeviceCapabilities, DeviceInfo, GetComboBulkRequest, GetComboBulkResponse, GetEncoderRequest,
    GetKeymapBulkRequest, GetKeymapBulkResponse, GetMacroRequest, GetMorseBulkRequest, GetMorseBulkResponse,
    KeyPosition, LockStatus, MacroData, MatrixState, PeripheralStatus, ProtocolVersion, SetComboBulkRequest,
    SetComboRequest, SetEncoderRequest, SetForkRequest, SetKeyRequest, SetKeymapBulkRequest, SetMacroRequest,
    SetMorseBulkRequest, SetMorseRequest, StorageResetMode, command,
};

use crate::driver::{Client, RynkHostError};
#[cfg(feature = "alloc")]
use crate::layout::LayoutInfo;

impl Client {
    /// Reject a bulk command locally when capabilities lack bulk transfer.
    fn require_bulk_transfer(&self, cmd: Cmd) -> Result<(), RynkHostError> {
        if self.capabilities.bulk_transfer_supported {
            Ok(())
        } else {
            Err(RynkHostError::Unsupported(cmd, "bulk transfer not supported"))
        }
    }

    /// Reject a BLE-only command locally when capabilities lack BLE.
    fn require_ble(&self, cmd: Cmd) -> Result<(), RynkHostError> {
        if self.capabilities.ble_enabled {
            Ok(())
        } else {
            Err(RynkHostError::Unsupported(cmd, "BLE not enabled"))
        }
    }

    /// Read the firmware's protocol version.
    pub async fn get_version(&self) -> Result<ProtocolVersion, RynkHostError> {
        self.request::<command::GetVersion>(&()).await
    }

    /// Read the firmware's capability set.
    pub async fn get_capabilities(&self) -> Result<DeviceCapabilities, RynkHostError> {
        self.request::<command::GetCapabilities>(&()).await
    }

    /// Read the firmware and device identity.
    pub async fn get_device_info(&self) -> Result<DeviceInfo, RynkHostError> {
        self.request::<command::GetDeviceInfo>(&()).await
    }

    /// Reboot the device — fire-and-forget: the firmware resets before its
    /// session loop can reply, so `Ok(())` only means the request frame was
    /// queued; keep the driver running until it drains.
    pub async fn reboot(&self) -> Result<(), RynkHostError> {
        self.send_no_reply::<command::Reboot>(&()).await
    }

    /// Jump to the bootloader (DFU mode) — fire-and-forget, same contract as
    /// [`reboot`](Self::reboot).
    pub async fn bootloader_jump(&self) -> Result<(), RynkHostError> {
        self.send_no_reply::<command::BootloaderJump>(&()).await
    }

    /// Reset persistent storage. Rejected locally when storage is disabled
    /// ([`DeviceCapabilities::storage_enabled`]), where the wipe would be a silent
    /// no-op.
    pub async fn storage_reset(&self, mode: StorageResetMode) -> Result<(), RynkHostError> {
        if !self.capabilities.storage_enabled {
            return Err(RynkHostError::Unsupported(Cmd::StorageReset, "storage not enabled"));
        }
        self.request::<command::StorageReset>(&mode).await
    }

    /// Read the current lock state without side effects.
    ///
    /// [`LockStatus::key_positions`] is the challenge to hold; empty while
    /// [`locked`](LockStatus::locked) means the device is permanently locked
    /// (no `unlock_keys` configured in keyboard.toml).
    pub async fn get_lock_status(&self) -> Result<LockStatus, RynkHostError> {
        self.request::<command::GetLockStatus>(&()).await
    }

    /// Arm/refresh a physical-presence unlock attempt and sample the held
    /// challenge keys.
    ///
    /// Poll every ~150 ms while the user holds the challenge keys:
    /// [`remaining_keys`](LockStatus::remaining_keys) counts down, and the
    /// attempt succeeds ([`locked`](LockStatus::locked) `== false`) once all
    /// are held simultaneously. The firmware window lapses ~500 ms after polls
    /// stop, so a cancel is just "stop polling".
    pub async fn unlock_poll(&self) -> Result<LockStatus, RynkHostError> {
        self.request::<command::UnlockPoll>(&()).await
    }

    /// Relock immediately. A no-op on an `insecure` device.
    pub async fn lock(&self) -> Result<(), RynkHostError> {
        self.request::<command::Lock>(&()).await
    }

    /// Read one key's action.
    pub async fn get_key(&self, layer: u8, row: u8, col: u8) -> Result<KeyAction, RynkHostError> {
        self.request::<command::GetKeyAction>(&KeyPosition { layer, row, col })
            .await
    }

    /// Write one key's action.
    pub async fn set_key(&self, layer: u8, row: u8, col: u8, action: KeyAction) -> Result<(), RynkHostError> {
        let req = SetKeyRequest {
            position: KeyPosition { layer, row, col },
            action,
        };
        self.request::<command::SetKeyAction>(&req).await
    }

    /// Read the currently selected default layer index.
    pub async fn get_default_layer(&self) -> Result<u8, RynkHostError> {
        self.request::<command::GetDefaultLayer>(&()).await
    }

    /// Set the default layer.
    pub async fn set_default_layer(&self, layer: u8) -> Result<(), RynkHostError> {
        self.request::<command::SetDefaultLayer>(&layer).await
    }

    /// Read both rotation actions for one encoder on one layer.
    pub async fn get_encoder(&self, encoder_id: u8, layer: u8) -> Result<EncoderAction, RynkHostError> {
        self.request::<command::GetEncoderAction>(&GetEncoderRequest { encoder_id, layer })
            .await
    }

    /// Set both rotation actions for one encoder on one layer.
    pub async fn set_encoder(&self, encoder_id: u8, layer: u8, action: EncoderAction) -> Result<(), RynkHostError> {
        let req = SetEncoderRequest {
            encoder_id,
            layer,
            action,
        };
        self.request::<command::SetEncoderAction>(&req).await
    }

    /// Read one page of key actions starting at `(layer, start_row, start_col)`,
    /// walking the row-major, layer-major keymap: up to `max_bulk_keys`, fewer at
    /// the end. An out-of-geometry position is rejected with `RynkError::Invalid`.
    /// Bulk firmware only ([`DeviceCapabilities::bulk_transfer_supported`]);
    /// rejected locally otherwise.
    pub async fn get_keymap_bulk(
        &self,
        layer: u8,
        start_row: u8,
        start_col: u8,
    ) -> Result<GetKeymapBulkResponse, RynkHostError> {
        self.require_bulk_transfer(Cmd::GetKeymapBulk)?;
        self.request::<command::GetKeymapBulk>(&GetKeymapBulkRequest {
            layer,
            start_row,
            start_col,
        })
        .await
    }

    /// Write `request.actions` into the keymap starting at
    /// `(request.layer, request.start_row, request.start_col)`. Bulk firmware only
    /// ([`DeviceCapabilities::bulk_transfer_supported`]); rejected locally otherwise.
    pub async fn set_keymap_bulk(&self, request: SetKeymapBulkRequest) -> Result<(), RynkHostError> {
        self.require_bulk_transfer(Cmd::SetKeymapBulk)?;
        self.request::<command::SetKeymapBulk>(&request).await
    }

    /// Read the physical layout. The firmware serves it as an opaque,
    /// compressed blob paged over `GetLayout`; this reassembles every page (by
    /// byte offset), inflates the blob, and decodes it into [`LayoutInfo`]. An
    /// empty blob (firmware built without a `[layout].map`) yields an empty
    /// [`LayoutInfo`], not an error.
    #[cfg(feature = "alloc")]
    pub async fn get_layout(&self) -> Result<LayoutInfo, RynkHostError> {
        const MAX_LAYOUT_BLOB_LEN: usize = 64 * 1024;
        let first = self.request::<command::GetLayout>(&0u32).await?;
        let total_len = first.total_len as usize;
        if total_len > MAX_LAYOUT_BLOB_LEN {
            return Err(RynkHostError::Layout(alloc::format!(
                "advertised layout blob length {total_len} exceeds maximum {MAX_LAYOUT_BLOB_LEN}"
            )));
        }
        let mut collected: Vec<u8> = first.bytes.to_vec();
        // The advertised length bounds repeated or over-long pages; an empty page ends a stalled transfer.
        while !collected.is_empty() && collected.len() < total_len {
            let chunk = self.request::<command::GetLayout>(&(collected.len() as u32)).await?;
            if chunk.bytes.is_empty() {
                break;
            }
            collected.extend_from_slice(&chunk.bytes);
        }
        collected.truncate(total_len);
        LayoutInfo::from_compressed_blob(&collected).map_err(RynkHostError::Layout)
    }

    /// Read one combo entry by index.
    pub async fn get_combo(&self, index: u8) -> Result<Combo, RynkHostError> {
        self.request::<command::GetCombo>(&index).await
    }

    /// Write one combo entry by index.
    pub async fn set_combo(&self, index: u8, config: Combo) -> Result<(), RynkHostError> {
        self.request::<command::SetCombo>(&SetComboRequest { index, config })
            .await
    }

    /// Read one page of combos starting at slot `start_index`: up to
    /// `max_bulk_configs`, fewer at the end, empty once `start_index` reaches the
    /// slot count. Bulk firmware only
    /// ([`DeviceCapabilities::bulk_transfer_supported`]); rejected locally otherwise.
    pub async fn get_combo_bulk(&self, start_index: u8) -> Result<GetComboBulkResponse, RynkHostError> {
        self.require_bulk_transfer(Cmd::GetComboBulk)?;
        self.request::<command::GetComboBulk>(&GetComboBulkRequest { start_index })
            .await
    }

    /// Write `request.configs` into the combo table at slot `request.start_index`.
    /// Bulk firmware only ([`DeviceCapabilities::bulk_transfer_supported`]);
    /// rejected locally otherwise.
    pub async fn set_combo_bulk(&self, request: SetComboBulkRequest) -> Result<(), RynkHostError> {
        self.require_bulk_transfer(Cmd::SetComboBulk)?;
        self.request::<command::SetComboBulk>(&request).await
    }

    /// Read one fork entry by index.
    pub async fn get_fork(&self, index: u8) -> Result<Fork, RynkHostError> {
        self.request::<command::GetFork>(&index).await
    }

    /// Write one fork entry by index.
    pub async fn set_fork(&self, index: u8, config: Fork) -> Result<(), RynkHostError> {
        self.request::<command::SetFork>(&SetForkRequest { index, config })
            .await
    }

    /// Read one morse entry by index.
    pub async fn get_morse(&self, index: u8) -> Result<Morse, RynkHostError> {
        self.request::<command::GetMorse>(&index).await
    }

    /// Write one morse entry by index.
    pub async fn set_morse(&self, index: u8, config: Morse) -> Result<(), RynkHostError> {
        self.request::<command::SetMorse>(&SetMorseRequest { index, config })
            .await
    }

    /// Read one page of morses starting at slot `start_index`: up to
    /// `max_bulk_configs`, fewer at the end, empty once `start_index` reaches the
    /// slot count. Bulk firmware only
    /// ([`DeviceCapabilities::bulk_transfer_supported`]); rejected locally otherwise.
    pub async fn get_morse_bulk(&self, start_index: u8) -> Result<GetMorseBulkResponse, RynkHostError> {
        self.require_bulk_transfer(Cmd::GetMorseBulk)?;
        self.request::<command::GetMorseBulk>(&GetMorseBulkRequest { start_index })
            .await
    }

    /// Write `request.configs` into the morse table at slot `request.start_index`.
    /// Bulk firmware only ([`DeviceCapabilities::bulk_transfer_supported`]);
    /// rejected locally otherwise.
    pub async fn set_morse_bulk(&self, request: SetMorseBulkRequest) -> Result<(), RynkHostError> {
        self.require_bulk_transfer(Cmd::SetMorseBulk)?;
        self.request::<command::SetMorseBulk>(&request).await
    }

    /// Read one chunk of macro data at byte `offset`. The reply is always a
    /// full build-time chunk, zero-filled past the end of macro space —
    /// termination comes from parsing the macro encoding, not chunk length.
    pub async fn get_macro(&self, offset: u16) -> Result<MacroData, RynkHostError> {
        self.request::<command::GetMacro>(&GetMacroRequest { offset }).await
    }

    /// Write one chunk of macro data starting at byte `offset`. Writes past
    /// the end of the device's macro space are truncated by the firmware.
    pub async fn set_macro(&self, offset: u16, data: MacroData) -> Result<(), RynkHostError> {
        self.request::<command::SetMacro>(&SetMacroRequest { offset, data })
            .await
    }

    /// Read the global behavior config.
    pub async fn get_behavior(&self) -> Result<BehaviorConfig, RynkHostError> {
        self.request::<command::GetBehaviorConfig>(&()).await
    }

    /// Write the global behavior config.
    pub async fn set_behavior(&self, config: BehaviorConfig) -> Result<(), RynkHostError> {
        self.request::<command::SetBehaviorConfig>(&config).await
    }

    /// Read the currently active layer.
    pub async fn get_current_layer(&self) -> Result<u8, RynkHostError> {
        self.request::<command::GetCurrentLayer>(&()).await
    }

    /// Read the matrix scan bitmap.
    pub async fn get_matrix_state(&self) -> Result<MatrixState, RynkHostError> {
        self.request::<command::GetMatrixState>(&()).await
    }

    /// Read battery status. BLE firmware only ([`DeviceCapabilities::ble_enabled`]);
    /// rejected locally otherwise.
    pub async fn get_battery_status(&self) -> Result<BatteryStatus, RynkHostError> {
        self.require_ble(Cmd::GetBatteryStatus)?;
        self.request::<command::GetBatteryStatus>(&()).await
    }

    /// Read one split peripheral's status by slot. Split keyboards only
    /// ([`DeviceCapabilities::is_split`]); rejected locally otherwise.
    pub async fn get_peripheral_status(&self, slot: u8) -> Result<PeripheralStatus, RynkHostError> {
        if !self.capabilities.is_split {
            return Err(RynkHostError::Unsupported(
                Cmd::GetPeripheralStatus,
                "not a split keyboard",
            ));
        }
        self.request::<command::GetPeripheralStatus>(&slot).await
    }

    /// Read the current words-per-minute estimate.
    pub async fn get_wpm(&self) -> Result<u16, RynkHostError> {
        self.request::<command::GetWpm>(&()).await
    }

    /// Read the firmware's sleep state.
    pub async fn get_sleep_state(&self) -> Result<bool, RynkHostError> {
        self.request::<command::GetSleepState>(&()).await
    }

    /// Read the host LED indicator state (caps/num/scroll lock, etc.).
    pub async fn get_led_indicator(&self) -> Result<LedIndicator, RynkHostError> {
        self.request::<command::GetLedIndicator>(&()).await
    }

    /// Read the active connection type (USB / BLE).
    pub async fn get_connection_type(&self) -> Result<ConnectionType, RynkHostError> {
        self.request::<command::GetConnectionType>(&()).await
    }

    /// Read the full connection status — the same payload the `ConnectionChange`
    /// topic pushes, for recovering a missed push.
    pub async fn get_connection_status(&self) -> Result<ConnectionStatus, RynkHostError> {
        self.request::<command::GetConnectionStatus>(&()).await
    }

    /// Read BLE status (active profile, connection state). BLE firmware only
    /// ([`DeviceCapabilities::ble_enabled`]); rejected locally otherwise.
    pub async fn get_ble_status(&self) -> Result<BleStatus, RynkHostError> {
        self.require_ble(Cmd::GetBleStatus)?;
        self.request::<command::GetBleStatus>(&()).await
    }

    /// Switch to a BLE profile by slot. BLE firmware only; rejected locally
    /// otherwise.
    pub async fn switch_ble_profile(&self, slot: u8) -> Result<(), RynkHostError> {
        self.require_ble(Cmd::SwitchBleProfile)?;
        self.request::<command::SwitchBleProfile>(&slot).await
    }

    /// Clear (unbond) a BLE profile by slot. Tears down the active link if it
    /// targets the connected profile. BLE firmware only; rejected locally
    /// otherwise.
    pub async fn clear_ble_profile(&self, slot: u8) -> Result<(), RynkHostError> {
        self.require_ble(Cmd::ClearBleProfile)?;
        self.request::<command::ClearBleProfile>(&slot).await
    }
}

#[cfg(feature = "alloc")]
impl Client {
    /// Read the whole keymap (every layer, row-major) by paging `GetKeymapBulk`.
    pub async fn read_all_keymap(&self) -> Result<Vec<KeyAction>, RynkHostError> {
        let caps = self.capabilities;
        let (rows, cols) = (caps.num_rows as u16, caps.num_cols as u16);
        let total = caps.num_layers as usize * rows as usize * cols as usize;
        self.read_all(total, async |c, start| {
            let (layer, row, col) = keymap_pos(start, rows, cols);
            c.get_keymap_bulk(layer, row, col).await.map(|r| r.actions)
        })
        .await
    }

    /// Read every combo slot by paging `GetComboBulk`.
    pub async fn read_all_combos(&self) -> Result<Vec<Combo>, RynkHostError> {
        let total = self.capabilities.max_combos as usize;
        self.read_all(total, async |c, start| {
            c.get_combo_bulk(start as u8).await.map(|r| r.configs)
        })
        .await
    }

    /// Read every morse slot by paging `GetMorseBulk`.
    pub async fn read_all_morses(&self) -> Result<Vec<Morse>, RynkHostError> {
        let total = self.capabilities.max_morse as usize;
        self.read_all(total, async |c, start| {
            c.get_morse_bulk(start as u8).await.map(|r| r.configs)
        })
        .await
    }

    /// Write the whole keymap by paging `SetKeymapBulk` in `max_bulk_keys` chunks.
    pub async fn write_all_keymap(&self, actions: &[KeyAction]) -> Result<(), RynkHostError> {
        let caps = self.capabilities;
        let (rows, cols) = (caps.num_rows as u16, caps.num_cols as u16);
        let page = caps.max_bulk_keys as usize;
        self.write_all(page, actions, async |c, start, actions| {
            let (layer, row, col) = keymap_pos(start, rows, cols);
            c.set_keymap_bulk(SetKeymapBulkRequest {
                layer,
                start_row: row,
                start_col: col,
                actions,
            })
            .await
        })
        .await
    }

    /// Write every combo by paging `SetComboBulk` in `max_bulk_configs` chunks.
    pub async fn write_all_combos(&self, configs: &[Combo]) -> Result<(), RynkHostError> {
        let page = self.capabilities.max_bulk_configs as usize;
        self.write_all(page, configs, async |c, start, configs| {
            c.set_combo_bulk(SetComboBulkRequest {
                start_index: start as u8,
                configs,
            })
            .await
        })
        .await
    }

    /// Write every morse by paging `SetMorseBulk` in `max_bulk_configs` chunks.
    pub async fn write_all_morses(&self, configs: &[Morse]) -> Result<(), RynkHostError> {
        let page = self.capabilities.max_bulk_configs as usize;
        self.write_all(page, configs, async |c, start, configs| {
            c.set_morse_bulk(SetMorseBulkRequest {
                start_index: start as u8,
                configs,
            })
            .await
        })
        .await
    }

    /// Page a whole resource in: fetch from cursor 0 until `total` items are read
    /// or an empty page marks the firmware's clamped end.
    async fn read_all<Item>(
        &self,
        total: usize,
        mut fetch: impl AsyncFnMut(&Self, u16) -> Result<Vec<Item>, RynkHostError>,
    ) -> Result<Vec<Item>, RynkHostError> {
        let mut out = Vec::new();
        let mut start: u16 = 0;
        while (start as usize) < total {
            let page = fetch(self, start).await?;
            if page.is_empty() {
                break; // firmware paged out before reaching `total`
            }
            start += page.len() as u16;
            out.extend(page);
        }
        Ok(out)
    }

    /// Page a whole resource out: send `items` in `page`-sized chunks, each a
    /// bounded `Set*Bulk` at its flat cursor.
    async fn write_all<Item: Clone>(
        &self,
        page: usize,
        items: &[Item],
        mut store: impl AsyncFnMut(&Self, u16, Vec<Item>) -> Result<(), RynkHostError>,
    ) -> Result<(), RynkHostError> {
        // Preserve the capability error path instead of panicking in `chunks(0)`.
        let mut start: u16 = 0;
        for chunk in items.chunks(page.max(1)) {
            store(self, start, chunk.to_vec()).await?;
            start += chunk.len() as u16;
        }
        Ok(())
    }
}

/// Map a flat, row-major, layer-major key cursor to its `(layer, row, col)`
/// address for the device's `rows`×`cols` geometry. `u16` arithmetic since the
/// keymap can exceed 255 keys; the address components each fit in `u8`.
#[cfg(feature = "alloc")]
fn keymap_pos(cursor: u16, rows: u16, cols: u16) -> (u8, u8, u8) {
    let layer = cursor / (rows * cols);
    let row = (cursor / cols) % rows;
    let col = cursor % cols;
    (layer as u8, row as u8, col as u8)
}
