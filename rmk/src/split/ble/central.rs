use core::sync::atomic::Ordering;

use bt_hci::cmd::le::{LeReadLocalSupportedFeatures, LeSetPhy, LeSetScanParams};
use bt_hci::controller::{ControllerCmdAsync, ControllerCmdSync};
use embassy_futures::select::{Either, Either3, select, select3};
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};
use embedded_storage_async::nor_flash::NorFlash;
use trouble_host::prelude::*;
#[cfg(feature = "controller")]
use {
    crate::channel::{CONTROLLER_CHANNEL, ControllerPub, send_controller_event},
    crate::event::ControllerEvent,
};

use crate::ble::{SLEEPING_STATE, update_ble_phy, update_conn_params};
use crate::channel::FLASH_CHANNEL;
#[cfg(feature = "storage")]
use crate::split::ble::PeerAddress;
use crate::split::driver::{PeripheralManager, SplitDriverError, SplitReader, SplitWriter};
use crate::split::{SPLIT_MESSAGE_MAX_SIZE, SplitMessage};
use crate::storage::{FlashOperationMessage, Storage};
use crate::{CONNECTION_STATE, SPLIT_CENTRAL_SLEEP_TIMEOUT_MINUTES};

pub(crate) static STACK_STARTED: Signal<crate::RawMutex, bool> = Signal::new();
pub(crate) static PERIPHERAL_FOUND: Signal<crate::RawMutex, (u8, BdAddr)> = Signal::new();

/// Sleep management signal for BLE Split Central
///
/// This signal serves dual purposes for sleep management:
/// - `signal(true)`: Indicates central has entered sleep mode
/// - `signal(false)`: Indicates activity detected, wake up or reset sleep timer
pub(crate) static CENTRAL_SLEEP: Signal<crate::RawMutex, bool> = Signal::new();

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
                info!("Found split peripheral: id={:?}, addr={:?}", peripheral_id, report.addr);
                PERIPHERAL_FOUND.signal((peripheral_id, report.addr));
                break;
            }
        }
    }
}

pub(crate) async fn run_ble_peripheral_manager<
    'a,
    C: Controller
        + ControllerCmdSync<LeSetScanParams>
        + ControllerCmdAsync<LeSetPhy>
        + ControllerCmdSync<LeReadLocalSupportedFeatures>,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
>(
    peripheral_id: usize,
    addr: Option<[u8; 6]>,
    stack: &'a Stack<'a, C, DefaultPacketPool>,
) {
    trace!("SPLIT_MESSAGE_MAX_SIZE: {}", SPLIT_MESSAGE_MAX_SIZE);
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
            min_connection_interval: Duration::from_micros(7500),
            max_connection_interval: Duration::from_micros(7500),
            max_latency: 400, // 3s
            supervision_timeout: Duration::from_secs(7),
            ..Default::default()
        },
        scan_config: ScanConfig {
            filter_accept_list: &[(address.kind, &address.addr)],
            ..Default::default()
        },
    };
    wait_for_stack_started().await;

    #[cfg(feature = "controller")]
    let mut controller_pub = unwrap!(CONTROLLER_CHANNEL.publisher());

    loop {
        #[cfg(feature = "controller")]
        send_controller_event(
            &mut controller_pub,
            ControllerEvent::SplitPeripheral(peripheral_id, false),
        );
        info!("Connecting peripheral");
        if let Err(e) = connect_and_run_peripheral_manager::<_, _, ROW, COL, ROW_OFFSET, COL_OFFSET>(
            peripheral_id,
            stack,
            &mut central,
            &config,
            #[cfg(feature = "controller")]
            &mut controller_pub,
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
    C: Controller + ControllerCmdAsync<LeSetPhy> + ControllerCmdSync<LeReadLocalSupportedFeatures>,
    P: PacketPool,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
>(
    id: usize,
    stack: &'a Stack<'a, C, P>,
    central: &mut Central<'a, C, P>,
    config: &ConnectConfig<'_>,
    #[cfg(feature = "controller")] controller_pub: &mut ControllerPub,
) -> Result<(), BleHostError<C::Error>> {
    let conn = central.connect(config).await?;

    info!("Connected to peripheral");

    #[cfg(feature = "controller")]
    send_controller_event(controller_pub, ControllerEvent::SplitPeripheral(id, true));

    let client = GattClient::<C, P, 10>::new(&stack, &conn).await?;

    // Use 2M Phy
    update_ble_phy(stack, &conn).await;

    info!("Updating connection parameters for peripheral");
    update_conn_params(
        stack,
        &conn,
        &ConnectParams {
            min_connection_interval: Duration::from_micros(7500),
            max_connection_interval: Duration::from_micros(7500),
            max_latency: 400, // 3s
            supervision_timeout: Duration::from_secs(7),
            ..Default::default()
        },
    )
    .await;

    match select3(
        ble_central_task(&client, &conn),
        run_peripheral_manager::<_, _, ROW, COL, ROW_OFFSET, COL_OFFSET>(id, &client),
        sleep_manager_task(stack, &conn),
    )
    .await
    {
        Either3::First(e) => e,
        Either3::Second(e) => e,
        Either3::Third(e) => e,
    }
}

async fn ble_central_task<'a, C: Controller + ControllerCmdAsync<LeSetPhy>, P: PacketPool>(
    client: &GattClient<'a, C, P, 10>,
    conn: &Connection<'a, P>,
) -> Result<(), BleHostError<C::Error>> {
    // Simply monitor connection status
    let conn_check = async {
        while conn.is_connected() {
            Timer::after_secs(5).await;
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
    C: Controller + ControllerCmdAsync<LeSetPhy>,
    P: PacketPool,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
>(
    id: usize,
    client: &GattClient<'a, C, P, 10>,
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
pub(crate) struct BleSplitCentralDriver<'a, 'b, 'c, C: Controller + ControllerCmdAsync<LeSetPhy>, P: PacketPool> {
    // Listener for split message from peripheral
    listener: NotificationListener<'b, 512>,
    // Characteristic to send split message to peripheral
    message_to_peripheral: Characteristic<[u8; SPLIT_MESSAGE_MAX_SIZE]>,
    // Client
    client: &'c GattClient<'a, C, P, 10>,
    // Cached connection state
    connection_state: bool,
}

impl<'a, 'b, 'c, C: Controller + ControllerCmdAsync<LeSetPhy>, P: PacketPool> BleSplitCentralDriver<'a, 'b, 'c, C, P> {
    pub(crate) fn new(
        listener: NotificationListener<'b, 512>,
        message_to_peripheral: Characteristic<[u8; SPLIT_MESSAGE_MAX_SIZE]>,
        client: &'c GattClient<'a, C, P, 10>,
    ) -> Self {
        Self {
            listener,
            message_to_peripheral,
            client,
            connection_state: CONNECTION_STATE.load(Ordering::Acquire),
        }
    }
}

impl<'a, 'b, 'c, C: Controller + ControllerCmdAsync<LeSetPhy>, P: PacketPool> SplitReader
    for BleSplitCentralDriver<'a, 'b, 'c, C, P>
{
    async fn read(&mut self) -> Result<SplitMessage, SplitDriverError> {
        let data = self.listener.next().await;
        let message = postcard::from_bytes(&data.as_ref()).map_err(|_| SplitDriverError::DeserializeError)?;
        info!("Received split message: {:?}", message);

        // Update last activity time when receiving key events from peripheral
        match &message {
            SplitMessage::Key(_) => {
                debug!("Key activity detected from peripheral");
                update_activity_time();
            }
            SplitMessage::Event(_) => {
                debug!("Event activity detected from peripheral");
                update_activity_time();
            }
            _ => {}
        }

        Ok(message)
    }
}

impl<'a, 'b, 'c, C: Controller + ControllerCmdAsync<LeSetPhy>, P: PacketPool> SplitWriter
    for BleSplitCentralDriver<'a, 'b, 'c, C, P>
{
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

/// Sleep manager task for connection between split central and peripheral
/// Handles sleep timeout and connection parameter adjustments using event-driven approach
async fn sleep_manager_task<
    'a,
    C: Controller + ControllerCmdAsync<LeSetPhy> + ControllerCmdSync<LeReadLocalSupportedFeatures>,
    P: PacketPool,
>(
    stack: &'a Stack<'a, C, P>,
    conn: &Connection<'a, P>,
) -> Result<(), BleHostError<C::Error>> {
    // Skip sleep management if timeout is 0 (disabled)
    if SPLIT_CENTRAL_SLEEP_TIMEOUT_MINUTES == 0 {
        info!("Sleep management disabled (timeout = 0)");
        core::future::pending::<()>().await;
        return Ok(());
    }

    let sleep_timeout = SPLIT_CENTRAL_SLEEP_TIMEOUT_MINUTES as u64 * 60;

    info!(
        "Sleep manager started with {}min timeout",
        SPLIT_CENTRAL_SLEEP_TIMEOUT_MINUTES
    );

    loop {
        if !SLEEPING_STATE.load(Ordering::Acquire) {
            // Wait for timeout or activity (false signal means activity/wakeup)
            match select(Timer::after_secs(sleep_timeout), CENTRAL_SLEEP.wait()).await {
                Either::First(_) => {
                    // Timeout: enter sleep mode
                }
                Either::Second(signal_value) => {
                    // Received signal - if false, it means activity detected
                    if !signal_value {
                        debug!("Activity detected, resetting sleep timeout");
                        continue;
                    }
                    // True, enter sleep mode
                }
            }

            // Timeout or received true from CENTRAL_SLEEP signal, enter sleep mode
            info!("Entering sleep mode");

            // Connection parameters are different when central is broadcasting and connected to host
            let conn_params = if CONNECTION_STATE.load(Ordering::Acquire) {
                ConnectParams {
                    min_connection_interval: Duration::from_millis(20),
                    max_connection_interval: Duration::from_millis(20),
                    max_latency: 200, // 4s
                    supervision_timeout: Duration::from_secs(9),
                    ..Default::default()
                }
            } else {
                ConnectParams {
                    min_connection_interval: Duration::from_millis(200),
                    max_connection_interval: Duration::from_millis(200),
                    max_latency: 25, // 5s
                    supervision_timeout: Duration::from_secs(11),
                    ..Default::default()
                }
            };

            // Update connection parameters
            update_conn_params(stack, conn, &conn_params).await;
            SLEEPING_STATE.store(true, Ordering::Release);
        } else {
            // Wait for activity to wake up (false signal means activity/wakeup)
            let signal_value = CENTRAL_SLEEP.wait().await;
            if !signal_value {
                info!("Waking up from sleep mode due to activity");
                SLEEPING_STATE.store(false, Ordering::Release);

                // Restore normal connection parameters
                update_conn_params(
                    stack,
                    conn,
                    &ConnectParams {
                        min_connection_interval: Duration::from_micros(7500),
                        max_connection_interval: Duration::from_micros(7500),
                        max_latency: 400, // 3s
                        supervision_timeout: Duration::from_secs(7),
                        ..Default::default()
                    },
                )
                .await;
            }
        }
    }
}

/// Update the activity time to indicate user activity
/// This function triggers activity wakeup signal for sleep management
pub(crate) fn update_activity_time() {
    CENTRAL_SLEEP.signal(false);
    debug!("Activity detected, signaling wakeup");
}
