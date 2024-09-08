use defmt::error;
use nrf_softdevice::ble::{gatt_server, Connection};

use crate::split::{
    driver::{SplitDriverError, SplitReader, SplitWriter},
    SplitMessage, SPLIT_MESSAGE_MAX_SIZE,
};

/// Gatt service used in split slave to send split message to master
#[nrf_softdevice::gatt_service(uuid = "4dd5fbaa-18e5-4b07-bf0a-353698659946")]
pub(crate) struct SplitBleService {
    #[characteristic(uuid = "0e6313e3-bd0b-45c2-8d2e-37a2e8128bc3", read, notify)]
    pub(crate) message_to_central: [u8; SPLIT_MESSAGE_MAX_SIZE],

    #[characteristic(uuid = "4b3514fb-cae4-4d38-a097-3a2a3d1c3b9c", write_without_response)]
    pub(crate) message_to_peripheral: [u8; SPLIT_MESSAGE_MAX_SIZE],
}

/// Gatt server in split slave
#[nrf_softdevice::gatt_server]
pub(crate) struct BleSplitSlaveServer {
    pub(crate) service: SplitBleService,
}

/// BLE driver for split slave
pub(crate) struct BleSplitSlaveDriver<'a> {
    server: &'a BleSplitSlaveServer,
    conn: &'a Connection,
}

impl<'a> BleSplitSlaveDriver<'a> {
    pub(crate) fn new(server: &'a BleSplitSlaveServer, conn: &'a Connection) -> Self {
        Self { server, conn }
    }
}

impl<'a> SplitReader for BleSplitSlaveDriver<'a> {
    async fn read(&mut self) -> Result<SplitMessage, SplitDriverError> {
        let message = self
            .server
            .service
            .message_to_peripheral_get()
            .map_err(|e| {
                error!("BLE read error: {:?}", e);
                SplitDriverError::BleError(1)
            })?;
        let message: SplitMessage = postcard::from_bytes(&message).map_err(|e| {
            error!("Postcard deserialize split message error: {}", e);
            SplitDriverError::DeserializeError
        })?;
        Ok(message)
    }
}

impl<'a> SplitWriter for BleSplitSlaveDriver<'a> {
    async fn write(&mut self, message: &SplitMessage) -> Result<usize, SplitDriverError> {
        let mut buf = [0_u8; SPLIT_MESSAGE_MAX_SIZE];
        let bytes = postcard::to_slice(message, &mut buf).map_err(|e| {
            error!("Postcard serialize split message error: {}", e);
            SplitDriverError::SerializeError
        })?;
        gatt_server::notify_value(
            &self.conn,
            self.server.service.message_to_central_value_handle,
            bytes,
        )
        .map_err(|e| {
            error!("BLE notify error: {:?}", e);
            SplitDriverError::BleError(1)
        })?;
        Ok(bytes.len())
    }
}
