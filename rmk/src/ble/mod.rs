use core::cell::Cell;
use core::sync::atomic::{AtomicBool, Ordering};

use bt_hci::cmd::le::{LeReadLocalSupportedFeatures, LeSetPhy};
use bt_hci::controller::{ControllerCmdAsync, ControllerCmdSync};
use embassy_futures::join::join;
#[cfg(any(not(feature = "_no_usb"), feature = "passkey_entry"))]
use embassy_futures::select::Either;
use embassy_futures::select::{Either3, select, select3};
use embassy_sync::blocking_mutex::Mutex;
use embassy_time::{Duration, Timer, with_timeout};
#[cfg(feature = "passkey_entry")]
use embassy_time::{Instant, with_deadline};
use rand_core::{CryptoRng, RngCore};
use rmk_types::led_indicator::LedIndicator;
use trouble_host::prelude::appearance::human_interface_device::KEYBOARD;
use trouble_host::prelude::service::{BATTERY, HUMAN_INTERFACE_DEVICE};
use trouble_host::prelude::*;
#[cfg(feature = "host")]
use {crate::ble::host_service::BleHostServer, crate::keymap::KeyMap};
#[cfg(all(feature = "host", not(feature = "_no_usb")))]
use {crate::descriptor::ViaReport, crate::host::UsbHostReaderWriter};
#[cfg(not(feature = "_no_usb"))]
use {
    crate::descriptor::{CompositeReport, KeyboardReport},
    crate::light::UsbLedReader,
    crate::state::get_connection_type,
    crate::usb::{USB_REMOTE_WAKEUP, UsbKeyboardWriter, add_usb_reader_writer, add_usb_writer, new_usb_builder},
    embassy_futures::select::{Either4, select4},
    embassy_usb::driver::Driver,
};
#[cfg(feature = "storage")]
use {
    crate::storage::{Storage, StorageData, StorageKey},
    crate::{read_storage, state::CONNECTION_TYPE},
    embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash,
};

use crate::ble::battery_service::BleBatteryServer;
use crate::ble::ble_server::{BleHidServer, Server};
use crate::ble::device_info::{PnPID, VidSource};
use crate::ble::led::BleLedReader;
use crate::ble::profile::{ProfileInfo, ProfileManager, UPDATED_CCCD_TABLE, UPDATED_PROFILE};
use crate::channel::{KEYBOARD_REPORT_CHANNEL, LED_SIGNAL};
use crate::config::RmkConfig;
use crate::event::{BleStatusChangeEvent, ConnectionChangeEvent, ConnectionType, publish_event};
use crate::hid::{DummyWriter, RunnableHidWriter};
#[cfg(feature = "split")]
use crate::split::ble::central::CENTRAL_SLEEP;
use crate::state::ConnectionState;
#[cfg(feature = "usb_log")]
use crate::usb::add_usb_logger;
use crate::{CONNECTION_STATE, run_keyboard};
pub(crate) mod battery_service;
pub(crate) mod ble_server;
pub(crate) mod device_info;
#[cfg(feature = "host")]
pub(crate) mod host_service;
pub(crate) mod led;
#[cfg(feature = "passkey_entry")]
pub mod passkey;
pub(crate) mod profile;

use rmk_types::ble::{BleState, BleStatus};

/// Global BLE status: tracks the active profile and current BLE state.
pub static BLE_STATUS: Mutex<crate::RawMutex, Cell<BleStatus>> = Mutex::new(Cell::new(BleStatus {
    profile: 0,
    state: BleState::Inactive,
}));

/// Get current profile number
pub(crate) fn get_current_profile() -> u8 {
    BLE_STATUS.lock(|c| c.get().profile)
}

/// Update the BLE status and publish an event.
pub(crate) fn set_ble_status(status: BleStatus) {
    BLE_STATUS.lock(|c| c.set(status));
    publish_event(BleStatusChangeEvent(status));
}

/// Update the BLE state while preserving the current profile.
pub(crate) fn set_ble_state(state: BleState) {
    let profile = get_current_profile();
    let status = BleStatus { profile, state };
    set_ble_status(status);
}

/// Global state of sleep management
/// - `true`: Indicates central is sleeping
/// - `false`: Indicates central is awake
pub(crate) static SLEEPING_STATE: AtomicBool = AtomicBool::new(false);

// TODO: Add documentation about how to define split peripheral num in Rust code
/// Max number of connections
pub(crate) const CONNECTIONS_MAX: usize = crate::SPLIT_PERIPHERALS_NUM + 1;

/// Max number of L2CAP channels
pub(crate) const L2CAP_CHANNELS_MAX: usize = CONNECTIONS_MAX * 4; // Signal + att + smp + hid

#[cfg(feature = "passkey_entry")]
struct PasskeyInputState {
    deadline: Option<Instant>,
    cleanup: Option<crate::ble::passkey::PasskeyCleanupGuard>,
}

#[cfg(feature = "passkey_entry")]
impl PasskeyInputState {
    const fn new() -> Self {
        Self {
            deadline: None,
            cleanup: None,
        }
    }

    fn clear(&mut self) {
        self.deadline = None;
        drop(self.cleanup.take());
    }

    fn begin(&mut self) {
        use crate::ble::passkey::{PasskeyCleanupGuard, begin_passkey_entry_session};

        self.clear();
        begin_passkey_entry_session();
        self.cleanup = Some(PasskeyCleanupGuard::new());
        self.deadline = Some(Instant::now() + Duration::from_secs(crate::PASSKEY_ENTRY_TIMEOUT_SECS as u64));
    }
}

#[cfg(feature = "passkey_entry")]
async fn next_gatt_event<'a, 'b>(
    conn: &GattConnection<'a, 'b, DefaultPacketPool>,
    passkey_state: &mut PasskeyInputState,
) -> Option<GattConnectionEvent<'a, 'b, DefaultPacketPool>> {
    if crate::PASSKEY_ENTRY_ENABLED
        && let Some(deadline) = passkey_state.deadline
    {
        use crate::ble::passkey::PASSKEY_RESPONSE;

        return match select(conn.next(), with_deadline(deadline, PASSKEY_RESPONSE.wait())).await {
            Either::First(event) => Some(event),
            Either::Second(Ok(Some(passkey))) => {
                passkey_state.clear();

                info!("[gatt] Passkey entered: submitting");
                if let Err(e) = conn.raw().pass_key_input(passkey) {
                    error!("[gatt] pass_key_input error: {:?}", e);
                }
                None
            }
            Either::Second(Ok(None)) => {
                passkey_state.clear();

                info!("[gatt] Passkey entry cancelled");
                if let Err(e) = conn.raw().pass_key_cancel() {
                    error!("[gatt] pass_key_cancel error: {:?}", e);
                }
                None
            }
            Either::Second(Err(_)) => {
                passkey_state.clear();

                warn!("[gatt] Passkey entry timeout");
                let _ = conn.raw().pass_key_cancel();
                None
            }
        };
    }

    Some(conn.next().await)
}

/// Build the BLE stack.
pub async fn build_ble_stack<
    'a,
    C: Controller + ControllerCmdAsync<LeSetPhy>,
    P: PacketPool,
    RNG: RngCore + CryptoRng,
>(
    controller: C,
    host_address: [u8; 6],
    random_generator: &mut RNG,
    resources: &'a mut HostResources<P, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX>,
) -> Stack<'a, C, P> {
    // Initialize trouble host stack
    trouble_host::new(controller, resources)
        .set_random_address(Address::random(host_address))
        .set_random_generator_seed(random_generator)
}

#[doc(hidden)]
pub fn passkey_entry_enabled() -> bool {
    #[cfg(feature = "passkey_entry")]
    {
        crate::PASSKEY_ENTRY_ENABLED
    }
    #[cfg(not(feature = "passkey_entry"))]
    {
        false
    }
}

/// Run the BLE stack.
// `'a` is only used by the `host`-cfg-gated keymap parameter, so when that
// feature is disabled clippy sees it as unused. Silence it here.
#[allow(clippy::extra_unused_lifetimes)]
pub(crate) async fn run_ble<
    'a,
    'b,
    C: Controller + ControllerCmdAsync<LeSetPhy> + ControllerCmdSync<LeReadLocalSupportedFeatures>,
    #[cfg(feature = "storage")] F: AsyncNorFlash,
    #[cfg(not(feature = "_no_usb"))] D: Driver<'static>,
    #[cfg(feature = "storage")] const ROW: usize,
    #[cfg(feature = "storage")] const COL: usize,
    #[cfg(feature = "storage")] const NUM_LAYER: usize,
    #[cfg(feature = "storage")] const NUM_ENCODER: usize,
>(
    #[cfg(feature = "host")] keymap: &'a KeyMap<'a>,
    #[cfg(not(feature = "_no_usb"))] usb_driver: D,
    stack: &'b Stack<'b, C, DefaultPacketPool>,
    #[cfg(feature = "storage")] storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    #[cfg_attr(not(feature = "_nrf_ble"), allow(unused_mut))] mut rmk_config: RmkConfig<'static>,
) {
    #[cfg(feature = "_nrf_ble")]
    {
        rmk_config.device_config.serial_number = crate::hid::get_serial_number();
    }

    // Initialize usb device and usb hid reader/writer
    #[cfg(not(feature = "_no_usb"))]
    let (mut _usb_builder, mut keyboard_reader, mut keyboard_writer, mut other_writer) = {
        let mut usb_builder: embassy_usb::Builder<'_, D> = new_usb_builder(usb_driver, rmk_config.device_config);
        let keyboard_reader_writer = add_usb_reader_writer!(&mut usb_builder, KeyboardReport, 1, 8);
        let other_writer = add_usb_writer!(&mut usb_builder, CompositeReport, 9);
        let (keyboard_reader, keyboard_writer) = keyboard_reader_writer.split();
        (usb_builder, keyboard_reader, keyboard_writer, other_writer)
    };

    #[cfg(all(not(feature = "_no_usb"), feature = "host"))]
    let mut host_reader_writer = add_usb_reader_writer!(&mut _usb_builder, ViaReport, 32, 32);

    // Optional usb logger initialization
    #[cfg(all(feature = "usb_log", not(feature = "_no_usb")))]
    let usb_logger = add_usb_logger!(&mut _usb_builder);

    #[cfg(not(feature = "_no_usb"))]
    let mut usb_device = _usb_builder.build();

    // Load current connection type
    #[cfg(feature = "storage")]
    {
        let mut buf: [u8; 16] = [0; 16];
        let key = StorageKey::ConnectionType;
        if let Ok(Some(StorageData::ConnectionType(conn_type))) = read_storage!(storage, &key, buf) {
            CONNECTION_TYPE.store(conn_type, Ordering::SeqCst);
        } else {
            // If no saved connection type, return default value
            #[cfg(feature = "_no_usb")]
            CONNECTION_TYPE.store(ConnectionType::Ble.into(), Ordering::SeqCst);
            #[cfg(not(feature = "_no_usb"))]
            CONNECTION_TYPE.store(ConnectionType::Ble.into(), Ordering::SeqCst);
        }

        CONNECTION_TYPE.store(ConnectionType::Ble.into(), Ordering::SeqCst);
        publish_event(ConnectionChangeEvent::new(
            CONNECTION_TYPE.load(Ordering::SeqCst).into(),
        ));
    }

    // Create profile manager
    let mut profile_manager = ProfileManager::new(stack);

    #[cfg(feature = "storage")]
    // Load saved bonding information
    profile_manager.load_bonded_devices(storage).await;
    // Update bonding information in the stack
    profile_manager.update_stack_bonds();

    // Build trouble host stack
    let Host {
        mut peripheral, runner, ..
    } = stack.build();

    info!("Starting advertising and GATT service");
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: rmk_config.device_config.product_name,
        appearance: &appearance::human_interface_device::KEYBOARD,
    }))
    .unwrap();

    server
        .set(
            &server.device_config_service.pnp_id,
            &PnPID {
                vid_source: VidSource::UsbIF,
                vendor_id: rmk_config.device_config.vid,
                product_id: rmk_config.device_config.pid,
                product_version: 0x0001,
            },
        )
        .unwrap();

    server
        .set(
            &server.device_config_service.serial_number,
            &heapless::String::try_from(rmk_config.device_config.serial_number).unwrap(),
        )
        .unwrap();

    server
        .set(
            &server.device_config_service.manufacturer_name,
            &heapless::String::try_from(rmk_config.device_config.manufacturer).unwrap(),
        )
        .unwrap();

    #[cfg(not(feature = "_no_usb"))]
    let usb_task = async {
        loop {
            usb_device.run_until_suspend().await;
            match select(usb_device.wait_resume(), USB_REMOTE_WAKEUP.wait()).await {
                Either::First(_) => continue,
                Either::Second(_) => {
                    info!("USB wakeup remote");
                    if let Err(e) = usb_device.remote_wakeup().await {
                        info!("USB wakeup remote error: {:?}", e)
                    }
                }
            }
        }
    };

    #[cfg(all(not(feature = "usb_log"), not(feature = "_no_usb")))]
    let background_task = join(ble_task(runner), usb_task);
    #[cfg(all(feature = "usb_log", not(feature = "_no_usb")))]
    let background_task = join(
        ble_task(runner),
        select(
            usb_task,
            embassy_usb_logger::with_class!(1024, log::LevelFilter::Debug, usb_logger),
        ),
    );
    #[cfg(feature = "_no_usb")]
    let background_task = ble_task(runner);

    // Main loop
    join(background_task, async {
        loop {
            // Advertising state
            set_ble_state(BleState::Advertising);
            let adv_fut = advertise(rmk_config.device_config.product_name, &mut peripheral, &server);
            // USB + BLE dual mode
            #[cfg(not(feature = "_no_usb"))]
            {
                match get_connection_type() {
                    ConnectionType::Usb => {
                        use crate::usb::wait_until_usb_configured;

                        info!("USB priority mode, waiting for USB enabled or BLE connection");
                        match select4(
                            wait_until_usb_configured(),
                            adv_fut,
                            #[cfg(feature = "storage")]
                            run_dummy_keyboard(storage),
                            #[cfg(not(feature = "storage"))]
                            run_dummy_keyboard(),
                            profile_manager.update_profile(),
                        )
                        .await
                        {
                            Either4::First(_) => {
                                use crate::usb::wait_until_usb_disabled;

                                info!("USB enabled, run USB keyboard");

                                set_ble_state(BleState::Inactive);
                                let usb_fut = run_keyboard(
                                    #[cfg(feature = "storage")]
                                    storage,
                                    #[cfg(feature = "host")]
                                    keymap,
                                    #[cfg(feature = "host")]
                                    UsbHostReaderWriter::new(&mut host_reader_writer),
                                    #[cfg(feature = "vial")]
                                    rmk_config.vial_config,
                                    wait_until_usb_disabled(),
                                    UsbLedReader::new(&mut keyboard_reader),
                                    UsbKeyboardWriter::new(&mut keyboard_writer, &mut other_writer),
                                );
                                select(usb_fut, profile_manager.update_profile()).await;
                            }
                            Either4::Second(Ok(conn)) => {
                                info!("No USB, BLE connected, run BLE keyboard");
                                let ble_fut = run_ble_keyboard(
                                    &server,
                                    &conn,
                                    stack,
                                    #[cfg(feature = "host")]
                                    keymap,
                                    #[cfg(feature = "host")]
                                    &mut rmk_config,
                                    #[cfg(feature = "storage")]
                                    storage,
                                );
                                select3(ble_fut, wait_until_usb_configured(), profile_manager.update_profile()).await;
                                continue;
                            }
                            Either4::Second(Err(BleHostError::BleHost(Error::Timeout))) => {
                                warn!("Advertising timeout, sleep and wait for any key");

                                set_ble_state(BleState::Inactive);
                                // Set CONNECTION_STATE to true to keep receiving messages from the peripheral
                                CONNECTION_STATE.store(ConnectionState::Connected.into(), Ordering::Release);

                                // Enter sleep mode to reduce the power consumption
                                #[cfg(feature = "split")]
                                CENTRAL_SLEEP.signal(true);

                                // Wait for the keyboard report for wake the keyboard
                                let _ = KEYBOARD_REPORT_CHANNEL.receive().await;

                                // Quit from sleep mode
                                #[cfg(feature = "split")]
                                CENTRAL_SLEEP.signal(false);
                                continue;
                            }
                            _ => {}
                        }
                    }
                    ConnectionType::Ble => {
                        info!("BLE priority mode, running USB keyboard while advertising");
                        let usb_fut = run_keyboard(
                            #[cfg(feature = "storage")]
                            storage,
                            #[cfg(feature = "host")]
                            keymap,
                            #[cfg(feature = "host")]
                            UsbHostReaderWriter::new(&mut host_reader_writer),
                            #[cfg(feature = "vial")]
                            rmk_config.vial_config,
                            core::future::pending::<()>(), // Run forever until BLE connected
                            UsbLedReader::new(&mut keyboard_reader),
                            UsbKeyboardWriter::new(&mut keyboard_writer, &mut other_writer),
                        );
                        match select3(adv_fut, usb_fut, profile_manager.update_profile()).await {
                            Either3::First(Ok(conn)) => {
                                info!("BLE connected, running BLE keyboard");
                                select(
                                    run_ble_keyboard(
                                        &server,
                                        &conn,
                                        stack,
                                        #[cfg(feature = "host")]
                                        keymap,
                                        #[cfg(feature = "host")]
                                        &mut rmk_config,
                                        #[cfg(feature = "storage")]
                                        storage,
                                    ),
                                    profile_manager.update_profile(),
                                )
                                .await;
                            }
                            Either3::First(Err(BleHostError::BleHost(Error::Timeout))) => {
                                warn!("Advertising timeout, sleep and wait for any key");

                                set_ble_state(BleState::Inactive);
                                // Set CONNECTION_STATE to true to keep receiving messages from the peripheral
                                CONNECTION_STATE.store(ConnectionState::Connected.into(), Ordering::Release);

                                // Enter sleep mode to reduce the power consumption
                                #[cfg(feature = "split")]
                                CENTRAL_SLEEP.signal(true);

                                // Wait for the keyboard report for wake the keyboard
                                let _ = KEYBOARD_REPORT_CHANNEL.receive().await;

                                // Quit from sleep mode
                                #[cfg(feature = "split")]
                                CENTRAL_SLEEP.signal(false);

                                continue;
                            }
                            _ => {}
                        }
                    }
                }
            }

            #[cfg(feature = "_no_usb")]
            match adv_fut.await {
                Ok(conn) => {
                    // BLE connected
                    select(
                        run_ble_keyboard(
                            &server,
                            &conn,
                            &stack,
                            #[cfg(feature = "host")]
                            keymap,
                            #[cfg(feature = "host")]
                            &mut rmk_config,
                            #[cfg(feature = "storage")]
                            storage,
                        ),
                        profile_manager.update_profile(),
                    )
                    .await;
                }
                Err(BleHostError::BleHost(Error::Timeout)) => {
                    warn!("Advertising timeout, sleep and wait for any key");

                    set_ble_state(BleState::Inactive);
                    // Set CONNECTION_STATE to true to keep receiving messages from the peripheral
                    CONNECTION_STATE.store(ConnectionState::Connected.into(), Ordering::Release);

                    // Enter sleep mode to reduce the power consumption
                    #[cfg(feature = "split")]
                    CENTRAL_SLEEP.signal(true);

                    // Wait for the keyboard report for wake the keyboard
                    let _ = KEYBOARD_REPORT_CHANNEL.receive().await;

                    // Quit from sleep mode
                    #[cfg(feature = "split")]
                    CENTRAL_SLEEP.signal(false);
                    continue;
                }
                Err(e) => {
                    #[cfg(feature = "defmt")]
                    let e = defmt::Debug2Format(&e);
                    error!("Advertise error: {:?}", e);
                }
            }

            // Retry after 200 ms
            Timer::after_millis(200).await;
        }
    })
    .await;
}

/// This is a background task that is required to run forever alongside any other BLE tasks.
pub(crate) async fn ble_task<C: Controller + ControllerCmdAsync<LeSetPhy>, P: PacketPool>(
    mut runner: Runner<'_, C, P>,
) {
    loop {
        #[cfg(not(feature = "split"))]
        if let Err(_e) = runner.run().await {
            error!("[ble_task] runner.run() error");
            embassy_time::Timer::after_millis(100).await;
        }

        #[cfg(feature = "split")]
        {
            // Signal to indicate the stack is started
            crate::split::ble::central::STACK_STARTED.signal(true);
            if let Err(_e) = runner
                .run_with_handler(&crate::split::ble::central::ScanHandler {})
                .await
            {
                error!("[ble_task] runner.run_with_handler error");
                embassy_time::Timer::after_millis(100).await;
            }
        }
    }
}

/// Stream Events until the connection closes.
///
/// This function will handle the GATT events and process them.
/// This is how we interact with read and write requests.
async fn gatt_events_task(server: &Server<'_>, conn: &GattConnection<'_, '_, DefaultPacketPool>) -> Result<(), Error> {
    let level = server.battery_service.level;
    let output_keyboard = server.hid_service.output_keyboard;
    let hid_control_point = server.hid_service.hid_control_point;
    let input_keyboard = server.hid_service.input_keyboard;
    #[cfg(feature = "host")]
    let output_host = server.host_service.output_data;
    #[cfg(feature = "host")]
    let input_host = server.host_service.input_data;
    #[cfg(feature = "host")]
    let host_control_point = server.host_service.hid_control_point;
    let battery_level = server.battery_service.level;
    let mouse = server.composite_service.mouse_report;
    let media = server.composite_service.media_report;
    let media_control_point = server.composite_service.hid_control_point;
    let system_control = server.composite_service.system_report;

    CONNECTION_STATE.store(ConnectionState::Connected.into(), Ordering::Release);

    let mut connected = false;
    let mut published_connected_state = false;
    #[cfg(feature = "passkey_entry")]
    let mut passkey_state = PasskeyInputState::new();

    loop {
        #[cfg(feature = "passkey_entry")]
        let Some(event) = next_gatt_event(conn, &mut passkey_state).await else {
            continue;
        };
        #[cfg(not(feature = "passkey_entry"))]
        let event = conn.next().await;

        match event {
            GattConnectionEvent::Disconnected { reason } => {
                #[cfg(feature = "passkey_entry")]
                passkey_state.clear();
                info!("[gatt] disconnected: {:?}", reason);
                break;
            }
            GattConnectionEvent::PairingComplete { security_level, bond } => {
                #[cfg(feature = "passkey_entry")]
                passkey_state.clear();
                info!("[gatt] pairing complete: {:?}", security_level);
                let profile = get_current_profile();
                if let Some(bond_info) = bond {
                    let profile_info = ProfileInfo {
                        slot_num: profile,
                        info: bond_info,
                        removed: false,
                        cccd_table: server.get_cccd_table(conn.raw()).unwrap(),
                    };
                    UPDATED_PROFILE.signal(profile_info);
                }

                {
                    connected = true;
                }
            }
            GattConnectionEvent::PairingFailed(err) => {
                #[cfg(feature = "passkey_entry")]
                passkey_state.clear();
                error!("[gatt] pairing error: {:?}", err);
            }
            GattConnectionEvent::Gatt { event: gatt_event } => {
                let mut cccd_updated = false;
                let result = match &gatt_event {
                    GattEvent::Read(event) => {
                        if event.handle() == level.handle {
                            let value = server.get(&level);
                            debug!("Read GATT Event to Level: {:?}", value);
                        } else {
                            debug!("Read GATT Event to Unknown: {:?}", event.handle());
                        }

                        if conn.raw().security_level()?.encrypted() {
                            None
                        } else {
                            Some(AttErrorCode::INSUFFICIENT_ENCRYPTION)
                        }
                    }
                    GattEvent::Write(event) => {
                        if event.handle() == output_keyboard.handle {
                            if event.data().len() == 1 {
                                let led_indicator = LedIndicator::from_bits(event.data()[0]);
                                debug!("Got keyboard state: {:?}", led_indicator);
                                LED_SIGNAL.signal(led_indicator);
                            } else {
                                warn!("Wrong keyboard state data: {:?}", event.data());
                            }
                        } else if event.handle() == input_keyboard.cccd_handle.expect("No CCCD for input keyboard")
                            || event.handle() == mouse.cccd_handle.expect("No CCCD for mouse report")
                            || event.handle() == media.cccd_handle.expect("No CCCD for media report")
                            || event.handle() == system_control.cccd_handle.expect("No CCCD for system report")
                            || event.handle() == battery_level.cccd_handle.expect("No CCCD for battery level")
                        {
                            // CCCD write event
                            cccd_updated = true;
                        } else if event.handle() == hid_control_point.handle
                            || event.handle() == media_control_point.handle
                        {
                            info!("Write GATT Event to Control Point: {:?}", event.handle());
                            #[cfg(feature = "split")]
                            if event.data().len() == 1 {
                                let data = event.data()[0];
                                if data == 0 {
                                    // Enter sleep mode
                                    CENTRAL_SLEEP.signal(true);
                                } else if data == 1 {
                                    // Wake up
                                    CENTRAL_SLEEP.signal(false);
                                }
                            }
                        } else {
                            #[cfg(feature = "host")]
                            if event.handle() == output_host.handle {
                                debug!("Got host packet: {:?}", event.data());
                                if event.data().len() == 32 {
                                    use crate::ble::host_service::HOST_GUI_INPUT_CHANNEL;

                                    let mut data = [0u8; 32];
                                    data.copy_from_slice(event.data());
                                    HOST_GUI_INPUT_CHANNEL.send(data).await;
                                } else {
                                    warn!("Wrong host packet data: {:?}", event.data());
                                }
                            } else if event.handle() == input_host.cccd_handle.expect("No CCCD for input host") {
                                // CCCD write event
                                cccd_updated = true;
                            } else if event.handle() == host_control_point.handle {
                                info!("Write GATT Event to Control Point: {:?}", event.handle());
                                #[cfg(feature = "split")]
                                if event.data().len() == 1 {
                                    let data = event.data()[0];
                                    if data == 0 {
                                        // Enter sleep mode
                                        CENTRAL_SLEEP.signal(true);
                                    } else if data == 1 {
                                        // Wake up
                                        CENTRAL_SLEEP.signal(false);
                                    }
                                }
                            } else {
                                debug!("Write GATT Event to Unknown: {:?}", event.handle());
                            }
                            #[cfg(not(feature = "host"))]
                            debug!("Write GATT Event to Unknown: {:?}", event.handle());
                        }

                        if conn.raw().security_level()?.encrypted() {
                            None
                        } else {
                            Some(AttErrorCode::INSUFFICIENT_ENCRYPTION)
                        }
                    }
                    GattEvent::Other(_) => None,
                    GattEvent::NotAllowed(_) => None,
                };

                // This step is also performed at drop(), but writing it explicitly is necessary
                // in order to ensure reply is sent.
                let result = if let Some(code) = result {
                    gatt_event.reject(code)
                } else {
                    gatt_event.accept()
                };
                match result {
                    Ok(reply) => reply.send().await,
                    Err(e) => warn!("[gatt] error sending response: {:?}", e),
                }

                // Update CCCD table after processing the event
                if cccd_updated {
                    // When macOS wakes up from sleep mode, it won't send EXIT SUSPEND command
                    // So we need to monitor the sleep state by using CCCD write event
                    #[cfg(feature = "split")]
                    CENTRAL_SLEEP.signal(false);

                    if let Some(table) = server.get_cccd_table(conn.raw()) {
                        UPDATED_CCCD_TABLE.signal(table);
                    }
                }
            }
            GattConnectionEvent::PhyUpdated { tx_phy, rx_phy } => {
                info!("[gatt] PhyUpdated: {:?}, {:?}", tx_phy, rx_phy)
            }
            GattConnectionEvent::ConnectionParamsUpdated {
                conn_interval,
                peripheral_latency,
                supervision_timeout,
            } => {
                {
                    connected = true;
                }
                info!(
                    "[gatt] ConnectionParamsUpdated: {:?}ms, {:?}, {:?}ms",
                    conn_interval.as_millis(),
                    peripheral_latency,
                    supervision_timeout.as_millis()
                );
            }
            GattConnectionEvent::RequestConnectionParams(req) => info!(
                "[gatt] RequestConnectionParams: interval: ({:?}, {:?})ms, {:?}, {:?}ms",
                req.params().min_connection_interval.as_millis(),
                req.params().max_connection_interval.as_millis(),
                req.params().max_latency,
                req.params().supervision_timeout.as_millis(),
            ),
            GattConnectionEvent::DataLengthUpdated {
                max_tx_octets,
                max_tx_time,
                max_rx_octets,
                max_rx_time,
            } => {
                {
                    connected = true;
                }
                info!(
                    "[gatt] DataLengthUpdated: tx/rx octets: ({:?}, {:?}), tx/rx time: ({:?}, {:?})",
                    max_tx_octets, max_rx_octets, max_tx_time, max_rx_time
                );
            }
            GattConnectionEvent::FrameSpaceUpdated {
                frame_space,
                initiator,
                phys,
                spacing_types,
            } => {
                {
                    connected = true;
                }
                info!(
                    "[gatt] FrameSpaceUpdated: {:?}, {:?}, {:?}, {:?}",
                    frame_space, initiator, phys, spacing_types
                );
            }
            GattConnectionEvent::ConnectionRateChanged {
                conn_interval,
                subrate_factor,
                peripheral_latency,
                continuation_number,
                supervision_timeout,
            } => {
                {
                    connected = true;
                }
                info!(
                    "[gatt] ConnectionRateChanged: {:?}ms, {:?}, {:?}, {:?}, {:?}ms",
                    conn_interval.as_millis(),
                    subrate_factor,
                    peripheral_latency,
                    continuation_number,
                    supervision_timeout.as_millis()
                );
            }
            GattConnectionEvent::PassKeyDisplay(pass_key) => info!("[gatt] PassKeyDisplay: {:?}", pass_key),
            GattConnectionEvent::PassKeyConfirm(pass_key) => info!("[gatt] PassKeyConfirm: {:?}", pass_key),
            GattConnectionEvent::PassKeyInput => {
                #[cfg(feature = "passkey_entry")]
                if crate::PASSKEY_ENTRY_ENABLED {
                    info!("[gatt] PassKeyInput: entering passkey entry mode");
                    passkey_state.begin();
                } else {
                    warn!("[gatt] PassKeyInput: disabled in config, cancelling pairing, this shouldn't happen");
                    if let Err(e) = conn.raw().pass_key_cancel() {
                        error!("[gatt] pass_key_cancel error: {:?}", e);
                    }
                }
                #[cfg(not(feature = "passkey_entry"))]
                warn!("[gatt] PassKeyInput event, should not happen")
            }
            GattConnectionEvent::BondLost => warn!("[gatt] BondLost"),
        }

        // Publish the BLE connected event

        if connected && !published_connected_state {
            set_ble_state(BleState::Connected);
            published_connected_state = true;
        }
    }
    info!("[gatt] task finished");
    Ok(())
}

/// Create an advertiser to use to connect to a BLE Central, and wait for it to connect.
async fn advertise<'a, 'b, C: Controller>(
    name: &'a str,
    peripheral: &mut Peripheral<'a, C, DefaultPacketPool>,
    server: &'b Server<'_>,
) -> Result<GattConnection<'a, 'b, DefaultPacketPool>, BleHostError<C::Error>> {
    // Wait for 10ms to ensure the USB is checked
    embassy_time::Timer::after_millis(10).await;
    let mut advertiser_data = [0; 31];
    AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::CompleteServiceUuids16(&[BATTERY.to_le_bytes(), HUMAN_INTERFACE_DEVICE.to_le_bytes()]),
            AdStructure::CompleteLocalName(name.as_bytes()),
            AdStructure::Unknown {
                ty: 0x19, // Appearance
                data: &KEYBOARD.to_le_bytes(),
            },
        ],
        &mut advertiser_data[..],
    )?;

    let advertise_config = AdvertisementParameters {
        primary_phy: PhyKind::Le2M,
        secondary_phy: PhyKind::Le2M,
        tx_power: TxPower::Plus8dBm,
        interval_min: Duration::from_millis(200),
        interval_max: Duration::from_millis(200),
        ..Default::default()
    };

    info!("[adv] advertising");
    let advertiser = peripheral
        .advertise(
            &advertise_config,
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..],
                scan_data: &[],
            },
        )
        .await?;

    // Timeout for advertising is 300s
    match with_timeout(Duration::from_secs(300), advertiser.accept()).await {
        Ok(conn_res) => {
            let conn = conn_res?.with_attribute_server(server)?;
            info!("[adv] connection established");
            if let Err(e) = conn.raw().set_bondable(true) {
                error!("Set bondable error: {:?}", e);
            };
            Ok(conn)
        }
        Err(_) => Err(BleHostError::BleHost(Error::Timeout)),
    }
}

// Dummy keyboard service is used to monitoring keys when there's no actual connection.
// It's useful for functions like switching active profiles when there's no connection.
pub(crate) async fn run_dummy_keyboard<
    #[cfg(feature = "storage")] F: AsyncNorFlash,
    #[cfg(feature = "storage")] const ROW: usize,
    #[cfg(feature = "storage")] const COL: usize,
    #[cfg(feature = "storage")] const NUM_LAYER: usize,
    #[cfg(feature = "storage")] const NUM_ENCODER: usize,
>(
    #[cfg(feature = "storage")] storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
) {
    CONNECTION_STATE.store(ConnectionState::Disconnected.into(), Ordering::Release);
    #[cfg(feature = "storage")]
    let storage_fut = storage.run();
    let mut dummy_writer = DummyWriter {};
    #[cfg(feature = "storage")]
    select(storage_fut, dummy_writer.run_writer()).await;
    #[cfg(not(feature = "storage"))]
    dummy_writer.run_writer().await;
}

pub(crate) async fn set_conn_params<
    'a,
    'b,
    C: Controller + ControllerCmdSync<LeReadLocalSupportedFeatures>,
    P: PacketPool,
>(
    stack: &Stack<'_, C, P>,
    conn: &GattConnection<'a, 'b, P>,
) {
    // Wait for 5 seconds before setting connection parameters to avoid connection drop
    embassy_time::Timer::after_secs(5).await;

    // For macOS/iOS(aka Apple devices), both interval should be set to 15ms
    // Reference: https://developer.apple.com/accessories/Accessory-Design-Guidelines.pdf
    update_conn_params(
        stack,
        conn.raw(),
        &RequestedConnParams {
            min_connection_interval: Duration::from_millis(15),
            max_connection_interval: Duration::from_millis(15),
            max_latency: 30,
            min_event_length: Duration::from_secs(0),
            max_event_length: Duration::from_secs(0),
            supervision_timeout: Duration::from_secs(5),
        },
    )
    .await;

    embassy_time::Timer::after_secs(5).await;

    // Setting the conn param the second time ensures that we have best performance on all platforms
    update_conn_params(
        stack,
        conn.raw(),
        &RequestedConnParams {
            min_connection_interval: Duration::from_micros(7500),
            max_connection_interval: Duration::from_micros(7500),
            max_latency: 30,
            min_event_length: Duration::from_secs(0),
            max_event_length: Duration::from_secs(0),
            supervision_timeout: Duration::from_secs(5),
        },
    )
    .await;

    // Wait forever. This is because we want the conn params setting can be interrupted when the connection is lost.
    // So this task shouldn't quit after setting the conn params.
    core::future::pending::<()>().await;
}

/// Run BLE keyboard with connected device
// `'c` / `'d` are only used by `host`-cfg-gated parameters; silence the
// unused-lifetime lint when the host feature is disabled.
#[allow(clippy::extra_unused_lifetimes)]
async fn run_ble_keyboard<
    'a,
    'b,
    'c,
    'd,
    C: Controller + ControllerCmdAsync<LeSetPhy> + ControllerCmdSync<LeReadLocalSupportedFeatures>,
    #[cfg(feature = "storage")] F: AsyncNorFlash,
    #[cfg(feature = "storage")] const ROW: usize,
    #[cfg(feature = "storage")] const COL: usize,
    #[cfg(feature = "storage")] const NUM_LAYER: usize,
    #[cfg(feature = "storage")] const NUM_ENCODER: usize,
>(
    server: &'b Server<'_>,
    conn: &GattConnection<'a, 'b, DefaultPacketPool>,
    stack: &Stack<'_, C, DefaultPacketPool>,
    #[cfg(feature = "host")] keymap: &'c KeyMap<'c>,
    #[cfg(feature = "host")] rmk_config: &'d mut RmkConfig<'static>,
    #[cfg(feature = "storage")] storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
) {
    let ble_hid_server = BleHidServer::new(server, conn);
    #[cfg(feature = "host")]
    let ble_host_server = BleHostServer::new(server, conn);
    let ble_led_reader = BleLedReader {};
    let mut ble_battery_server = BleBatteryServer::new(server, conn);

    // Load CCCD table from storage
    #[cfg(feature = "storage")]
    if let Ok(Some(bond_info)) = storage.read_trouble_bond_info(get_current_profile()).await
        && bond_info.info.identity.match_identity(&conn.raw().peer_identity())
    {
        info!("Loading CCCD table from storage: {:?}", bond_info.cccd_table);
        server.set_cccd_table(conn.raw(), bond_info.cccd_table.clone());
    }

    // Use 2M Phy
    update_ble_phy(stack, conn.raw()).await;

    let communication_task = async {
        if let Either3::First(e) = select3(
            gatt_events_task(server, conn),
            set_conn_params(stack, conn),
            ble_battery_server.run(),
        )
        .await
        {
            error!("[gatt_events_task] end: {:?}", e)
        }
    };

    run_keyboard(
        #[cfg(feature = "storage")]
        storage,
        #[cfg(feature = "host")]
        keymap,
        #[cfg(feature = "host")]
        ble_host_server,
        #[cfg(feature = "vial")]
        rmk_config.vial_config,
        communication_task,
        ble_led_reader,
        ble_hid_server,
    )
    .await;
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use super::{BLE_STATUS, BleState, BleStatus, set_ble_state, set_ble_status};

    fn ble_status_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn set_ble_state_preserves_current_profile() {
        let _guard = ble_status_test_lock().lock().unwrap();

        BLE_STATUS.lock(|c| {
            c.set(BleStatus {
                profile: 2,
                state: BleState::Inactive,
            })
        });
        set_ble_state(BleState::Advertising);

        assert_eq!(
            BLE_STATUS.lock(|c| c.get()),
            BleStatus {
                profile: 2,
                state: BleState::Advertising,
            }
        );
    }

    #[test]
    fn set_ble_status_can_reset_state_when_profile_changes() {
        let _guard = ble_status_test_lock().lock().unwrap();

        BLE_STATUS.lock(|c| {
            c.set(BleStatus {
                profile: 1,
                state: BleState::Connected,
            })
        });
        set_ble_status(BleStatus {
            profile: 3,
            state: BleState::Inactive,
        });

        assert_eq!(
            BLE_STATUS.lock(|c| c.get()),
            BleStatus {
                profile: 3,
                state: BleState::Inactive,
            }
        );
    }
}

// Update the PHY to 2M
pub(crate) async fn update_ble_phy<P: PacketPool>(
    stack: &Stack<'_, impl Controller + ControllerCmdAsync<LeSetPhy>, P>,
    conn: &Connection<'_, P>,
) {
    loop {
        match conn.set_phy(stack, PhyKind::Le2M).await {
            Err(BleHostError::BleHost(Error::Hci(error))) => {
                if 0x2A == error.to_status().into_inner() {
                    // Busy, retry
                    info!("[update_ble_phy] HCI busy: {:?}", error);
                    continue;
                } else {
                    error!("[update_ble_phy] HCI error: {:?}", error);
                }
            }
            Err(e) => {
                #[cfg(feature = "defmt")]
                let e = defmt::Debug2Format(&e);
                error!("[update_ble_phy] error: {:?}", e);
            }
            Ok(_) => {
                info!("[update_ble_phy] PHY updated");
            }
        }
        break;
    }
}

// Update the connection parameters
pub(crate) async fn update_conn_params<
    'a,
    'b,
    C: Controller + ControllerCmdSync<LeReadLocalSupportedFeatures>,
    P: PacketPool,
>(
    stack: &Stack<'a, C, P>,
    conn: &Connection<'b, P>,
    params: &RequestedConnParams,
) {
    loop {
        match conn.update_connection_params(stack, params).await {
            Err(BleHostError::BleHost(Error::Hci(error))) => {
                if 0x3A == error.to_status().into_inner() {
                    // Busy, retry
                    info!("[update_conn_params] HCI busy: {:?}", error);
                    embassy_time::Timer::after_millis(100).await;
                    continue;
                } else {
                    error!("[update_conn_params] HCI error: {:?}", error);
                }
            }
            Err(e) => {
                #[cfg(feature = "defmt")]
                let e = defmt::Debug2Format(&e);
                error!("[update_conn_params] BLE host error: {:?}", e);
            }
            _ => (),
        }
        break;
    }
}
