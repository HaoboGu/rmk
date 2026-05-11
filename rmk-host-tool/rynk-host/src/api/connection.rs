//! Connection endpoints — preferred transport, BLE status, BLE profile mgmt.

use rmk_types::ble::BleStatus;
use rmk_types::connection::ConnectionType;
use rmk_types::protocol::rynk::{Cmd, RynkResult};

use crate::transport::{Transport, TransportError};

pub async fn get_connection_type<T: Transport>(t: &mut T) -> Result<ConnectionType, TransportError> {
    t.request::<(), ConnectionType>(Cmd::GetConnectionType, &()).await
}

pub async fn set_connection_type<T: Transport>(t: &mut T, conn: ConnectionType) -> Result<RynkResult, TransportError> {
    t.request::<ConnectionType, RynkResult>(Cmd::SetConnectionType, &conn)
        .await
}

pub async fn get_ble_status<T: Transport>(t: &mut T) -> Result<BleStatus, TransportError> {
    t.request::<(), BleStatus>(Cmd::GetBleStatus, &()).await
}

pub async fn switch_ble_profile<T: Transport>(t: &mut T, slot: u8) -> Result<RynkResult, TransportError> {
    t.request::<u8, RynkResult>(Cmd::SwitchBleProfile, &slot).await
}

pub async fn clear_ble_profile<T: Transport>(t: &mut T, slot: u8) -> Result<RynkResult, TransportError> {
    t.request::<u8, RynkResult>(Cmd::ClearBleProfile, &slot).await
}
