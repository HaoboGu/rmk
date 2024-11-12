use core::{cell::RefCell, sync::atomic::Ordering};

use defmt::{debug, error, info};
use embassy_executor::Spawner;
use embassy_futures::{
    join::join,
    select::{select3, Either3},
};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver, Sender},
};
use embassy_time::Timer;
use embassy_usb::driver::Driver;
use embedded_hal::digital::OutputPin;
use heapless::FnvIndexMap;
use nrf_softdevice::{
    ble::{
        central, gatt_client, peripheral, security::SecurityHandler, set_address, Address,
        AddressType,
    },
    Flash, Softdevice,
};
use rmk_config::RmkConfig;
use sequential_storage::{cache::NoCache, map::fetch_item};
use static_cell::StaticCell;

use crate::{
    action::KeyAction,
    ble::nrf::{
        advertise::{create_advertisement_data, SCAN_DATA},
        bonder::{BondInfo, MultiBonder},
        nrf_ble_config,
        profile::update_profile,
        run_ble_keyboard, run_dummy_keyboard,
        server::BleServer,
        softdevice_task, wait_for_status_change, ACTIVE_PROFILE, BONDED_DEVICE_NUM,
    },
    keyboard::{keyboard_report_channel, Keyboard},
    keymap::KeyMap,
    light::LightService,
    matrix::MatrixTrait,
    run_usb_keyboard,
    split::{
        driver::{PeripheralMatrixMonitor, SplitDriverError, SplitReader},
        SplitMessage, SPLIT_MESSAGE_MAX_SIZE,
    },
    storage::{get_bond_info_key, Storage, StorageData, StorageKeys},
    usb::{wait_for_usb_enabled, wait_for_usb_suspend, KeyboardUsbDevice, UsbState, USB_STATE},
    via::process::VialService,
    CONNECTION_TYPE,
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
    // embassy_time::Timer::after_secs(10).await;
    let channel: Channel<CriticalSectionRawMutex, SplitMessage, 8> = Channel::new();

    let sender = channel.sender();
    let run_ble_client = run_ble_client(sender, addr);

    let receiver = channel.receiver();
    let split_ble_driver = BleSplitCentralDriver { receiver };

    let peripheral =
        PeripheralMatrixMonitor::<ROW, COL, ROW_OFFSET, COL_OFFSET, _>::new(split_ble_driver, id);

    info!("Running peripheral monitor {}", id);
    join(peripheral.run(), run_ble_client).await;
}

/// Run a single ble client, which receives split message from the ble peripheral.
///
/// All received messages are sent to the sender, those message are received in `SplitBleCentralDriver`.
/// Split driver will take `SplitBleCentralDriver` as the reader, process the message in matrix scanning.
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

        let ble_client: BleSplitCentralClient = match gatt_client::discover(&conn).await {
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

        // Receive peripheral's notifications
        let disconnect_error = gatt_client::run(&conn, &ble_client, |event| match event {
            BleSplitCentralClientEvent::MessageToCentralNotification(message) => {
                match postcard::from_bytes(&message) {
                    Ok(split_message) => {
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

/// Ble central driver which reads the split message.
///
/// Different from serial, BLE split message is received and processed in a separate service.
/// The BLE service should keep running, it sends out the split message to the channel in the callback.
/// It's impossible to implement `SplitReader` for BLE service,
/// so we need this thin wrapper that receives the message from the channel.
pub(crate) struct BleSplitCentralDriver<'a> {
    pub(crate) receiver: Receiver<'a, CriticalSectionRawMutex, SplitMessage, 8>,
}

impl<'a> SplitReader for BleSplitCentralDriver<'a> {
    async fn read(&mut self) -> Result<SplitMessage, SplitDriverError> {
        Ok(self.receiver.receive().await)
    }
}

/// Initialize and run the keyboard service, with given keyboard usb config. This function never returns.
///
/// # Arguments
///
/// * `driver` - embassy usb driver instance
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `flash` - optional **async** flash storage, which is used for storing keymap and keyboard configs
/// * `keymap` - default keymap definition
/// * `central_addr` - BLE random static address of central
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
pub(crate) async fn initialize_ble_split_central_and_run<
    M: MatrixTrait,
    Out: OutputPin,
    #[cfg(not(feature = "_no_usb"))] D: Driver<'static>,
    const TOTAL_ROW: usize,
    const TOTAL_COL: usize,
    const CENTRAL_ROW: usize,
    const CENTRAL_COL: usize,
    const CENTRAL_ROW_OFFSET: usize,
    const CENTRAL_COL_OFFSET: usize,
    const NUM_LAYER: usize,
>(
    mut matrix: M,
    #[cfg(not(feature = "_no_usb"))] usb_driver: D,
    default_keymap: &mut [[[KeyAction; TOTAL_COL]; TOTAL_ROW]; NUM_LAYER],
    mut keyboard_config: RmkConfig<'static, Out>,
    central_addr: [u8; 6],
    spawner: Spawner,
) -> ! {
    // Set ble config and start nrf-softdevice background task first
    let keyboard_name = keyboard_config.usb_config.product_name;
    let ble_config = nrf_ble_config(keyboard_name);

    let sd = Softdevice::enable(&ble_config);
    set_address(sd, &Address::new(AddressType::RandomStatic, central_addr));

    {
        // Use the immutable ref of `Softdevice` to run the softdevice_task
        // The mumtable ref is used for configuring Flash and BleServer
        let sdv = unsafe { nrf_softdevice::Softdevice::steal() };
        defmt::unwrap!(spawner.spawn(softdevice_task(sdv)))
    };

    // Flash and keymap configuration
    let flash = Flash::take(sd);
    let mut storage = Storage::new(flash, &default_keymap, keyboard_config.storage_config).await;
    let keymap = RefCell::new(
        KeyMap::<TOTAL_ROW, TOTAL_COL, NUM_LAYER>::new_from_storage(
            default_keymap,
            Some(&mut storage),
        )
        .await,
    );

    let mut buf: [u8; 128] = [0; 128];
    // Load current active profile
    if let Ok(Some(StorageData::ActiveBleProfile(profile))) =
        fetch_item::<u32, StorageData<TOTAL_ROW, TOTAL_COL, NUM_LAYER>, _>(
            &mut storage.flash,
            storage.storage_range.clone(),
            &mut NoCache::new(),
            &mut buf,
            &(StorageKeys::ActiveBleProfile as u32),
        )
        .await
    {
        ACTIVE_PROFILE.store(profile, Ordering::SeqCst);
    } else {
        // If no saved active profile, use 0 as default
        ACTIVE_PROFILE.store(0, Ordering::SeqCst);
    };

    // Load current connection type
    if let Ok(Some(StorageData::ConnectionType(conn_type))) =
        fetch_item::<u32, StorageData<TOTAL_ROW, TOTAL_COL, NUM_LAYER>, _>(
            &mut storage.flash,
            storage.storage_range.clone(),
            &mut NoCache::new(),
            &mut buf,
            &(StorageKeys::ConnectionType as u32),
        )
        .await
    {
        CONNECTION_TYPE.store(conn_type, Ordering::Relaxed);
    } else {
        // If no saved connection type, use 0 as default
        CONNECTION_TYPE.store(0, Ordering::Relaxed);
    };

    #[cfg(feature = "_no_usb")]
    CONNECTION_TYPE.store(0, Ordering::Relaxed);

    // Get all saved bond info, config BLE bonder
    let mut bond_info: FnvIndexMap<u8, BondInfo, BONDED_DEVICE_NUM> = FnvIndexMap::new();
    for key in 0..BONDED_DEVICE_NUM {
        if let Ok(Some(StorageData::BondInfo(info))) =
            fetch_item::<u32, StorageData<TOTAL_ROW, TOTAL_COL, NUM_LAYER>, _>(
                &mut storage.flash,
                storage.storage_range.clone(),
                &mut NoCache::new(),
                &mut buf,
                &get_bond_info_key(key as u8),
            )
            .await
        {
            bond_info.insert(key as u8, info).ok();
        }
    }
    info!("Loaded {} saved bond info", bond_info.len());
    // static BONDER: StaticCell<Bonder> = StaticCell::new();
    // let bonder = BONDER.init(Bonder::new(RefCell::new(bond_info)));
    static BONDER: StaticCell<MultiBonder> = StaticCell::new();
    let bonder = BONDER.init(MultiBonder::new(RefCell::new(bond_info)));

    let ble_server = defmt::unwrap!(BleServer::new(sd, keyboard_config.usb_config, bonder));

    let keyboard_report_sender = keyboard_report_channel.sender();
    let keyboard_report_receiver = keyboard_report_channel.receiver();

    // Keyboard services
    let mut keyboard = Keyboard::new(&keymap, &keyboard_report_sender);
    #[cfg(not(feature = "_no_usb"))]
    let (mut usb_device, mut vial_service) = (
        KeyboardUsbDevice::new(usb_driver, keyboard_config.usb_config),
        VialService::new(&keymap, keyboard_config.vial_config),
    );
    let mut light_service = LightService::from_config(keyboard_config.light_config);

    // Main loop
    loop {
        // Init BLE advertising data
        let mut config = peripheral::Config::default();
        // Interval: 500ms
        config.interval = 800;
        let adv_data = create_advertisement_data(keyboard_name);
        let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
            adv_data: &adv_data,
            scan_data: &SCAN_DATA,
        };

        // If there is a USB device, things become a little bit complex because we need to enable switching between USB and BLE.
        // Remember that USB ALWAYS has higher priority than BLE.
        #[cfg(not(feature = "_no_usb"))]
        {
            debug!(
                "usb state: {}, connection type: {}",
                USB_STATE.load(Ordering::SeqCst),
                CONNECTION_TYPE.load(Ordering::Relaxed)
            );
            // Check whether the USB is connected
            if USB_STATE.load(Ordering::SeqCst) != UsbState::Disabled as u8 {
                let usb_fut = run_usb_keyboard(
                    &mut usb_device,
                    &mut keyboard,
                    &mut matrix,
                    &mut storage,
                    &mut light_service,
                    &mut vial_service,
                    &keyboard_report_receiver,
                );
                if CONNECTION_TYPE.load(Ordering::Relaxed) == 0 {
                    info!("Running USB keyboard");
                    // USB is connected, connection_type is USB, then run USB keyboard
                    match select3(usb_fut, wait_for_usb_suspend(), update_profile(bonder)).await {
                        Either3::Third(_) => {
                            Timer::after_millis(10).await;
                            continue;
                        }
                        _ => (),
                    }
                } else {
                    // USB is connected, but connection type is BLE, try BLE while running USB keyboard
                    info!("Running USB keyboard, while advertising");
                    let adv_fut = peripheral::advertise_pairable(sd, adv, &config, bonder);
                    // TODO: Test power consumption in this case
                    match select3(adv_fut, usb_fut, update_profile(bonder)).await {
                        Either3::First(Ok(conn)) => {
                            info!("Connected to BLE");
                            // Check whether the peer address is matched with current profile
                            if !bonder.check_connection(&conn) {
                                error!(
                                    "Bonded peer address doesn't match active profile, disconnect"
                                );
                                continue;
                            }
                            bonder.load_sys_attrs(&conn);
                            // Run the ble keyboard, wait for disconnection or USB connect
                            match select3(
                                run_ble_keyboard(
                                    &conn,
                                    &ble_server,
                                    &mut keyboard,
                                    &mut matrix,
                                    &mut storage,
                                    &mut light_service,
                                    &mut vial_service,
                                    &mut keyboard_config.ble_battery_config,
                                    &keyboard_report_receiver,
                                ),
                                wait_for_usb_enabled(),
                                update_profile(bonder),
                            )
                            .await
                            {
                                Either3::First(_) => info!("BLE disconnected"),
                                Either3::Second(_) => info!("Detected USB configured, quit BLE"),
                                Either3::Third(_) => info!("Switch profile"),
                            }
                        }
                        _ => {
                            // Wait 10ms
                            Timer::after_millis(10).await;
                            continue;
                        }
                    }
                }
            } else {
                // USB isn't connected, wait for any of BLE/USB connection
                let dummy_task = run_dummy_keyboard(
                    &mut keyboard,
                    &mut matrix,
                    &mut storage,
                    &keyboard_report_receiver,
                );
                let adv_fut = peripheral::advertise_pairable(sd, adv, &config, bonder);

                info!("BLE advertising");
                // Wait for BLE or USB connection
                match select3(adv_fut, wait_for_status_change(bonder), dummy_task).await {
                    Either3::First(Ok(conn)) => {
                        info!("Connected to BLE");
                        // Check whether the peer address is matched with current profile
                        if !bonder.check_connection(&conn) {
                            error!("Bonded peer address doesn't match active profile, disconnect");
                            continue;
                        }
                        bonder.load_sys_attrs(&conn);
                        // Run the ble keyboard, wait for disconnection
                        match select3(
                            run_ble_keyboard(
                                &conn,
                                &ble_server,
                                &mut keyboard,
                                &mut matrix,
                                &mut storage,
                                &mut light_service,
                                &mut vial_service,
                                &mut keyboard_config.ble_battery_config,
                                &keyboard_report_receiver,
                            ),
                            wait_for_usb_enabled(),
                            update_profile(bonder),
                        )
                        .await
                        {
                            Either3::First(_) => info!("BLE disconnected"),
                            Either3::Second(_) => info!("Detected USB configured, quit BLE"),
                            Either3::Third(_) => info!("Switch profile"),
                        }
                    }
                    _ => {
                        // Wait 10ms for usb resuming/switching profile/advertising error
                        Timer::after_millis(10).await;
                    }
                }
            }
        }

        #[cfg(feature = "_no_usb")]
        match peripheral::advertise_pairable(sd, adv, &config, bonder).await {
            Ok(conn) => {
                bonder.load_sys_attrs(&conn);
                select(
                    run_ble_keyboard(
                        &conn,
                        &ble_server,
                        &mut keyboard,
                        &mut matrix,
                        &mut storage,
                        &mut light_service,
                        &mut vial_service,
                        &mut keyboard_config.ble_battery_config,
                        &keyboard_report_receiver,
                    ),
                    update_profile(bonder),
                )
                .await;
            }
            Err(e) => error!("Advertise error: {}", e),
        }

        // Retry after 200 ms
        Timer::after_millis(200).await;
    }
}
