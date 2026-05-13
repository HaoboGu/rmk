//! Runtime status endpoints — current layer, matrix bitmap, battery, peripherals,
//! plus topic-snapshot getters for WPM / sleep / LED.

use rmk_types::battery::BatteryStatus;
use rmk_types::led_indicator::LedIndicator;
use rmk_types::protocol::rynk::{Cmd, MatrixState, PeripheralStatus};

use crate::transport::{Transport, TransportError};

pub async fn get_current_layer<T: Transport>(t: &mut T) -> Result<u8, TransportError> {
    t.request::<(), u8>(Cmd::GetCurrentLayer, &()).await
}

pub async fn get_matrix_state<T: Transport>(t: &mut T) -> Result<MatrixState, TransportError> {
    t.request::<(), MatrixState>(Cmd::GetMatrixState, &()).await
}

pub async fn get_battery_status<T: Transport>(t: &mut T) -> Result<BatteryStatus, TransportError> {
    t.request::<(), BatteryStatus>(Cmd::GetBatteryStatus, &()).await
}

pub async fn get_peripheral_status<T: Transport>(t: &mut T, slot: u8) -> Result<PeripheralStatus, TransportError> {
    t.request::<u8, PeripheralStatus>(Cmd::GetPeripheralStatus, &slot).await
}

pub async fn get_wpm<T: Transport>(t: &mut T) -> Result<u16, TransportError> {
    t.request::<(), u16>(Cmd::GetWpm, &()).await
}

pub async fn get_sleep_state<T: Transport>(t: &mut T) -> Result<bool, TransportError> {
    t.request::<(), bool>(Cmd::GetSleepState, &()).await
}

pub async fn get_led_indicator<T: Transport>(t: &mut T) -> Result<LedIndicator, TransportError> {
    t.request::<(), LedIndicator>(Cmd::GetLedIndicator, &()).await
}
