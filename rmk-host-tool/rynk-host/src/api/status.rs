//! Runtime status endpoints — current layer, matrix bitmap, battery, peripherals,
//! plus topic-snapshot getters for WPM / sleep / LED.

use rmk_types::battery::BatteryStatus;
use rmk_types::led_indicator::LedIndicator;
use rmk_types::protocol::rynk::{Cmd, MatrixState, PeripheralStatus};

use crate::RynkResult;
use crate::transport::{Transport, TransportError};

pub async fn get_current_layer<T: Transport>(t: &mut T) -> Result<RynkResult<u8>, TransportError> {
    t.request::<(), RynkResult<u8>>(Cmd::GetCurrentLayer, &()).await
}

pub async fn get_matrix_state<T: Transport>(t: &mut T) -> Result<RynkResult<MatrixState>, TransportError> {
    t.request::<(), RynkResult<MatrixState>>(Cmd::GetMatrixState, &()).await
}

pub async fn get_battery_status<T: Transport>(t: &mut T) -> Result<RynkResult<BatteryStatus>, TransportError> {
    t.request::<(), RynkResult<BatteryStatus>>(Cmd::GetBatteryStatus, &())
        .await
}

pub async fn get_peripheral_status<T: Transport>(
    t: &mut T,
    slot: u8,
) -> Result<RynkResult<PeripheralStatus>, TransportError> {
    t.request::<u8, RynkResult<PeripheralStatus>>(Cmd::GetPeripheralStatus, &slot)
        .await
}

pub async fn get_wpm<T: Transport>(t: &mut T) -> Result<RynkResult<u16>, TransportError> {
    t.request::<(), RynkResult<u16>>(Cmd::GetWpm, &()).await
}

pub async fn get_sleep_state<T: Transport>(t: &mut T) -> Result<RynkResult<bool>, TransportError> {
    t.request::<(), RynkResult<bool>>(Cmd::GetSleepState, &()).await
}

pub async fn get_led_indicator<T: Transport>(t: &mut T) -> Result<RynkResult<LedIndicator>, TransportError> {
    t.request::<(), RynkResult<LedIndicator>>(Cmd::GetLedIndicator, &())
        .await
}
