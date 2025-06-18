use bt_hci::cmd::le::LeSetPhy;
use bt_hci::controller::ControllerCmdAsync;
use embassy_futures::join::join;
use embassy_futures::select::select;
use embassy_time::Timer;
use trouble_host::prelude::*;
#[cfg(feature = "storage")]
use {super::PeerAddress, crate::storage::Storage, embedded_storage_async::nor_flash::NorFlash};

use crate::split::driver::{SplitDriverError, SplitReader, SplitWriter};
use crate::split::peripheral::SplitPeripheral;
use crate::split::{SplitMessage, SPLIT_MESSAGE_MAX_SIZE};
use crate::CONNECTION_STATE;

/// Gatt service used in split peripheral to send split message to central
#[gatt_service(uuid = "4dd5fbaa-18e5-4b07-bf0a-353698659946")]
pub(crate) struct SplitBleService {
    #[characteristic(uuid = "0e6313e3-bd0b-45c2-8d2e-37a2e8128bc3", read, notify, indicate)]
    pub(crate) message_to_central: [u8; SPLIT_MESSAGE_MAX_SIZE],

    #[characteristic(uuid = "4b3514fb-cae4-4d38-a097-3a2a3d1c3b9c", write_without_response, read, notify)]
    pub(crate) message_to_peripheral: [u8; SPLIT_MESSAGE_MAX_SIZE],
}

/// Gatt server in split peripheral
#[gatt_server]
pub(crate) struct BleSplitPeripheralServer {
    pub(crate) service: SplitBleService,
}

/// BLE driver for split peripheral
pub(crate) struct BleSplitPeripheralDriver<'stack, 'server, 'c, P: PacketPool> {
    message_to_peripheral: Characteristic<[u8; SPLIT_MESSAGE_MAX_SIZE]>,
    message_to_central: Characteristic<[u8; SPLIT_MESSAGE_MAX_SIZE]>,
    conn: &'c GattConnection<'stack, 'server, P>,
}

impl<'stack, 'server, 'c, P: PacketPool> BleSplitPeripheralDriver<'stack, 'server, 'c, P> {
    pub(crate) fn new(server: &'server BleSplitPeripheralServer, conn: &'c GattConnection<'stack, 'server, P>) -> Self {
        Self {
            message_to_central: server.service.message_to_central,
            message_to_peripheral: server.service.message_to_peripheral,
            conn,
        }
    }
}

impl<'stack, 'server, 'c, P: PacketPool> SplitReader for BleSplitPeripheralDriver<'stack, 'server, 'c, P> {
    async fn read(&mut self) -> Result<SplitMessage, SplitDriverError> {
        let message = loop {
            match self.conn.next().await {
                GattConnectionEvent::Disconnected { reason } => {
                    error!("Disconnected from central: {:?}", reason);
                    CONNECTION_STATE.store(false, core::sync::atomic::Ordering::Release);
                    return Err(SplitDriverError::Disconnected);
                }
                GattConnectionEvent::Gatt { event: gatt_event } => {
                    match &gatt_event {
                        GattEvent::Read(event) => {
                            info!("Gatt read event: {:?}", event.handle());
                        }
                        GattEvent::Write(event) => {
                            // Write to peripheral
                            if event.handle() == self.message_to_peripheral.handle {
                                trace!("Got message from central: {:?}", event.data());
                                match postcard::from_bytes::<SplitMessage>(&event.data()) {
                                    Ok(message) => {
                                        trace!("Message from central: {:?}", message);
                                        break message;
                                    }
                                    Err(e) => error!("Postcard deserialize split message error: {}", e),
                                }
                            } else {
                                info!("Gatt write other event: {:?}", event.handle());
                            }
                        }
                        _ => debug!("Other gatt event"),
                    };
                    match gatt_event.accept() {
                        Ok(r) => r.send().await,
                        Err(e) => warn!("[gatt] error sending response: {:?}", e),
                    }
                }
                GattConnectionEvent::ConnectionParamsUpdated {
                    conn_interval,
                    peripheral_latency,
                    supervision_timeout,
                } => {
                    info!(
                        "Connection parameters updated: {:?}ms, {:?}, {:?}ms",
                        conn_interval.as_millis(),
                        peripheral_latency,
                        supervision_timeout.as_millis()
                    );
                }
                GattConnectionEvent::PhyUpdated { tx_phy, rx_phy } => {
                    info!("PHY updated: {:?}, {:?}", tx_phy, rx_phy);
                }
                _ => (),
            }
        };
        Ok(message)
    }
}

impl<'stack, 'server, 'c, P: PacketPool> SplitWriter for BleSplitPeripheralDriver<'stack, 'server, 'c, P> {
    async fn write(&mut self, message: &SplitMessage) -> Result<usize, SplitDriverError> {
        let mut buf = [0_u8; SPLIT_MESSAGE_MAX_SIZE];
        postcard::to_slice(message, &mut buf).map_err(|e| {
            error!("Postcard serialize split message error: {}", e);
            SplitDriverError::SerializeError
        })?;
        info!("Writing split message to central: {:?}", message);
        self.message_to_central.notify(&self.conn, &buf).await.map_err(|e| {
            error!("BLE notify error: {:?}", e);
            SplitDriverError::BleError(1)
        })?;
        Ok(buf.len())
    }
}

/// Initialize and run the nRF peripheral keyboard service via BLE.
///
/// # Arguments
///
/// * `id` - The id of the peripheral
/// * `central_addr` - The address of the central
/// * `stack` - The stack to use
pub async fn initialize_nrf_ble_split_peripheral_and_run<
    'stack,
    's,
    C: Controller + ControllerCmdAsync<LeSetPhy>,
    F: NorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    id: usize,
    stack: &'stack Stack<'stack, C, DefaultPacketPool>,
    storage: &'s mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
) {
    let Host {
        mut peripheral, runner, ..
    } = stack.build();

    // First, read central address from storage
    let mut central_saved = false;
    let mut central_addr = if let Ok(Some(central_addr)) = storage.read_peer_address(0).await {
        if central_addr.is_valid {
            central_saved = true;
            Some(central_addr.address)
        } else {
            None
        }
    } else {
        None
    };

    let peri_task = async {
        let server = BleSplitPeripheralServer::new_default("rmk").unwrap();
        loop {
            CONNECTION_STATE.store(false, core::sync::atomic::Ordering::Release);
            match split_peripheral_advertise(id, central_addr, &mut peripheral, &server).await {
                Ok(conn) => {
                    info!("Connected to the central");

                    let mut peripheral = SplitPeripheral::new(BleSplitPeripheralDriver::new(&server, &conn));
                    // Save central address to storage if the central address is not saved
                    if !central_saved {
                        info!("Saving central address to storage");
                        if let Ok(()) = storage
                            .write_peer_address(PeerAddress {
                                peer_id: 0,
                                is_valid: true,
                                address: conn.raw().peer_address().into_inner(),
                            })
                            .await
                        {
                            central_saved = true;
                            central_addr = Some(conn.raw().peer_address().into_inner());
                        }
                    }
                    // Start run peripheral service
                    select(storage.run(), peripheral.run()).await;
                    info!("Disconnected from the central");
                }
                Err(e) => {
                    #[cfg(feature = "defmt")]
                    let e = defmt::Debug2Format(&e);
                    error!("Advertise error: {:?}", e);
                    Timer::after_millis(500).await;
                    continue;
                }
            };
        }
    };

    join(ble_task(runner), peri_task).await;
}

/// Create an advertiser to use to connect to a BLE Central, and wait for it to connect.
async fn split_peripheral_advertise<'a, 'b, C: Controller>(
    id: usize,
    central_addr: Option<[u8; 6]>,
    peripheral: &mut Peripheral<'a, C, DefaultPacketPool>,
    server: &'b BleSplitPeripheralServer<'_>,
) -> Result<GattConnection<'a, 'b, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut advertiser_data = [0; 31];
    let advertisement = match central_addr {
        Some(addr) => Advertisement::ConnectableNonscannableDirected {
            peer: Address::random(addr),
        },
        None => {
            // No central address provided, so we advertise as undirected
            AdStructure::encode_slice(
                &[
                    AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
                    AdStructure::ServiceUuids128(&[
                        // uuid: 4dd5fbaa-18e5-4b07-bf0a-353698659946
                        [
                            70u8, 153u8, 101u8, 152u8, 54u8, 53u8, 10u8, 191u8, 7u8, 75u8, 229u8, 24u8, 170u8, 251u8,
                            213u8, 77u8,
                        ],
                    ]),
                    AdStructure::ManufacturerSpecificData {
                        company_identifier: 0xe118,
                        payload: &[id as u8],
                    },
                ],
                &mut advertiser_data[..],
            )?;
            trace!("advertising data: {:?}", advertiser_data);
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..],
                scan_data: &[],
            }
        }
    };

    let advertiser = peripheral
        .advertise(&AdvertisementParameters::default(), advertisement)
        .await?;

    let conn = advertiser.accept().await?.with_attribute_server(server)?;
    info!("[adv] connection established");
    Ok(conn)
}

/// This is a background task that is required to run forever alongside any other BLE tasks.
async fn ble_task<C: Controller + ControllerCmdAsync<LeSetPhy>, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    loop {
        if let Err(e) = runner.run().await {
            panic!("[ble_task] error: {:?}", e);
        }
    }
}
