//! Status handlers — current layer, matrix bitmap, battery, peripheral status,
//! plus the topic-snapshot getters for WPM / sleep / LED.

use rmk_types::protocol::rynk::{MATRIX_BITMAP_SIZE, MatrixState, RynkError};

use super::super::RynkService;

impl<'a> RynkService<'a> {
    pub(crate) async fn handle_get_current_layer(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let layer = self.ctx.active_layer();
        Self::write_response(&layer, payload)
    }

    pub(crate) async fn handle_get_matrix_state(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        // Sized for the maximum supported geometry — host slices it down
        // using num_rows / num_cols from `DeviceCapabilities`.
        let mut bitmap: heapless::Vec<u8, MATRIX_BITMAP_SIZE> = heapless::Vec::new();
        bitmap.resize_default(MATRIX_BITMAP_SIZE).expect("bitmap size matches");

        // Matrix tracking is gated on `host_security`. Without it, return
        // the zero bitmap so the wire shape stays consistent and tools
        // degrade cleanly to "no key pressed".
        #[cfg(feature = "host_security")]
        self.ctx.read_matrix_state(&mut bitmap);

        let state = MatrixState { pressed_bitmap: bitmap };
        Self::write_response(&state, payload)
    }

    #[cfg(feature = "_ble")]
    pub(crate) async fn handle_get_battery_status(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let status = self.ctx.battery_status();
        Self::write_response(&status, payload)
    }

    /// `Cmd::GetPeripheralStatus` — payload is a peripheral slot id. The
    /// snapshot is mirrored from `PeripheralConnectedEvent` /
    /// `PeripheralBatteryEvent` publishes (see
    /// [`crate::split::peripheral_state`]).
    #[cfg(all(feature = "_ble", feature = "split"))]
    pub(crate) async fn handle_get_peripheral_status(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (id, _) = postcard::take_from_bytes::<u8>(payload).map_err(|_| RynkError::InvalidRequest)?;
        let status = crate::split::peripheral_state::peripheral_status(id as usize).ok_or(RynkError::InvalidRequest)?;
        Self::write_response(&status, payload)
    }

    pub(crate) async fn handle_get_wpm(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let wpm = self.ctx.wpm();
        Self::write_response(&wpm, payload)
    }

    pub(crate) async fn handle_get_sleep_state(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let sleep = self.ctx.sleep_state();
        Self::write_response(&sleep, payload)
    }

    pub(crate) async fn handle_get_led_indicator(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let led = self.ctx.led_indicator();
        Self::write_response(&led, payload)
    }
}
