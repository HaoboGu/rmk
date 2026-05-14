//! Connection handlers — USB/BLE preferred-transport read, BLE profile management.

use rmk_types::protocol::rynk::RynkError;

use super::super::RynkService;

impl<'a> RynkService<'a> {
    pub(crate) async fn handle_get_connection_type(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let t = self.ctx.preferred_connection();
        Self::write_response(&t, payload)
    }

    #[cfg(feature = "_ble")]
    pub(crate) async fn handle_get_ble_status(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let status = self.ctx.connection_status().ble;
        Self::write_response(&status, payload)
    }

    /// `Cmd::SwitchBleProfile` — payload is the slot index. `try_send` is
    /// used so a host hammering this Cmd while the previous switch is still
    /// running observes the queue-full error rather than blocking the
    /// dispatch loop.
    #[cfg(feature = "_ble")]
    pub(crate) async fn handle_switch_ble_profile(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (slot, _) = postcard::take_from_bytes::<u8>(payload).map_err(|_| RynkError::InvalidRequest)?;
        if (slot as usize) >= crate::NUM_BLE_PROFILE {
            return Err(RynkError::InvalidRequest);
        }
        crate::channel::BLE_PROFILE_CHANNEL
            .try_send(crate::ble::profile::BleProfileAction::Switch(slot))
            .map_err(|_| RynkError::NotReady)?;
        Self::write_response(&(), payload)
    }

    /// `Cmd::ClearBleProfile` — wipes the bond at the given slot without
    /// requiring a prior switch (uses [`BleProfileAction::ClearSlot`]).
    #[cfg(feature = "_ble")]
    pub(crate) async fn handle_clear_ble_profile(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (slot, _) = postcard::take_from_bytes::<u8>(payload).map_err(|_| RynkError::InvalidRequest)?;
        if (slot as usize) >= crate::NUM_BLE_PROFILE {
            return Err(RynkError::InvalidRequest);
        }
        crate::channel::BLE_PROFILE_CHANNEL
            .try_send(crate::ble::profile::BleProfileAction::ClearSlot(slot))
            .map_err(|_| RynkError::NotReady)?;
        Self::write_response(&(), payload)
    }
}
