//! Connection handlers — USB/BLE preferred-transport read, BLE profile management.

use rmk_types::protocol::rynk::{RynkError, RynkMessage};

use super::super::RynkService;

impl<'a> RynkService<'a> {
    pub(crate) async fn handle_get_connection_type(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        let t = self.ctx.preferred_connection();
        Self::write_response(&t, msg.response_payload_mut())
    }

    /// `Cmd::GetConnectionStatus` — the same payload the `ConnectionChange`
    /// topic pushes, so a host can recover a missed push.
    pub(crate) async fn handle_get_connection_status(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        Self::write_response(&self.ctx.connection_status(), msg.response_payload_mut())
    }

    #[cfg(feature = "_ble")]
    pub(crate) async fn handle_get_ble_status(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        let status = self.ctx.connection_status().ble;
        Self::write_response(&status, msg.response_payload_mut())
    }

    /// `Cmd::SwitchBleProfile` — payload is the slot index. `try_send` is
    /// used so a host hammering this Cmd while the previous switch is still
    /// running observes the queue-full error rather than blocking the
    /// dispatch loop.
    #[cfg(feature = "_ble")]
    pub(crate) async fn handle_switch_ble_profile(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        let slot = msg.request::<u8>()?;
        if (slot as usize) >= crate::NUM_BLE_PROFILE {
            return Err(RynkError::Invalid);
        }
        crate::channel::BLE_PROFILE_CHANNEL
            .try_send(crate::ble::profile::BleProfileAction::Switch(slot))
            .map_err(|_| RynkError::NotReady)?;
        Self::write_response(&(), msg.response_payload_mut())
    }

    /// `Cmd::ClearBleProfile` — wipes the bond at the given slot without
    /// requiring a prior switch (uses [`BleProfileAction::ClearSlot`]).
    #[cfg(feature = "_ble")]
    pub(crate) async fn handle_clear_ble_profile(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        let slot = msg.request::<u8>()?;
        if (slot as usize) >= crate::NUM_BLE_PROFILE {
            return Err(RynkError::Invalid);
        }
        crate::channel::BLE_PROFILE_CHANNEL
            .try_send(crate::ble::profile::BleProfileAction::ClearSlot(slot))
            .map_err(|_| RynkError::NotReady)?;
        Self::write_response(&(), msg.response_payload_mut())
    }
}
