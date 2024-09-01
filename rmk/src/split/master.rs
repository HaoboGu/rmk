use crate::action::KeyAction;
use crate::ble::nrf::advertise::create_advertisement_data;
use crate::ble::nrf::advertise::SCAN_DATA;
use crate::ble::nrf::bonder::BondInfo;
use crate::ble::nrf::bonder::Bonder;
use crate::ble::nrf::nrf_ble_config;
use crate::ble::nrf::run_ble_keyboard;
use crate::ble::nrf::server::BleServer;
use crate::ble::nrf::softdevice_task;
use crate::ble::nrf::BONDED_DEVICE_NUM;
#[cfg(not(feature = "rapid_debouncer"))]
use crate::debounce::default_bouncer::DefaultDebouncer;
#[cfg(feature = "rapid_debouncer")]
use crate::debounce::fast_debouncer::RapidDebouncer;
use crate::debounce::{DebounceState, DebouncerTrait};
use crate::keyboard::{communication_task, Keyboard, KeyboardReportMessage};
use crate::keymap::KeyMap;
use crate::matrix::{KeyState, MatrixTrait};
use crate::run_usb_keyboard;
use crate::split::driver::nrf_ble::run_ble_client;
// use crate::split::driver::nrf_ble::run_ble_slave_monitor;
use crate::split::driver::SplitMasterReceiver;
use crate::split::KeySyncSignal;
use crate::split::SplitMessage;
use crate::split::SYNC_SIGNALS;
use crate::storage::get_bond_info_key;
use crate::storage::Storage;
use crate::storage::StorageData;
use crate::usb::wait_for_usb_configured;
use crate::usb::wait_for_usb_suspend;
use crate::usb::KeyboardUsbDevice;
use crate::usb::USB_DEVICE_ENABLED;
use crate::via::process::VialService;
use crate::{
    keyboard::keyboard_task,
    light::{led_hid_task, LightService},
    via::vial_task,
};
use core::cell::RefCell;
use core::sync::atomic::Ordering;
use defmt::{error, info, warn};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_futures::select::Either;
use embassy_futures::select::{select, select4, Either4};
use embassy_futures::yield_now;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Instant, Timer};
use embassy_usb::driver::Driver;
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
use embedded_io_async::{Read, Write};
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
use futures::pin_mut;
use heapless::FnvIndexMap;
use nrf_softdevice::ble::peripheral;
use nrf_softdevice::ble::security::SecurityHandler;
use nrf_softdevice::Flash;
use nrf_softdevice::Softdevice;
use rmk_config::RmkConfig;
use sequential_storage::cache::NoCache;
use sequential_storage::map::fetch_item;
use static_cell::StaticCell;

use super::driver::nrf_ble::SplitBleMasterDriver;
use super::driver::serial::SerialSplitDriver;
use super::{KeySyncMessage, MASTER_SYNC_CHANNELS};

/// Initialize and run the keyboard service, with given keyboard usb config. This function never returns.
///
/// # Arguments
///
/// * `driver` - embassy usb driver instance
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `flash` - optional **async** flash storage, which is used for storing keymap and keyboard configs
/// * `keymap` - default keymap definition
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
pub async fn initialize_split_ble_master_and_run<
    D: Driver<'static>,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    const TOTAL_ROW: usize,
    const TOTAL_COL: usize,
    const MASTER_ROW: usize,
    const MASTER_COL: usize,
    const MASTER_ROW_OFFSET: usize,
    const MASTER_COL_OFFSET: usize,
    const NUM_LAYER: usize,
>(
    #[cfg(any(feature = "nrf52840_ble", feature = "nrf52833_ble"))] usb_driver: Option<D>,
    #[cfg(feature = "col2row")] input_pins: [In; MASTER_ROW],
    #[cfg(not(feature = "col2row"))] input_pins: [In; MASTER_COL],
    #[cfg(feature = "col2row")] output_pins: [Out; MASTER_COL],
    #[cfg(not(feature = "col2row"))] output_pins: [Out; MASTER_ROW],
    default_keymap: [[[KeyAction; TOTAL_COL]; TOTAL_ROW]; NUM_LAYER],
    mut keyboard_config: RmkConfig<'static, Out>,
    spawner: Spawner,
) -> ! {
    // Set ble config and start nrf-softdevice background task first
    let keyboard_name = keyboard_config.usb_config.product_name;
    let ble_config = nrf_ble_config(keyboard_name);

    let sd = Softdevice::enable(&ble_config);
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
    let debouncer: RapidDebouncer<MASTER_ROW, MASTER_COL> = RapidDebouncer::new();
    #[cfg(all(not(feature = "col2row"), feature = "rapid_debouncer"))]
    let debouncer: RapidDebouncer<MASTER_COL, MASTER_ROW> = RapidDebouncer::new();
    #[cfg(all(feature = "col2row", not(feature = "rapid_debouncer")))]
    let debouncer: DefaultDebouncer<MASTER_ROW, MASTER_COL> = DefaultDebouncer::new();
    #[cfg(all(not(feature = "col2row"), not(feature = "rapid_debouncer")))]
    let debouncer: DefaultDebouncer<MASTER_COL, MASTER_ROW> = DefaultDebouncer::new();

    #[cfg(feature = "col2row")]
    let matrix = MasterMatrix::<
        In,
        Out,
        _,
        TOTAL_ROW,
        TOTAL_COL,
        MASTER_ROW_OFFSET,
        MASTER_COL_OFFSET,
        MASTER_ROW,
        MASTER_COL,
    >::new(input_pins, output_pins, debouncer);
    #[cfg(not(feature = "col2row"))]
    let matrix = MasterMatrix::<
        In,
        Out,
        _,
        TOTAL_ROW,
        TOTAL_COL,
        MASTER_ROW_OFFSET,
        MASTER_COL_OFFSET,
        MASTER_COL,
        MASTER_ROW,
    >::new(input_pins, output_pins, debouncer);

    // Keyboard services
    let mut keyboard = Keyboard::new(matrix, &keymap);
    #[cfg(any(feature = "nrf52840_ble", feature = "nrf52833_ble"))]
    let (mut usb_device, mut vial_service) = (
        usb_driver.map(|u| KeyboardUsbDevice::new(u, keyboard_config.usb_config)),
        VialService::new(&keymap, keyboard_config.vial_config),
    );

    let mut light_service = LightService::from_config(keyboard_config.light_config);

    static keyboard_channel: Channel<CriticalSectionRawMutex, KeyboardReportMessage, 8> =
        Channel::new();
    let mut keyboard_report_sender = keyboard_channel.sender();
    let mut keyboard_report_receiver = keyboard_channel.receiver();

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
        #[cfg(any(feature = "nrf52840_ble", feature = "nrf52833_ble"))]
        if let Some(ref mut usb_device) = usb_device {
            // Check and run via USB first
            if USB_DEVICE_ENABLED.load(Ordering::SeqCst) {
                let usb_fut = run_usb_keyboard(
                    usb_device,
                    &mut keyboard,
                    &mut storage,
                    &mut light_service,
                    &mut vial_service,
                    &mut keyboard_report_receiver,
                    &mut keyboard_report_sender,
                );
                info!("Running USB keyboard!");
                select(usb_fut, wait_for_usb_suspend()).await;
            }

            // Usb device have to be started to check if usb is configured
            let usb_fut = usb_device.device.run();
            let usb_configured = wait_for_usb_configured();
            info!("USB suspended, BLE Advertising");

            // Wait for BLE or USB connection
            match select(adv_fut, select(usb_fut, usb_configured)).await {
                Either::First(re) => match re {
                    Ok(conn) => {
                        info!("Connected to BLE");
                        bonder.load_sys_attrs(&conn);
                        let usb_configured = wait_for_usb_configured();
                        let usb_fut = usb_device.device.run();
                        match select(
                            run_ble_keyboard(
                                &conn,
                                &ble_server,
                                &mut keyboard,
                                &mut storage,
                                &mut light_service,
                                &mut keyboard_config.ble_battery_config,
                                &mut keyboard_report_receiver,
                                &mut keyboard_report_sender,
                            ),
                            select(usb_fut, usb_configured),
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
        } else {
            // If no USB device, just start BLE advertising
            info!("No USB, Start BLE advertising!");
            match adv_fut.await {
                Ok(conn) => {
                    bonder.load_sys_attrs(&conn);
                    run_ble_keyboard(
                        &conn,
                        &ble_server,
                        &mut keyboard,
                        &mut storage,
                        &mut light_service,
                        &mut keyboard_config.ble_battery_config,
                        &mut keyboard_report_receiver,
                        &mut keyboard_report_sender,
                    )
                    .await
                }
                Err(e) => error!("Advertise error: {}", e),
            }
        }

        #[cfg(any(
            feature = "nrf52832_ble",
            feature = "nrf52811_ble",
            feature = "nrf52810_ble"
        ))]
        match adv_fut.await {
            Ok(conn) => {
                bonder.load_sys_attrs(&conn);
                run_ble_keyboard(
                    &conn,
                    &ble_server,
                    &mut keyboard,
                    &mut storage,
                    &mut light_service,
                    &mut keyboard_config.ble_battery_config,
                    &mut keyboard_report_receiver,
                    &mut keyboard_report_sender,
                )
                .await
            }
            Err(e) => error!("Advertise error: {}", e),
        }
        // Retry after 3 second
        Timer::after_millis(100).await;
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
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
pub async fn initialize_split_master_and_run<
    F: AsyncNorFlash,
    D: Driver<'static>,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    const TOTAL_ROW: usize,
    const TOTAL_COL: usize,
    const MASTER_ROW: usize,
    const MASTER_COL: usize,
    const MASTER_ROW_OFFSET: usize,
    const MASTER_COL_OFFSET: usize,
    const NUM_LAYER: usize,
>(
    driver: D,
    #[cfg(feature = "col2row")] input_pins: [In; MASTER_ROW],
    #[cfg(not(feature = "col2row"))] input_pins: [In; MASTER_COL],
    #[cfg(feature = "col2row")] output_pins: [Out; MASTER_COL],
    #[cfg(not(feature = "col2row"))] output_pins: [Out; MASTER_ROW],
    flash: Option<F>,
    default_keymap: [[[KeyAction; TOTAL_COL]; TOTAL_ROW]; NUM_LAYER],
    keyboard_config: RmkConfig<'static, Out>,
) -> ! {
    // Initialize storage and keymap
    let (mut storage, keymap) = match flash {
        Some(f) => {
            let mut s = Storage::new(f, &default_keymap, keyboard_config.storage_config).await;
            let keymap = RefCell::new(
                KeyMap::<TOTAL_ROW, TOTAL_COL, NUM_LAYER>::new_from_storage(
                    default_keymap,
                    Some(&mut s),
                )
                .await,
            );
            (Some(s), keymap)
        }
        None => {
            let keymap = RefCell::new(
                KeyMap::<TOTAL_ROW, TOTAL_COL, NUM_LAYER>::new_from_storage::<F>(
                    default_keymap,
                    None,
                )
                .await,
            );
            (None, keymap)
        }
    };

    static keyboard_channel: Channel<CriticalSectionRawMutex, KeyboardReportMessage, 8> =
        Channel::new();
    let mut keyboard_report_sender = keyboard_channel.sender();
    let mut keyboard_report_receiver = keyboard_channel.receiver();

    // Keyboard matrix, use COL2ROW by default
    #[cfg(all(feature = "col2row", feature = "rapid_debouncer"))]
    let debouncer: RapidDebouncer<MASTER_ROW, MASTER_COL> = RapidDebouncer::new();
    #[cfg(all(not(feature = "col2row"), feature = "rapid_debouncer"))]
    let debouncer: RapidDebouncer<MASTER_COL, MASTER_ROW> = RapidDebouncer::new();
    #[cfg(all(feature = "col2row", not(feature = "rapid_debouncer")))]
    let debouncer: DefaultDebouncer<MASTER_ROW, MASTER_COL> = DefaultDebouncer::new();
    #[cfg(all(not(feature = "col2row"), not(feature = "rapid_debouncer")))]
    let debouncer: DefaultDebouncer<MASTER_COL, MASTER_ROW> = DefaultDebouncer::new();

    #[cfg(feature = "col2row")]
    let matrix = MasterMatrix::<
        In,
        Out,
        _,
        TOTAL_ROW,
        TOTAL_COL,
        MASTER_ROW_OFFSET,
        MASTER_COL_OFFSET,
        MASTER_ROW,
        MASTER_COL,
    >::new(input_pins, output_pins, debouncer);
    #[cfg(not(feature = "col2row"))]
    let matrix = MasterMatrix::<
        In,
        Out,
        _,
        TOTAL_ROW,
        TOTAL_COL,
        MASTER_ROW_OFFSET,
        MASTER_COL_OFFSET,
        MASTER_COL,
        MASTER_ROW,
    >::new(input_pins, output_pins, debouncer);

    // Create keyboard services and devices
    let (mut keyboard, mut usb_device, mut vial_service, mut light_service) = (
        Keyboard::new(matrix, &keymap),
        KeyboardUsbDevice::new(driver, keyboard_config.usb_config),
        VialService::new(&keymap, keyboard_config.vial_config),
        LightService::from_config(keyboard_config.light_config),
    );

    loop {
        // Run all tasks, if one of them fails, wait 1 second and then restart
        if let Some(ref mut s) = storage {
            run_usb_keyboard(
                &mut usb_device,
                &mut keyboard,
                s,
                &mut light_service,
                &mut vial_service,
                &mut keyboard_report_receiver,
                &mut keyboard_report_sender,
            )
            .await;
        } else {
            // Run 5 tasks: usb, keyboard, led, vial, communication
            let usb_fut = usb_device.device.run();
            let keyboard_fut = keyboard_task(&mut keyboard, &mut keyboard_report_sender);
            let communication_fut = communication_task(
                &mut keyboard_report_receiver,
                &mut usb_device.keyboard_hid_writer,
                &mut usb_device.other_hid_writer,
            );
            let led_fut = led_hid_task(&mut usb_device.keyboard_hid_reader, &mut light_service);
            let via_fut = vial_task(&mut usb_device.via_hid, &mut vial_service);
            // let slave_fut = select_slice(&mut slave_futs);
            pin_mut!(usb_fut);
            pin_mut!(keyboard_fut);
            pin_mut!(led_fut);
            pin_mut!(via_fut);
            pin_mut!(communication_fut);
            match select4(
                usb_fut,
                select(keyboard_fut, communication_fut),
                // select(led_fut, slave_fut),
                led_fut,
                via_fut,
            )
            .await
            {
                Either4::First(_) => {
                    error!("Usb task is died");
                }
                Either4::Second(_) => error!("Keyboard task is died"),
                Either4::Third(_) => error!("Led task is died"),
                Either4::Fourth(_) => error!("Via task is died"),
            }
        }

        warn!("Detected failure, restarting keyboard sevice after 1 second");
        Timer::after_secs(1).await;
    }
}

/// Receive split message from slave via serial and process it
///
/// Generic parameters:
/// - `const ROW`: row number of the slave's matrix
/// - `const COL`: column number of the slave's matrix
/// - `const ROW_OFFSET`: row offset of the slave's matrix in the whole matrix
/// - `const COL_OFFSET`: column offset of the slave's matrix in the whole matrix
/// - `S`: a serial port that implements `Read` and `Write` trait in embedded-io-async
pub async fn run_serial_slave_monitor<
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    S: Read + Write,
>(
    receiver: S,
    id: usize,
) {
    let split_serial_driver = SerialSplitDriver::new(receiver);
    let slave =
        SplitMasterReceiver::<ROW, COL, ROW_OFFSET, COL_OFFSET, _>::new(split_serial_driver, id);
    info!("Running slave monitor {}", id);
    slave.run().await;
}

pub async fn run_ble_slave_monitor<
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
>(
    id: usize,
    addr: [u8; 6],
) {
    embassy_time::Timer::after_secs(10).await;
    let channel: Channel<CriticalSectionRawMutex, SplitMessage, 8> = Channel::new();

    let sender = channel.sender();
    let ble_client_run = run_ble_client(sender, addr);

    let receiver = channel.receiver();
    let split_ble_driver = SplitBleMasterDriver { receiver };

    let slave =
        SplitMasterReceiver::<ROW, COL, ROW_OFFSET, COL_OFFSET, _>::new(split_ble_driver, id);

    info!("Running slave monitor {}", id);
    join(slave.run(), ble_client_run).await;
}

/// Matrix is the physical pcb layout of the keyboard matrix.
pub(crate) struct MasterMatrix<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    D: DebouncerTrait,
    const TOTAL_ROW: usize,
    const TOTAL_COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    const INPUT_PIN_NUM: usize,
    const OUTPUT_PIN_NUM: usize,
> {
    /// Input pins of the pcb matrix
    input_pins: [In; INPUT_PIN_NUM],
    /// Output pins of the pcb matrix
    output_pins: [Out; OUTPUT_PIN_NUM],
    /// Debouncer
    debouncer: D,
    /// Key state matrix
    key_states: [[KeyState; TOTAL_COL]; TOTAL_ROW],
    /// Start scanning
    scan_start: Option<Instant>,
}

impl<
        #[cfg(feature = "async_matrix")] In: Wait + InputPin,
        #[cfg(not(feature = "async_matrix"))] In: InputPin,
        Out: OutputPin,
        D: DebouncerTrait,
        const ROW: usize,
        const COL: usize,
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        const INPUT_PIN_NUM: usize,
        const OUTPUT_PIN_NUM: usize,
    > MatrixTrait
    for MasterMatrix<In, Out, D, ROW, COL, ROW_OFFSET, COL_OFFSET, INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    async fn scan(&mut self) {
        self.internal_scan().await;
        self.scan_slave().await;
    }

    fn get_key_state(&mut self, row: usize, col: usize) -> KeyState {
        self.key_states[row][col]
    }

    fn update_key_state(&mut self, row: usize, col: usize, f: impl FnOnce(&mut KeyState)) {
        f(&mut self.key_states[row][col]);
    }

    #[cfg(feature = "async_matrix")]
    async fn wait_for_key(&mut self) {
        todo!()
    }
}

impl<
        #[cfg(feature = "async_matrix")] In: Wait + InputPin,
        #[cfg(not(feature = "async_matrix"))] In: InputPin,
        Out: OutputPin,
        D: DebouncerTrait,
        const ROW: usize,
        const COL: usize,
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        const INPUT_PIN_NUM: usize,
        const OUTPUT_PIN_NUM: usize,
    > MasterMatrix<In, Out, D, ROW, COL, ROW_OFFSET, COL_OFFSET, INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    /// Initialization of master
    pub(crate) fn new(
        input_pins: [In; INPUT_PIN_NUM],
        output_pins: [Out; OUTPUT_PIN_NUM],
        debouncer: D,
    ) -> Self {
        MasterMatrix {
            input_pins,
            output_pins,
            debouncer,
            key_states: [[KeyState::default(); COL]; ROW],
            scan_start: None,
        }
    }

    pub(crate) async fn scan_slave(&mut self) {
        for (id, slave_channel) in MASTER_SYNC_CHANNELS.iter().enumerate() {
            // TODO: Skip unused slaves
            if id > 0 {
                break;
            }
            // Signal that slave scanning is started
            SYNC_SIGNALS[id].signal(KeySyncSignal::Start);
            // Receive slave key states
            if let KeySyncMessage::StartSend(n) = slave_channel.receive().await {
                // Update slave's key states
                for _ in 0..n {
                    if let KeySyncMessage::Key(row, col, key_state) = slave_channel.receive().await
                    {
                        if key_state != self.key_states[row as usize][col as usize].pressed {
                            self.key_states[row as usize][col as usize].pressed = key_state;
                            self.key_states[row as usize][col as usize].changed = true;
                        } else {
                            self.key_states[row as usize][col as usize].changed = false;
                        }
                    }
                }
            }
        }
    }

    pub(crate) async fn internal_scan(&mut self) {
        // Get the row and col index of current board in the whole key matrix
        for (out_idx, out_pin) in self.output_pins.iter_mut().enumerate() {
            // Pull up output pin, wait 1us ensuring the change comes into effect
            out_pin.set_high().ok();
            Timer::after_micros(1).await;
            for (in_idx, in_pin) in self.input_pins.iter_mut().enumerate() {
                #[cfg(feature = "col2row")]
                let (row_idx, col_idx) = (in_idx + ROW_OFFSET, out_idx + COL_OFFSET);
                #[cfg(not(feature = "col2row"))]
                let (row_idx, col_idx) = (out_idx + ROW_OFFSET, in_idx + COL_OFFSET);

                // Check input pins and debounce
                let debounce_state = self.debouncer.detect_change_with_debounce(
                    in_idx,
                    out_idx,
                    in_pin.is_high().ok().unwrap_or_default(),
                    &self.key_states[row_idx][col_idx],
                );

                match debounce_state {
                    DebounceState::Debounced => {
                        self.key_states[row_idx][col_idx].toggle_pressed();
                        self.key_states[row_idx][col_idx].changed = true;
                    }
                    _ => self.key_states[row_idx][col_idx].changed = false,
                }

                // If there's key changed or pressed, always refresh the self.scan_start
                if self.key_states[row_idx][col_idx].changed
                    || self.key_states[row_idx][col_idx].pressed
                {
                    #[cfg(feature = "async_matrix")]
                    {
                        self.scan_start = Some(Instant::now());
                    }
                }
            }
            out_pin.set_low().ok();
        }
    }

    /// Read key state OF CURRENT BOARD at position (row, col)
    pub(crate) fn get_key_state_current_board(
        &mut self,
        out_idx: usize,
        in_idx: usize,
    ) -> KeyState {
        #[cfg(feature = "col2row")]
        return self.key_states[in_idx + ROW_OFFSET][out_idx + COL_OFFSET];
        #[cfg(not(feature = "col2row"))]
        return self.key_states[out_idx + ROW_OFFSET][in_idx + COL_OFFSET];
    }
}
