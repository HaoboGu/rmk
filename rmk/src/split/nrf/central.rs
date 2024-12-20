use core::sync::atomic::{AtomicBool, Ordering};

use embassy_futures::{join::join, select::select};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver, Sender},
};
use nrf_softdevice::ble::{central, gatt_client, Address, AddressType};

use crate::split::{
    driver::{PeripheralMatrixMonitor, SplitDriverError, SplitReader, SplitWriter},
    SplitMessage, SPLIT_MESSAGE_MAX_SIZE,
};

/// Gatt client used in split central to receive split message from peripherals
#[nrf_softdevice::gatt_client(uuid = "4dd5fbaa-18e5-4b07-bf0a-353698659946")]
pub(crate) struct BleSplitCentralClient {
    #[characteristic(uuid = "0e6313e3-bd0b-45c2-8d2e-37a2e8128bc3", read, notify)]
    pub(crate) message_to_central: [u8; SPLIT_MESSAGE_MAX_SIZE],

    #[characteristic(uuid = "4b3514fb-cae4-4d38-a097-3a2a3d1c3b9c", write)]
    pub(crate) message_to_peripheral: [u8; SPLIT_MESSAGE_MAX_SIZE],
}

pub(crate) async fn run_ble_peripheral_monitor<
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
>(
    id: usize,
    addr: [u8; 6],
) {
    // Channel is used to receive messages from peripheral
    let receive_channel: Channel<CriticalSectionRawMutex, SplitMessage, 8> = Channel::new();
    // Channel is used to notify messages to peripheral
    let notify_channel: Channel<CriticalSectionRawMutex, SplitMessage, 8> = Channel::new();

    let receive_sender = receive_channel.sender();
    let receive_receiver = receive_channel.receiver();
    let notify_sender = notify_channel.sender();
    let notify_receiver = notify_channel.receiver();
    let run_ble_client = run_ble_client(receive_sender, notify_receiver, addr);

    let split_ble_driver = BleSplitCentralDriver {
        receiver: receive_receiver,
        sender: notify_sender,
    };

    let peripheral =
        PeripheralMatrixMonitor::<ROW, COL, ROW_OFFSET, COL_OFFSET, _>::new(split_ble_driver, id);

    info!("Running peripheral monitor {}", id);
    join(peripheral.run(), run_ble_client).await;
}

// If the one peripheral client is connecting, don't try to connect again
static CONNECTING_CLIENT: AtomicBool = AtomicBool::new(false);

/// Run a single ble client, which receives split message from the ble peripheral.
///
/// All received messages are sent to the sender, those message are received in `SplitBleCentralDriver`.
/// Split driver will take `SplitBleCentralDriver` as the reader, process the message in matrix scanning.
pub(crate) async fn run_ble_client(
    receive_sender: Sender<'_, CriticalSectionRawMutex, SplitMessage, 8>,
    notify_receiver: Receiver<'_, CriticalSectionRawMutex, SplitMessage, 8>,
    addr: [u8; 6],
) -> ! {
    // Wait 1s, ensure that the softdevice is ready
    embassy_time::Timer::after_secs(1).await;
    let sd = unsafe { nrf_softdevice::Softdevice::steal() };
    loop {
        let addrs = &[&Address::new(AddressType::RandomStatic, addr)];
        let mut config: central::ConnectConfig<'_> = central::ConnectConfig::default();
        config.conn_params = nrf_softdevice::raw::ble_gap_conn_params_t {
            min_conn_interval: 6,
            max_conn_interval: 6,
            slave_latency: 99,
            conn_sup_timeout: 500, // timeout: 5s
        };
        config.scan_config.whitelist = Some(addrs);
        let conn = loop {
            if let Ok(_) =
                CONNECTING_CLIENT.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            {
                info!("Starting connect to {:?}", addrs);
                let conn = match central::connect(sd, &config).await {
                    Ok(conn) => conn,
                    Err(e) => {
                        error!("BLE peripheral connect error: {:?}", e);
                        CONNECTING_CLIENT.store(false, Ordering::SeqCst);
                        continue;
                    }
                };
                CONNECTING_CLIENT.store(false, Ordering::SeqCst);
                break conn;
            }
            // Wait 200ms and check again
            embassy_time::Timer::after_millis(200).await;
        };

        let ble_client: BleSplitCentralClient = match gatt_client::discover(&conn).await {
            Ok(client) => client,
            Err(e) => {
                error!("BLE discover error: {:?}", e);
                continue;
            }
        };

        // Enable notifications from the peripherals
        if let Err(e) = ble_client.message_to_central_cccd_write(true).await {
            error!("BLE message_to_central_cccd_write error: {:?}", e);
            continue;
        }

        // Receive peripheral's notifications
        let receive_peripheral = gatt_client::run(&conn, &ble_client, |event| match event {
            BleSplitCentralClientEvent::MessageToCentralNotification(message) => {
                match postcard::from_bytes(&message) {
                    Ok(split_message) => {
                        if let Err(e) = receive_sender.try_send(split_message) {
                            error!("BLE_SYNC_CHANNEL send message error: {:?}", e);
                        }
                    }
                    Err(e) => {
                        error!("Postcard deserialize split message error: {}", e);
                    }
                };
            }
        });

        // Notify messages to peripheral
        let notify_peripheral = async {
            loop {
                let mut buf = [0_u8; SPLIT_MESSAGE_MAX_SIZE];
                let message = notify_receiver.receive().await;
                match postcard::to_slice(&message, &mut buf) {
                    Ok(_bytes) => {
                        if let Err(e) = ble_client.message_to_peripheral_write(&buf).await {
                            error!("BLE message_to_peripheral_write error: {:?}", e);
                        }
                    }
                    Err(e) => error!("Postcard serialize split message error: {}", e),
                };
            }
        };

        match select(receive_peripheral, notify_peripheral).await {
            embassy_futures::select::Either::First(e) => {
                error!("BLE peripheral disconnect error: {:?}", e);
            }
            embassy_futures::select::Either::Second(_) => (),
        }

        // Wait for 1s before trying to connect (again)
        embassy_time::Timer::after_secs(1).await;
    }
}

/// Ble central driver which reads and writes the split message.
///
/// Different from serial, BLE split message is processed in a separate service.
/// The BLE service should keep running, it processes the split message in the callback, which is not async.
/// It's impossible to implement `SplitReader` or `SplitWriter` for BLE service,
/// so we need this wrapper to forward split message to channel.
pub(crate) struct BleSplitCentralDriver<'a> {
    // Receiver that receives message from peripheral
    pub(crate) receiver: Receiver<'a, CriticalSectionRawMutex, SplitMessage, 8>,
    // Sender that send message to peripherals
    pub(crate) sender: Sender<'a, CriticalSectionRawMutex, SplitMessage, 8>,
}

impl<'a> SplitReader for BleSplitCentralDriver<'a> {
    async fn read(&mut self) -> Result<SplitMessage, SplitDriverError> {
        Ok(self.receiver.receive().await)
    }
}

impl SplitWriter for BleSplitCentralDriver<'_> {
    async fn write(&mut self, message: &SplitMessage) -> Result<usize, SplitDriverError> {
        self.sender.send(message.clone()).await;
        Ok(SPLIT_MESSAGE_MAX_SIZE)
    }
}
