use defmt::error;
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver, Sender},
};
use nrf_softdevice::ble::{central, gatt_client, gatt_server, Address, AddressType, Connection};

use crate::split::{SplitMessage, SPLIT_MESSAGE_MAX_SIZE};

use super::{SplitDriverError, SplitReader, SplitWriter};

#[nrf_softdevice::gatt_client(uuid = "4dd5fbaa-18e5-4b07-bf0a-353698659946")]
pub(crate) struct SplitBleClient {
    #[characteristic(uuid = "0e6313e3-bd0b-45c2-8d2e-37a2e8128bc3", read, notify)]
    pub(crate) message_to_central: [u8; SPLIT_MESSAGE_MAX_SIZE],

    #[characteristic(uuid = "4b3514fb-cae4-4d38-a097-3a2a3d1c3b9c", write)]
    pub(crate) message_to_peripheral: [u8; SPLIT_MESSAGE_MAX_SIZE],
}

#[nrf_softdevice::gatt_service(uuid = "4dd5fbaa-18e5-4b07-bf0a-353698659946")]
pub(crate) struct SplitBleService {
    #[characteristic(uuid = "0e6313e3-bd0b-45c2-8d2e-37a2e8128bc3", read, notify)]
    pub(crate) message_to_central: [u8; SPLIT_MESSAGE_MAX_SIZE],

    #[characteristic(uuid = "4b3514fb-cae4-4d38-a097-3a2a3d1c3b9c", write_without_response)]
    pub(crate) message_to_peripheral: [u8; SPLIT_MESSAGE_MAX_SIZE],
}

#[nrf_softdevice::gatt_server]
pub struct SplitBleServer {
    service: SplitBleService,
}

// Used in BLE master
pub(crate) static BLE_READER_CHANNEL: Channel<CriticalSectionRawMutex, SplitMessage, 8> =
    Channel::new();

/// Run a single ble client, receive notification from the ble peripheral and send it to the CHANNEL
pub(crate) async fn run_ble_client(
    sender: Sender<'_, CriticalSectionRawMutex, SplitMessage, 8>,
    addr: [u8; 6],
) -> ! {
    // Wait 1s, ensure that the softdevice is ready
    embassy_time::Timer::after_secs(1).await;
    let sd = unsafe { nrf_softdevice::Softdevice::steal() };
    loop {
        let addrs = &[&Address::new(AddressType::RandomStatic, addr)];
        let mut config: central::ConnectConfig<'_> = central::ConnectConfig::default();
        config.scan_config.whitelist = Some(addrs);
        let conn = match central::connect(sd, &config).await {
            Ok(conn) => conn,
            Err(e) => {
                error!("BLE peripheral connect error: {}", e);
                continue;
            }
        };

        let ble_client: SplitBleClient = match gatt_client::discover(&conn).await {
            Ok(client) => client,
            Err(e) => {
                error!("BLE discover error: {}", e);
                continue;
            }
        };

        // Enable notifications from the peripherals
        if let Err(e) = ble_client.message_to_central_cccd_write(true).await {
            error!("BLE message_to_central_cccd_write error: {}", e);
            continue;
        }

        // Receive slave's notifications
        let disconnect_error = gatt_client::run(&conn, &ble_client, |event| match event {
            SplitBleClientEvent::MessageToCentralNotification(message) => {
                match postcard::from_bytes(&message) {
                    Ok(split_message) => {
                        // if let Err(e) = BLE_READER_CHANNEL.try_send(split_message) {
                        if let Err(e) = sender.try_send(split_message) {
                            error!("BLE_SYNC_CHANNEL send message error: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Postcard deserialize split message error: {}", e);
                    }
                };
            }
        })
        .await;

        error!("BLE peripheral disconnect error: {:?}", disconnect_error);
        // Wait for 1s before trying to connect (again)
        embassy_time::Timer::after_secs(1).await;
    }
}

pub(crate) struct SplitBleMasterDriver<'a> {
    pub(crate) receiver: Receiver<'a, CriticalSectionRawMutex, SplitMessage, 8>,
}

impl<'a> SplitReader for SplitBleMasterDriver<'a> {
    async fn read(&mut self) -> Result<SplitMessage, SplitDriverError> {
        Ok(self.receiver.receive().await)
        // Ok(BLE_READER_CHANNEL.receive().await)
    }
}

pub(crate) struct SplitBleDriver<'a> {
    server: &'a SplitBleServer,
    conn: &'a Connection,
}

impl<'a> SplitBleDriver<'a> {
    pub(crate) fn new(server: &'a SplitBleServer, conn: &'a Connection) -> Self {
        Self { server, conn }
    }
}

impl<'a> SplitReader for SplitBleDriver<'a> {
    async fn read(&mut self) -> Result<SplitMessage, SplitDriverError> {
        let message = self
            .server
            .service
            .message_to_peripheral_get()
            .map_err(|e| {
                error!("BLE read error: {:?}", e);
                super::SplitDriverError::BleError(1)
            })?;
        let message: SplitMessage = postcard::from_bytes(&message).map_err(|e| {
            error!("Postcard deserialize split message error: {}", e);
            super::SplitDriverError::DeserializeError
        })?;
        Ok(message)
    }
}

impl<'a> SplitWriter for SplitBleDriver<'a> {
    async fn write(&mut self, message: &SplitMessage) -> Result<usize, SplitDriverError> {
        let mut buf = [0_u8; SPLIT_MESSAGE_MAX_SIZE];
        let bytes = postcard::to_slice(message, &mut buf).map_err(|e| {
            error!("Postcard serialize split message error: {}", e);
            super::SplitDriverError::SerializeError
        })?;
        gatt_server::notify_value(
            &self.conn,
            self.server.service.message_to_central_value_handle,
            bytes,
        )
        .map_err(|e| {
            error!("BLE notify error: {:?}", e);
            super::SplitDriverError::BleError(1)
        })?;
        Ok(bytes.len())
    }
}
