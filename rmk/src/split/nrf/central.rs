use core::{cell::RefCell, sync::atomic::Ordering};

use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_futures::{
    join::join,
    select::{select, Either},
};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver, Sender},
};
use embassy_time::Timer;
use embassy_usb::driver::Driver;
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
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

#[cfg(not(feature = "rapid_debouncer"))]
use crate::debounce::default_bouncer::DefaultDebouncer;
#[cfg(feature = "rapid_debouncer")]
use crate::debounce::fast_debouncer::RapidDebouncer;
use crate::{
    action::KeyAction,
    ble::nrf::{
        advertise::{create_advertisement_data, SCAN_DATA},
        bonder::{BondInfo, Bonder},
        nrf_ble_config, run_ble_keyboard,
        server::BleServer,
        softdevice_task, BONDED_DEVICE_NUM,
    },
    keyboard::{keyboard_report_channel, Keyboard},
    keymap::KeyMap,
    light::LightService,
    run_usb_keyboard,
    split::{
        driver::{PeripheralMatrixMonitor, SplitDriverError, SplitReader},
        SplitMessage, SPLIT_MESSAGE_MAX_SIZE,
    },
    storage::{get_bond_info_key, Storage, StorageData},
    usb::{wait_for_usb_enabled, wait_for_usb_suspend, KeyboardUsbDevice, USB_DEVICE_ENABLED},
    via::process::VialService,
};
use crate::{debounce::DebouncerTrait, split::central::CentralMatrix};

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
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
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
    #[cfg(feature = "col2row")] input_pins: [In; CENTRAL_ROW],
    #[cfg(not(feature = "col2row"))] input_pins: [In; CENTRAL_COL],
    #[cfg(feature = "col2row")] output_pins: [Out; CENTRAL_COL],
    #[cfg(not(feature = "col2row"))] output_pins: [Out; CENTRAL_ROW],
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

    // Get all saved bond info, config BLE bonder
    let mut buf: [u8; 128] = [0; 128];
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
    static BONDER: StaticCell<Bonder> = StaticCell::new();
    let bonder = BONDER.init(Bonder::new(RefCell::new(bond_info)));

    let ble_server = defmt::unwrap!(BleServer::new(sd, keyboard_config.usb_config, bonder));

    // Keyboard matrix, use COL2ROW by default
    #[cfg(all(feature = "col2row", feature = "rapid_debouncer"))]
    let debouncer: RapidDebouncer<CENTRAL_ROW, CENTRAL_COL> = RapidDebouncer::new();
    #[cfg(all(not(feature = "col2row"), feature = "rapid_debouncer"))]
    let debouncer: RapidDebouncer<CENTRAL_COL, CENTRAL_ROW> = RapidDebouncer::new();
    #[cfg(all(feature = "col2row", not(feature = "rapid_debouncer")))]
    let debouncer: DefaultDebouncer<CENTRAL_ROW, CENTRAL_COL> = DefaultDebouncer::new();
    #[cfg(all(not(feature = "col2row"), not(feature = "rapid_debouncer")))]
    let debouncer: DefaultDebouncer<CENTRAL_COL, CENTRAL_ROW> = DefaultDebouncer::new();

    #[cfg(feature = "col2row")]
    let mut matrix = CentralMatrix::<
        In,
        Out,
        _,
        CENTRAL_ROW_OFFSET,
        CENTRAL_COL_OFFSET,
        CENTRAL_ROW,
        CENTRAL_COL,
    >::new(input_pins, output_pins, debouncer);
    #[cfg(not(feature = "col2row"))]
    let mut matrix = CentralMatrix::<
        In,
        Out,
        _,
        CENTRAL_ROW_OFFSET,
        CENTRAL_COL_OFFSET,
        CENTRAL_COL,
        CENTRAL_ROW,
    >::new(input_pins, output_pins, debouncer);

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
        let adv_fut = peripheral::advertise_pairable(sd, adv, &config, bonder);

        // If there is a USB device, things become a little bit complex because we need to enable switching between USB and BLE.
        // Remember that USB ALWAYS has higher priority than BLE.
        #[cfg(not(feature = "_no_usb"))]
        {
            // Check and run via USB first
            if USB_DEVICE_ENABLED.load(Ordering::SeqCst) {
                // Run usb keyboard
                let usb_fut = run_usb_keyboard(
                    &mut usb_device,
                    &mut keyboard,
                    &mut matrix,
                    &mut storage,
                    &mut light_service,
                    &mut vial_service,
                    &keyboard_report_receiver,
                );
                info!("Running USB keyboard!");
                select(usb_fut, wait_for_usb_suspend()).await;
            }

            // Usb device have to be started to check if usb is configured
            info!("USB suspended, BLE Advertising");

            // Wait for BLE or USB connection
            match select(adv_fut, wait_for_usb_enabled()).await {
                Either::First(re) => match re {
                    Ok(conn) => {
                        info!("Connected to BLE");
                        bonder.load_sys_attrs(&conn);
                        match select(
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
                        )
                        .await
                        {
                            Either::First(_) => (),
                            Either::Second(_) => {
                                info!("Detected USB configured, quit BLE");
                                continue;
                            }
                        }
                    }
                    Err(e) => error!("Advertise error: {}", e),
                },
                Either::Second(_) => {
                    // Wait 10ms for usb resuming
                    Timer::after_millis(10).await;
                    continue;
                }
            }
        }

        #[cfg(feature = "_no_usb")]
        match adv_fut.await {
            Ok(conn) => {
                bonder.load_sys_attrs(&conn);
                run_ble_keyboard(
                    &conn,
                    &ble_server,
                    &mut keyboard,
                    &mut matrix,
                    &mut storage,
                    &mut light_service,
                    &mut keyboard_config.ble_battery_config,
                    &keyboard_report_receiver,
                )
                .await
            }
            Err(e) => error!("Advertise error: {}", e),
        }

        // Retry after 1 second
        Timer::after_secs(1).await;
    }
}
