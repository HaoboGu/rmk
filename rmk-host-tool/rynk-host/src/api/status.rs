//! Runtime status endpoints — current layer, matrix bitmap, battery, peripherals.

use rmk_types::battery::BatteryStatus;
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
