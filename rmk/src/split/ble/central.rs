use core::sync::atomic::Ordering;

use embassy_futures::select::{select, Either};
use embassy_sync::signal::Signal;
use embassy_time::Duration;
use trouble_host::prelude::*;

use crate::split::driver::{PeripheralManager, SplitDriverError, SplitReader, SplitWriter};
use crate::split::{SplitMessage, SPLIT_MESSAGE_MAX_SIZE};
use crate::CONNECTION_STATE;

pub(crate) static STACK_STARTED: Signal<crate::RawMutex, bool> = Signal::new();

const L2CAP_MTU: usize = 255;

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
    info!("Peer address: {:?}", address);
    let config = ConnectConfig {
        connect_params: ConnectParams {
            min_connection_interval: Duration::from_millis(15),
            max_connection_interval: Duration::from_millis(15),
            max_latency: 99,
            supervision_timeout: Duration::from_secs(5),
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
    // GattConnection is used for notification
    let server = BleSplitCentralServer::new_with_config(GapConfig::Central(CentralConfig {
        name: "rmk".into(),
        appearance: &appearance::UNKNOWN,
    }))
    .unwrap();

    let gatt_conn = conn.clone().with_attribute_server(&server)?;

    let client = GattClient::<C, 10, L2CAP_MTU>::new(&stack, &conn).await?;

    match select(
        client.task(),
        run_peripheral_manager::<_, ROW, COL, ROW_OFFSET, COL_OFFSET>(id, gatt_conn, &client),
    )
    .await
    {
        Either::First(e) => e?,
        Either::Second(e) => e?,
    }

    Ok(())
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
    gatt_conn: GattConnection<'a, '_>,
    client: &GattClient<'_, C, 10, L2CAP_MTU>,
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
        let split_ble_driver = BleSplitCentralDriver::new(listener, message_to_peripheral, &gatt_conn);
        let peripheral_manager = PeripheralManager::<ROW, COL, ROW_OFFSET, COL_OFFSET, _>::new(split_ble_driver, id);
        peripheral_manager.run().await;
    };
    Ok(())
}

/// Ble central driver which reads and writes the split message.
///
/// Different from serial, BLE split message is processed in a separate service.
/// The BLE service should keep running, it processes the split message in the callback, which is not async.
/// It's impossible to implement `SplitReader` or `SplitWriter` for BLE service,
/// so we need this wrapper to forward split message to channel.
pub(crate) struct BleSplitCentralDriver<'a, 'b, 'c> {
    // Listener for split message from peripheral
    listener: NotificationListener<'c, L2CAP_MTU>,
    // Characteristic to send split message to peripheral
    message_to_peripheral: Characteristic<[u8; SPLIT_MESSAGE_MAX_SIZE]>,
    // Cached connection state
    connection_state: bool,
    // Connection
    conn: &'b GattConnection<'a, 'b>,
}

impl<'a, 'b, 'c> BleSplitCentralDriver<'a, 'b, 'c> {
    pub(crate) fn new(
        listener: NotificationListener<'c, L2CAP_MTU>,
        message_to_peripheral: Characteristic<[u8; SPLIT_MESSAGE_MAX_SIZE]>,
        conn: &'b GattConnection<'a, 'b>,
    ) -> Self {
        Self {
            listener,
            message_to_peripheral,
            connection_state: CONNECTION_STATE.load(Ordering::Acquire),
            conn,
        }
    }
}

impl<'a, 'b, 'c> SplitReader for BleSplitCentralDriver<'a, 'b, 'c> {
    async fn read(&mut self) -> Result<SplitMessage, SplitDriverError> {
        let data = self.listener.next().await;
        let message = postcard::from_bytes(&data.as_ref()).map_err(|_| SplitDriverError::DeserializeError)?;
        info!("Received split message: {:?}", message);
        Ok(message)
    }
}

impl<'a, 'b, 'c> SplitWriter for BleSplitCentralDriver<'a, 'b, 'c> {
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
                if let Err(e) = self.message_to_peripheral.notify(&self.conn, &buf).await {
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
