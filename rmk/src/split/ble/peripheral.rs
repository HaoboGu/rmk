use embassy_futures::join::join;
use embassy_futures::select::{select, Either};
use embassy_time::Timer;
use rand_core::{CryptoRng, RngCore};
use trouble_host::prelude::*;

use crate::ble::trouble::{CONNECTIONS_MAX, L2CAP_CHANNELS_MAX, L2CAP_MTU};
use crate::boot::reboot_keyboard;
use crate::split::driver::{SplitDriverError, SplitReader, SplitWriter};
use crate::split::peripheral::SplitPeripheral;
use crate::split::{SplitMessage, SPLIT_MESSAGE_MAX_SIZE};
use crate::CONNECTION_STATE;

#[cfg(feature = "storage")]
use {
    super::PeerAddress,
    crate::storage::Storage,
    crate::storage::{get_peer_address_key, StorageData},
    embedded_storage_async::nor_flash::NorFlash,
    sequential_storage::cache::NoCache,
    sequential_storage::map::store_item,
};

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
pub(crate) struct BleSplitPeripheralDriver<'stack, 'server, 'c> {
    message_to_peripheral: Characteristic<[u8; SPLIT_MESSAGE_MAX_SIZE]>,
    message_to_central: Characteristic<[u8; SPLIT_MESSAGE_MAX_SIZE]>,
    conn: &'c GattConnection<'stack, 'server>,
}

impl<'stack, 'server, 'c> BleSplitPeripheralDriver<'stack, 'server, 'c> {
    pub(crate) fn new(server: &'server BleSplitPeripheralServer, conn: &'c GattConnection<'stack, 'server>) -> Self {
        Self {
            message_to_central: server.service.message_to_central,
            message_to_peripheral: server.service.message_to_peripheral,
            conn,
        }
    }
}

impl<'stack, 'server, 'c> SplitReader for BleSplitPeripheralDriver<'stack, 'server, 'c> {
    async fn read(&mut self) -> Result<SplitMessage, SplitDriverError> {
        let message = loop {
            match self.conn.next().await {
                GattConnectionEvent::Disconnected { reason } => {
                    error!("Disconnected from central: {:?}", reason);
                    CONNECTION_STATE.store(false, core::sync::atomic::Ordering::Release);
                    return Err(SplitDriverError::Disconnected);
                }
                GattConnectionEvent::Gatt { event } => match event {
                    Ok(gatt_event) => {
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
                        };
                        match gatt_event.accept() {
                            Ok(r) => r.send().await,
                            Err(e) => warn!("[gatt] error sending response: {:?}", e),
                        }
                    }
                    Err(e) => warn!("[gatt] error processing event: {:?}", e),
                },
                _ => (),
            }
        };
        Ok(message)
    }
}

impl<'stack, 'server, 'c> SplitWriter for BleSplitPeripheralDriver<'stack, 'server, 'c> {
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

pub async fn discover_central<
    'a,
    C: Controller,
    RNG: RngCore + CryptoRng,
    #[cfg(feature = "storage")] F: NorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    peri_id: usize,
    static_addr: [u8; 6],
    controller: C,
    random_generator: &mut RNG,
    resources: &'a mut HostResources<CONNECTIONS_MAX, L2CAP_CHANNELS_MAX, L2CAP_MTU>,
    #[cfg(feature = "storage")] storage: &'a mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
) {
    // Special addresses for central and peripheral pairing
    let central_address = [0x18, 0xe2, 0x21, 0x88, 0xc0, 0xc7];
    let peripheral_address = [0x7e, 0xff, 0x73, peri_id as u8, 0x66, 0xe3];

    // Initialize trouble host stack
    let stack = trouble_host::new(controller, resources)
        .set_random_address(Address::random(peripheral_address))
        .set_random_generator_seed(random_generator);

    let Host {
        mut peripheral, runner, ..
    } = stack.build();

    let peri_task = async {
        let server = BleSplitPeripheralServer::new_default("rmk").unwrap();
        loop {
            match split_peripheral_advertise(central_address, &mut peripheral, &server).await {
                Ok(conn) => {
                    info!("Conected to the central");
                    let mut driver = BleSplitPeripheralDriver::new(&server, &conn);
                    // Wait for cccd to be set
                    loop {
                        match driver.conn.next().await {
                            GattConnectionEvent::Gatt {
                                event: Result::Ok(GattEvent::Write(e)),
                            } => {
                                info!("Gatt write event: {:?}", e.handle());
                                break;
                            }
                            _ => continue,
                        }
                    }
                    // Write local static address to central
                    loop {
                        let message = SplitMessage::Address(static_addr);
                        if let Err(e) = driver.write(&message).await {
                            error!("BLE notify error: {:?}", e);
                            continue;
                        }
                        break;
                    }
                    // Wait for central to send local address
                    let addr = loop {
                        match driver.read().await {
                            Ok(SplitMessage::Address(addr)) => {
                                info!("Received split addr message: {:?}", addr);
                                break addr;
                            }
                            Err(e) => {
                                error!("BLE read error: {:?}", e);
                                continue;
                            }
                            _ => continue,
                        }
                    };
                    info!("Disconnected from the central");
                    break addr;
                }
                Err(e) => {
                    #[cfg(feature = "defmt")]
                    let e = defmt::Debug2Format(&e);
                    error!("Advertise error: {:?}", e);
                    Timer::after_millis(500).await;
                    continue;
                }
            }
        }
    };

    match select(ble_task(runner), peri_task).await {
        Either::Second(addr) => {
            #[cfg(feature = "storage")]
            {
                // Write peer address to storage
                let key = get_peer_address_key(0);
                let data = StorageData::PeerAddress(PeerAddress::new(0, true, addr));
                let _ = store_item(
                    &mut storage.flash,
                    storage.storage_range.clone(),
                    &mut NoCache::new(),
                    &mut storage.buffer,
                    &key,
                    &data,
                )
                .await;
            }
        }
        _ => (),
    }

    info!("Central discovered, rebooting");
    // embassy_time::Timer::after_secs(5).await;
    // Reboot keyboard after the central is discovered
    reboot_keyboard();
}

/// Initialize and run the nRF peripheral keyboard service via BLE.
///
/// # Arguments
///
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `spawner` - embassy task spawner, used to spawn nrf_softdevice background task
pub async fn initialize_nrf_ble_split_peripheral_and_run<'stack, C: Controller>(
    central_addr: [u8; 6],
    stack: &'stack Stack<'stack, C>,
) {
    let Host {
        mut peripheral, runner, ..
    } = stack.build();

    let peri_task = async {
        let server = BleSplitPeripheralServer::new_default("rmk").unwrap();
        loop {
            CONNECTION_STATE.store(false, core::sync::atomic::Ordering::Release);
            match split_peripheral_advertise(central_addr, &mut peripheral, &server).await {
                Ok(conn) => {
                    info!("Conected to the central");
                    let mut peripheral = SplitPeripheral::new(BleSplitPeripheralDriver::new(&server, &conn));
                    peripheral.run().await;
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
    central_addr: [u8; 6],
    peripheral: &mut Peripheral<'a, C>,
    server: &'b BleSplitPeripheralServer<'_>,
) -> Result<GattConnection<'a, 'b>, BleHostError<C::Error>> {
    let advertisement = Advertisement::ConnectableNonscannableDirected {
        peer: Address::random(central_addr),
    };
    let advertiser = peripheral
        .advertise(&AdvertisementParameters::default(), advertisement)
        .await?;

    let conn = advertiser.accept().await?.with_attribute_server(server)?;
    info!("[adv] connection established");
    Ok(conn)
}

/// This is a background task that is required to run forever alongside any other BLE tasks.
async fn ble_task<C: Controller>(mut runner: Runner<'_, C>) {
    loop {
        if let Err(e) = runner.run().await {
            panic!("[ble_task] error: {:?}", e);
        }
    }
}
