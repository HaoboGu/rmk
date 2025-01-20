use crate::ble::nrf::initialize_nrf_sd_and_flash;
use crate::split::driver::{SplitDriverError, SplitReader, SplitWriter};
use crate::split::peripheral::SplitPeripheral;
use crate::split::{SplitMessage, SPLIT_MESSAGE_MAX_SIZE};
use crate::MatrixTrait;
use embassy_executor::Spawner;
use embassy_futures::block_on;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::{Channel, Receiver};
use nrf_softdevice::ble::gatt_server::set_sys_attrs;
use nrf_softdevice::ble::peripheral::{advertise_connectable, ConnectableAdvertisement};
use nrf_softdevice::ble::{gatt_server, Connection, PhySet, PhyUpdateError};
use nrf_softdevice::ble::{Address, AddressType};

/// Gatt service used in split peripheral to send split message to central
#[nrf_softdevice::gatt_service(uuid = "4dd5fbaa-18e5-4b07-bf0a-353698659946")]
pub(crate) struct SplitBleService {
    #[characteristic(uuid = "0e6313e3-bd0b-45c2-8d2e-37a2e8128bc3", read, notify)]
    pub(crate) message_to_central: [u8; SPLIT_MESSAGE_MAX_SIZE],

    #[characteristic(uuid = "4b3514fb-cae4-4d38-a097-3a2a3d1c3b9c", write_without_response)]
    pub(crate) message_to_peripheral: [u8; SPLIT_MESSAGE_MAX_SIZE],
}

/// Gatt server in split peripheral
#[nrf_softdevice::gatt_server]
pub(crate) struct BleSplitPeripheralServer {
    pub(crate) service: SplitBleService,
}

/// BLE driver for split peripheral
pub(crate) struct BleSplitPeripheralDriver<'a> {
    server: &'a BleSplitPeripheralServer,
    conn: &'a Connection,
    receiver: Receiver<'a, ThreadModeRawMutex, SplitMessage, 4>,
}

impl<'a> BleSplitPeripheralDriver<'a> {
    pub(crate) fn new(
        server: &'a BleSplitPeripheralServer,
        conn: &'a Connection,
        receiver: Receiver<'a, ThreadModeRawMutex, SplitMessage, 4>,
    ) -> Self {
        Self {
            server,
            conn,
            receiver,
        }
    }
}

impl<'a> SplitReader for BleSplitPeripheralDriver<'a> {
    async fn read(&mut self) -> Result<SplitMessage, SplitDriverError> {
        Ok(self.receiver.receive().await)
    }
}

impl<'a> SplitWriter for BleSplitPeripheralDriver<'a> {
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

/// Initialize and run the nRF peripheral keyboard service via BLE.
///
/// # Arguments
///
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `spawner` - embassy task spawner, used to spawn nrf_softdevice background task
pub async fn initialize_nrf_ble_split_peripheral_and_run<
    M: MatrixTrait,
    const ROW: usize,
    const COL: usize,
>(
    mut matrix: M,
    central_addr: [u8; 6],
    peripheral_addr: [u8; 6],
    spawner: Spawner,
) -> ! {
    use embassy_futures::select::select3;
    use nrf_softdevice::ble::gatt_server;

    use crate::{
        split::nrf::peripheral::{
            BleSplitPeripheralDriver, BleSplitPeripheralServer, BleSplitPeripheralServerEvent,
            SplitBleServiceEvent,
        },
        CONNECTION_STATE,
    };

    let (sd, _) = initialize_nrf_sd_and_flash("rmk_split_peri", spawner, Some(peripheral_addr));

    let server =
        BleSplitPeripheralServer::new(sd).expect("Failed to start BLE split peripheral server");

    loop {
        CONNECTION_STATE.store(false, core::sync::atomic::Ordering::Release);
        let advertisement: ConnectableAdvertisement<'_> =
            ConnectableAdvertisement::NonscannableDirected {
                peer: Address::new(AddressType::RandomStatic, central_addr),
            };
        let mut conn = match advertise_connectable(sd, advertisement, &Default::default()).await {
            Ok(conn) => conn,
            Err(e) => {
                error!("Split peripheral advertise error: {:?}", e);
                continue;
            }
        };

        // Channel used for receiving messages from central
        let receive_channel: Channel<ThreadModeRawMutex, SplitMessage, 4> = Channel::new();
        let receiver = receive_channel.receiver();
        let sender = receive_channel.sender();

        // Set sys attr of peripheral
        set_sys_attrs(&conn, None).ok();

        // Set PHY used
        if let Err(e) = conn.phy_update(PhySet::M2, PhySet::M2) {
            error!("Failed to update PHY");
            if let PhyUpdateError::Raw(re) = e {
                error!("Raw error code: {:?}", re);
            }
        }

        let server_fut = gatt_server::run(&conn, &server, |event| match event {
            BleSplitPeripheralServerEvent::Service(split_event) => match split_event {
                SplitBleServiceEvent::MessageToCentralCccdWrite { notifications } => {
                    info!("Split value CCCD updated: {}", notifications)
                }
                SplitBleServiceEvent::MessageToPeripheralWrite(message) => {
                    match postcard::from_bytes::<SplitMessage>(&message) {
                        Ok(message) => {
                            info!("Message from central: {:?}", message);
                            // Retry 3 times
                            for _i in 0..3 {
                                if let Err(e) = sender.try_send(message) {
                                    error!("Send split message to reader error: {:?}", e);
                                    // Wait for 20ms before the next try
                                    block_on(embassy_time::Timer::after_millis(20));
                                    continue;
                                }
                                break;
                            }
                        }
                        Err(e) => error!("Postcard deserialize split message error: {}", e),
                    }
                }
            },
        });

        let mut peripheral =
            SplitPeripheral::new(BleSplitPeripheralDriver::new(&server, &conn, receiver));
        let peripheral_fut = peripheral.run();
        let matrix_fut = matrix.run();
        select3(matrix_fut, server_fut, peripheral_fut).await;
    }
}
