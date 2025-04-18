use core::sync::atomic::Ordering;

use embassy_futures::select::{select, Either};
use embassy_sync::signal::Signal;
use embassy_time::Duration;
use rand_core::{CryptoRng, RngCore};
use trouble_host::prelude::*;

use crate::ble::trouble::{CONNECTIONS_MAX, L2CAP_CHANNELS_MAX, L2CAP_MTU};
use crate::boot::reboot_keyboard;
use crate::split::driver::{PeripheralManager, SplitDriverError, SplitReader, SplitWriter};
use crate::split::{SplitMessage, SPLIT_MESSAGE_MAX_SIZE};
use crate::CONNECTION_STATE;
#[cfg(feature = "storage")]
use {
    crate::split::ble::PeerAddress,
    crate::storage::Storage,
    crate::storage::{get_peer_address_key, StorageData},
    embedded_storage_async::nor_flash::NorFlash,
    sequential_storage::cache::NoCache,
    sequential_storage::map::store_item,
};

pub(crate) static STACK_STARTED: Signal<crate::RawMutex, bool> = Signal::new();

/// Gatt service used in split central to send split message to peripheral
#[gatt_service(uuid = "4dd5fbaa-18e5-4b07-bf0a-353698659946")]
struct SplitBleCentralService {
    #[characteristic(uuid = "0e6313e3-bd0b-45c2-8d2e-37a2e8128bc3", read, notify)]
    message_to_central: [u8; SPLIT_MESSAGE_MAX_SIZE],

    #[characteristic(uuid = "4b3514fb-cae4-4d38-a097-3a2a3d1c3b9c", write_without_response, read, notify)]
    message_to_peripheral: [u8; SPLIT_MESSAGE_MAX_SIZE],
}

/// Gatt server in split peripheral
#[gatt_server]
struct BleSplitCentralServer {
    service: SplitBleCentralService,
}

/// Do peripheral discovery.
///
/// To connect directly to the peripheral, the central should know the address of the peripheral.
/// This function is used to discover the peripheral and get its address.
///
/// The discovery process is done in the following steps:
/// 1. The peripheral advertises with a pre-defined address which is used only for the discovery.
/// 2. The central connects to the peripheral with the pre-defined address.
/// 3. The central and the peripheral exchange their unique static addresses.
/// 4. The central and the peripheral store the peer addresses in the flash.
/// 5. Reboot
///
/// After the reboot, if the peer address can be read correctly, then skip the discovery next time.
pub async fn discover_peripheral<
    'a,
    C: Controller,
    RNG: RngCore + CryptoRng,
    #[cfg(feature = "storage")] F: NorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    num_peripherals: usize,
    static_addr: [u8; 6],
    controller: C,
    random_generator: &mut RNG,
    resources: &'a mut HostResources<CONNECTIONS_MAX, L2CAP_CHANNELS_MAX, L2CAP_MTU>,
    #[cfg(feature = "storage")] storage: &'a mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
) {
    // Special central address for peripheral discovery
    let central_address = [0x18, 0xe2, 0x21, 0x88, 0xc0, 0xc7];
    // Initialize trouble host stack
    let stack = trouble_host::new(controller, resources)
        .set_random_address(Address::random(central_address))
        .set_random_generator_seed(random_generator);
    let Host {
        mut central,
        mut runner,
        ..
    } = stack.build();

    for peri_id in 0..num_peripherals {
        // Peripheral address
        let peripheral_address = [0x7e, 0xff, 0x73, peri_id as u8, 0x66, 0xe3];
        let address: Address = Address::random(peripheral_address);
        info!("Peripheral peer address: {:?}", address);

        let config = ConnectConfig {
            connect_params: ConnectParams {
                min_connection_interval: Duration::from_micros(7500), // 7.5ms
                max_connection_interval: Duration::from_micros(7500), // 7.5ms
                max_latency: 400,                                     // 3s
                supervision_timeout: Duration::from_secs(7),
                ..Default::default()
            },
            scan_config: ScanConfig {
                filter_accept_list: &[(address.kind, &address.addr)],
                ..Default::default()
            },
        };

        match select(
            runner.run(),
            connect_and_run_peripheral_discovering(static_addr, &stack, &mut central, &config),
        )
        .await
        {
            Either::Second(Ok(addr)) => {
                #[cfg(feature = "storage")]
                {
                    let key = get_peer_address_key(peri_id as u8);
                    let data = StorageData::PeerAddress(PeerAddress::new(peri_id as u8, true, addr));
                    debug!("Writing peer address to storage: {:?}", data);
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
    }

    info!("Peripherals discovered, rebooting");
    // embassy_time::Timer::after_secs(5).await;
    // Reboot keyboard after the peripheral is discovered and saved
    reboot_keyboard();
}

async fn connect_and_run_peripheral_discovering<'a, C: Controller>(
    addr: [u8; 6],
    stack: &'a Stack<'a, C>,
    central: &mut Central<'a, C>,
    config: &ConnectConfig<'_>,
) -> Result<[u8; 6], BleHostError<C::Error>> {
    let conn = central.connect(config).await?;
    info!("Connected to peripheral");
    let client = GattClient::<C, 10, L2CAP_MTU>::new(&stack, &conn).await?;
    match select(client.task(), run_discover_peripheral(addr, &client)).await {
        Either::First(_) => Err(BleHostError::BleHost(Error::Other)),
        Either::Second(result) => result,
    }
}

async fn run_discover_peripheral<'a, C: Controller>(
    addr: [u8; 6],
    client: &GattClient<'a, C, 10, L2CAP_MTU>,
) -> Result<[u8; 6], BleHostError<C::Error>> {
    let services = client
        .services_by_uuid(&Uuid::new_long([
            70u8, 153u8, 101u8, 152u8, 54u8, 53u8, 10u8, 191u8, 7u8, 75u8, 229u8, 24u8, 170u8, 251u8, 213u8, 77u8,
        ]))
        .await?;
    info!("Services found");
    if let Some(service) = services.first() {
        let message_to_central = client
            .characteristic_by_uuid::<[u8; SPLIT_MESSAGE_MAX_SIZE]>(
                &service,
                // uuid: 0e6313e3-bd0b-45c2-8d2e-37a2e8128bc3
                &Uuid::Uuid128([
                    195u8, 139u8, 18u8, 232u8, 162u8, 55u8, 46u8, 141u8, 194u8, 69u8, 11u8, 189u8, 227u8, 19u8, 99u8,
                    14u8,
                ]),
            )
            .await?;
        info!("Message to central found");
        let message_to_peripheral = client
            .characteristic_by_uuid::<[u8; SPLIT_MESSAGE_MAX_SIZE]>(
                &service,
                // uuid: 4b3514fb-cae4-4d38-a097-3a2a3d1c3b9c
                &Uuid::Uuid128([
                    156u8, 59u8, 28u8, 61u8, 42u8, 58u8, 151u8, 160u8, 56u8, 77u8, 228u8, 202u8, 251u8, 20u8, 53u8,
                    75u8,
                ]),
            )
            .await?;
        info!("Subscribing notifications");
        let peer_addr = loop {
            let mut listener = match client.subscribe(&message_to_central, false).await {
                Ok(listener) => listener,
                Err(e) => {
                    #[cfg(feature = "defmt")]
                    let e = defmt::Debug2Format(&e);
                    error!("Failed to subscribe to message_to_central: {:?}", e);
                    continue;
                }
            };
            let data = listener.next().await;
            let addr = match postcard::from_bytes::<SplitMessage>(&data.as_ref()) {
                Ok(SplitMessage::Address(addr)) => addr,
                Err(e) => {
                    error!("Failed to deserialize split message: {:?}", e);
                    continue;
                }
                _ => continue,
            };
            info!("Received split addr message: {:?}", addr);
            break addr;
        };

        loop {
            let mut buf = [0_u8; SPLIT_MESSAGE_MAX_SIZE];
            // Send local address to the peripheral
            let message = SplitMessage::Address(addr);
            if let Ok(_bytes) = postcard::to_slice(&message, &mut buf) {
                info!("Sending split addr message to peripheral");
                if let Err(e) = client
                    .write_characteristic_without_response(&message_to_peripheral, &buf)
                    .await
                {
                    #[cfg(feature = "defmt")]
                    let e = defmt::Debug2Format(&e);
                    error!("Failed to write split message to peripheral: {:?}", e);
                    continue;
                } else {
                    embassy_time::Timer::after_millis(500).await;
                    break;
                }
            };
        }
        info!("Peripheral discovery finished");
        Ok(peer_addr)
    } else {
        error!("Peripheral service discovery failed");
        Err(BleHostError::BleHost(Error::NotFound))
    }
}

pub(crate) async fn run_ble_peripheral_manager<
    'a,
    C: Controller,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
>(
    id: usize,
    addr: [u8; 6],
    stack: &'a Stack<'a, C>,
) {
    let Host { mut central, .. } = stack.build();
    let address: Address = Address::random(addr);
    info!("Peripheral peer address: {:?}", address);
    let config = ConnectConfig {
        connect_params: ConnectParams {
            min_connection_interval: Duration::from_micros(7500), // 7.5ms
            max_connection_interval: Duration::from_micros(7500), // 7.5ms
            max_latency: 400,                                     // 3s
            supervision_timeout: Duration::from_secs(7),
            ..Default::default()
        },
        scan_config: ScanConfig {
            filter_accept_list: &[(address.kind, &address.addr)],
            ..Default::default()
        },
    };
    wait_for_stack_started().await;
    loop {
        info!("Connecting peripheral");
        if let Err(e) =
            connect_and_run_peripheral_manager::<_, ROW, COL, ROW_OFFSET, COL_OFFSET>(id, stack, &mut central, &config)
                .await
        {
            #[cfg(feature = "defmt")]
            let e = defmt::Debug2Format(&e);
            error!("BLE central error: {:?}", e);
            // Reconnect after 500ms
            embassy_time::Timer::after_millis(500).await;
        }
    }
}

async fn connect_and_run_peripheral_manager<
    'a,
    C: Controller,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
>(
    id: usize,
    stack: &'a Stack<'a, C>,
    central: &mut Central<'a, C>,
    config: &ConnectConfig<'_>,
) -> Result<(), BleHostError<C::Error>> {
    let conn = central.connect(config).await?;

    info!("Connected to peripheral");
    let client = GattClient::<C, 10, L2CAP_MTU>::new(&stack, &conn).await?;
    match select(
        ble_central_task(&client, &conn),
        run_peripheral_manager::<_, ROW, COL, ROW_OFFSET, COL_OFFSET>(id, &client),
    )
    .await
    {
        Either::First(e) => e,
        Either::Second(e) => e,
    }
}

async fn ble_central_task<'a, C: Controller>(
    client: &GattClient<'a, C, 10, L2CAP_MTU>,
    conn: &Connection<'a>,
) -> Result<(), BleHostError<C::Error>> {
    let conn_check = async {
        while conn.is_connected() {
            embassy_time::Timer::after_secs(1).await;
        }
    };
    match select(client.task(), conn_check).await {
        Either::First(e) => e,
        Either::Second(_) => {
            info!("Connection lost");
            Ok(())
        }
    }
}

async fn run_peripheral_manager<
    'a,
    C: Controller,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
>(
    id: usize,
    client: &GattClient<'a, C, 10, L2CAP_MTU>,
) -> Result<(), BleHostError<C::Error>> {
    let services = client
        .services_by_uuid(&Uuid::new_long([
            70u8, 153u8, 101u8, 152u8, 54u8, 53u8, 10u8, 191u8, 7u8, 75u8, 229u8, 24u8, 170u8, 251u8, 213u8, 77u8,
        ]))
        .await?;
    info!("Services found");
    if let Some(service) = services.first() {
        let message_to_central = client
            .characteristic_by_uuid::<[u8; SPLIT_MESSAGE_MAX_SIZE]>(
                &service,
                // uuid: 0e6313e3-bd0b-45c2-8d2e-37a2e8128bc3
                &Uuid::Uuid128([
                    195u8, 139u8, 18u8, 232u8, 162u8, 55u8, 46u8, 141u8, 194u8, 69u8, 11u8, 189u8, 227u8, 19u8, 99u8,
                    14u8,
                ]),
            )
            .await?;
        info!("Message to central found");
        let message_to_peripheral = client
            .characteristic_by_uuid::<[u8; SPLIT_MESSAGE_MAX_SIZE]>(
                &service,
                // uuid: 4b3514fb-cae4-4d38-a097-3a2a3d1c3b9c
                &Uuid::Uuid128([
                    156u8, 59u8, 28u8, 61u8, 42u8, 58u8, 151u8, 160u8, 56u8, 77u8, 228u8, 202u8, 251u8, 20u8, 53u8,
                    75u8,
                ]),
            )
            .await?;
        info!("Subscribing notifications");
        let listener = client.subscribe(&message_to_central, false).await?;
        let split_ble_driver = BleSplitCentralDriver::new(listener, message_to_peripheral, client);
        let peripheral_manager = PeripheralManager::<ROW, COL, ROW_OFFSET, COL_OFFSET, _>::new(split_ble_driver, id);
        peripheral_manager.run().await;
        info!("Peripheral manager stopped");
    };
    Ok(())
}

/// Ble central driver which reads and writes the split message.
///
/// Different from serial, BLE split message is processed in a separate service.
/// The BLE service should keep running, it processes the split message in the callback, which is not async.
/// It's impossible to implement `SplitReader` or `SplitWriter` for BLE service,
/// so we need this wrapper to forward split message to channel.
pub(crate) struct BleSplitCentralDriver<'a, 'b, 'c, C: Controller> {
    // Listener for split message from peripheral
    listener: NotificationListener<'b, L2CAP_MTU>,
    // Characteristic to send split message to peripheral
    message_to_peripheral: Characteristic<[u8; SPLIT_MESSAGE_MAX_SIZE]>,
    // Client
    client: &'c GattClient<'a, C, 10, L2CAP_MTU>,
    // Cached connection state
    connection_state: bool,
}

impl<'a, 'b, 'c, C: Controller> BleSplitCentralDriver<'a, 'b, 'c, C> {
    pub(crate) fn new(
        listener: NotificationListener<'b, L2CAP_MTU>,
        message_to_peripheral: Characteristic<[u8; SPLIT_MESSAGE_MAX_SIZE]>,
        client: &'c GattClient<'a, C, 10, L2CAP_MTU>,
    ) -> Self {
        Self {
            listener,
            message_to_peripheral,
            client,
            connection_state: CONNECTION_STATE.load(Ordering::Acquire),
        }
    }
}

impl<'a, 'b, 'c, C: Controller> SplitReader for BleSplitCentralDriver<'a, 'b, 'c, C> {
    async fn read(&mut self) -> Result<SplitMessage, SplitDriverError> {
        let data = self.listener.next().await;
        let message = postcard::from_bytes(&data.as_ref()).map_err(|_| SplitDriverError::DeserializeError)?;
        info!("Received split message: {:?}", message);
        Ok(message)
    }
}

impl<'a, 'b, 'c, C: Controller> SplitWriter for BleSplitCentralDriver<'a, 'b, 'c, C> {
    async fn write(&mut self, message: &SplitMessage) -> Result<usize, SplitDriverError> {
        if let SplitMessage::ConnectionState(state) = message {
            // ConnectionState changed, update cached state and notify peripheral
            if self.connection_state != *state {
                self.connection_state = *state;
            }
        }
        // Always sync the connection state to peripheral since central doesn't know the CONNECTION_STATE of the peripheral.
        let mut buf = [0_u8; SPLIT_MESSAGE_MAX_SIZE];
        match postcard::to_slice(&message, &mut buf) {
            Ok(_bytes) => {
                if let Err(e) = self
                    .client
                    .write_characteristic_without_response(&self.message_to_peripheral, &buf)
                    .await
                {
                    if let BleHostError::BleHost(Error::NotFound) = e {
                        error!("Peripheral disconnected");
                        return Err(SplitDriverError::Disconnected);
                    }
                    #[cfg(feature = "defmt")]
                    let e = defmt::Debug2Format(&e);
                    error!("BLE message_to_peripheral_write error: {:?}", e);
                }
            }
            Err(e) => error!("Postcard serialize split message error: {}", e),
        };

        Ok(SPLIT_MESSAGE_MAX_SIZE)
    }
}

/// Wait for the BLE stack to start.
///
/// If the BLE stack has been started, wait 500ms then quit.
pub(crate) async fn wait_for_stack_started() {
    loop {
        if STACK_STARTED.signaled() {
            embassy_time::Timer::after_millis(500).await;
            break;
        }
        embassy_time::Timer::after_millis(500).await;
    }
}
