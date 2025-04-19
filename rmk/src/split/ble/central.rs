use core::sync::atomic::Ordering;

use bt_hci::cmd::le::LeSetScanParams;
use bt_hci::controller::ControllerCmdSync;
use embassy_futures::select::{select, Either};
use embassy_sync::signal::Signal;
use embassy_time::Duration;
use embedded_storage_async::nor_flash::NorFlash;
use trouble_host::prelude::*;

use crate::ble::trouble::L2CAP_MTU;
use crate::channel::FLASH_CHANNEL;
#[cfg(feature = "storage")]
use crate::split::ble::PeerAddress;
use crate::split::driver::{PeripheralManager, SplitDriverError, SplitReader, SplitWriter};
use crate::split::{SplitMessage, SPLIT_MESSAGE_MAX_SIZE};
use crate::storage::{FlashOperationMessage, Storage};
use crate::CONNECTION_STATE;

pub(crate) static STACK_STARTED: Signal<crate::RawMutex, bool> = Signal::new();
pub(crate) static PERIPHERAL_FOUND: Signal<crate::RawMutex, (u8, BdAddr)> = Signal::new();

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

/// Read peripheral addresses from storage.
///
/// # Arguments
///
/// * `storage` - The storage to read peripheral addresses from
pub async fn read_peripheral_addresses<
    const PERI_NUM: usize,
    F: NorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
) -> heapless::Vec<Option<[u8; 6]>, PERI_NUM> {
    let mut peripheral_addresses: heapless::Vec<Option<[u8; 6]>, PERI_NUM> = heapless::Vec::new();
    for id in 0..PERI_NUM {
        if let Ok(Some(peer_address)) = storage.read_peer_address(id as u8).await {
            if peer_address.is_valid {
                peripheral_addresses.push(Some(peer_address.address)).unwrap();
                continue;
            }
        }
        peripheral_addresses.push(None).unwrap();
    }
    peripheral_addresses
}

// When no peripheral address is saved, the central should first scan for peripheral.
// This handler is used to handle the scan result.
pub(crate) struct ScanHandler {}

impl EventHandler for ScanHandler {
    fn on_adv_reports(&self, mut it: LeAdvReportsIter<'_>) {
        while let Some(Ok(report)) = it.next() {
            // Check advertisement data
            if report.data.len() < 25 {
                continue;
            }
            if report.data[4] == 0x07
                && report.data[5..].starts_with(&[
                    // uuid: 4dd5fbaa-18e5-4b07-bf0a-353698659946
                    70u8, 153u8, 101u8, 152u8, 54u8, 53u8, 10u8, 191u8, 7u8, 75u8, 229u8, 24u8, 170u8, 251u8, 213u8,
                    77u8,
                ])
                && report.data[21..25] == [0x04, 0xff, 0x18, 0xe1]
            {
                // Uuid and manufacturer specific data check passed
                let peripheral_id = report.data[25];
                info!("Found split peripheral: id={}, addr={}", peripheral_id, report.addr);
                PERIPHERAL_FOUND.signal((peripheral_id, report.addr));
                break;
            }
        }
    }
}

pub(crate) async fn run_ble_peripheral_manager<
    'a,
    C: Controller + ControllerCmdSync<LeSetScanParams>,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
>(
    peripheral_id: usize,
    addr: Option<[u8; 6]>,
    stack: &'a Stack<'a, C>,
) {
    let address = match addr {
        Some(addr) => Address::random(addr),
        None => {
            let Host { central, .. } = stack.build();
            info!("No peripheral address is saved, scan for peripheral first");
            wait_for_stack_started().await;
            let mut scanner = Scanner::new(central);
            let scan_config = ScanConfig {
                active: false,
                ..Default::default()
            };
            let addr = if let Ok(_session) = scanner.scan(&scan_config).await {
                loop {
                    let (found_peripheral_id, addr) = PERIPHERAL_FOUND.wait().await;
                    if found_peripheral_id == peripheral_id as u8 {
                        // Peripheral found, save the peripheral's address to flash
                        FLASH_CHANNEL
                            .send(FlashOperationMessage::PeerAddress(PeerAddress::new(
                                found_peripheral_id,
                                true,
                                addr.into_inner(),
                            )))
                            .await;
                        // Then connect to the peripheral
                        break Address::random(addr.into_inner());
                    } else {
                        // Not this peripheral, signal the value back and continue
                        PERIPHERAL_FOUND.signal((found_peripheral_id, addr));
                        embassy_time::Timer::after_millis(500).await;
                        continue;
                    }
                }
            } else {
                panic!("Failed to start peripheral scanning");
            };
            addr
        }
    };

    let Host { mut central, .. } = stack.build();
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
        if let Err(e) = connect_and_run_peripheral_manager::<_, ROW, COL, ROW_OFFSET, COL_OFFSET>(
            peripheral_id,
            stack,
            &mut central,
            &config,
        )
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
