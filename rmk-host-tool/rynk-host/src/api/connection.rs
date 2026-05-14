//! Connection endpoints — preferred transport, BLE status, BLE profile mgmt.

use rmk_types::ble::BleStatus;
use rmk_types::connection::ConnectionType;
use rmk_types::protocol::rynk::Cmd;

use crate::RynkResult;
use crate::transport::{Transport, TransportError};

pub async fn get_connection_type<T: Transport>(t: &mut T) -> Result<RynkResult<ConnectionType>, TransportError> {
    t.request::<(), RynkResult<ConnectionType>>(Cmd::GetConnectionType, &())
        .await
}

pub async fn get_ble_status<T: Transport>(t: &mut T) -> Result<RynkResult<BleStatus>, TransportError> {
    t.request::<(), RynkResult<BleStatus>>(Cmd::GetBleStatus, &()).await
}

pub async fn switch_ble_profile<T: Transport>(t: &mut T, slot: u8) -> Result<RynkResult, TransportError> {
    t.request::<u8, RynkResult>(Cmd::SwitchBleProfile, &slot).await
}

pub async fn clear_ble_profile<T: Transport>(t: &mut T, slot: u8) -> Result<RynkResult, TransportError> {
    t.request::<u8, RynkResult>(Cmd::ClearBleProfile, &slot).await
}
