use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use battery_service::BleBatteryServer;
use ble_server::{BleHidServer, BleViaServer, Server};
use bt_hci::cmd::le::{LeReadLocalSupportedFeatures, LeSetPhy};
use bt_hci::controller::{ControllerCmdAsync, ControllerCmdSync};
use device_info::{PnPID, VidSource};
use embassy_futures::join::join;
use embassy_futures::select::{select, select3, Either3};
use embassy_time::{with_timeout, Duration, Timer};
use embedded_hal::digital::OutputPin;
use profile::{ProfileInfo, ProfileManager, UPDATED_CCCD_TABLE, UPDATED_PROFILE};
use rand_core::{CryptoRng, RngCore};
use trouble_host::prelude::appearance::human_interface_device::KEYBOARD;
use trouble_host::prelude::service::{BATTERY, HUMAN_INTERFACE_DEVICE};
use trouble_host::prelude::*;
#[cfg(feature = "controller")]
use {
    crate::channel::{send_controller_event, CONTROLLER_CHANNEL},
    crate::event::ControllerEvent,
};
#[cfg(not(feature = "_no_usb"))]
use {
    crate::descriptor::{CompositeReport, KeyboardReport, ViaReport},
    crate::light::UsbLedReader,
    crate::state::get_connection_type,
    crate::usb::UsbKeyboardWriter,
    crate::usb::{add_usb_reader_writer, add_usb_writer, new_usb_builder},
    crate::usb::{USB_ENABLED, USB_REMOTE_WAKEUP, USB_SUSPENDED},
    crate::via::UsbVialReaderWriter,
    embassy_futures::select::{select4, Either, Either4},
    embassy_usb::driver::Driver,
};
#[cfg(feature = "storage")]
use {
    crate::storage::{Storage, StorageData, StorageKeys},
    crate::{read_storage, state::CONNECTION_TYPE},
    embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash,
};

use crate::ble::led::BleLedReader;
use crate::channel::{KEYBOARD_REPORT_CHANNEL, LED_SIGNAL, VIAL_READ_CHANNEL};
use crate::config::RmkConfig;
use crate::hid::{DummyWriter, RunnableHidWriter};
use crate::keymap::KeyMap;
use crate::light::{LedIndicator, LightController};
#[cfg(feature = "split")]
use crate::split::ble::central::CENTRAL_SLEEP;
use crate::state::{ConnectionState, ConnectionType};
#[cfg(feature = "usb_log")]
use crate::usb::add_usb_logger;
use crate::{run_keyboard, CONNECTION_STATE};

pub(crate) mod battery_service;
pub(crate) mod ble_server;
pub(crate) mod device_info;
pub(crate) mod profile;

/// The number of the active profile
pub static ACTIVE_PROFILE: AtomicU8 = AtomicU8::new(0);

/// Global state of sleep management
/// - `true`: Indicates central is sleeping
/// - `false`: Indicates central is awake
pub(crate) static SLEEPING_STATE: AtomicBool = AtomicBool::new(false);

/// Max number of connections
pub(crate) const CONNECTIONS_MAX: usize = 4; // Should be number of the peripheral + 1?

/// Max number of L2CAP channels
pub(crate) const L2CAP_CHANNELS_MAX: usize = CONNECTIONS_MAX * 4; // Signal + att + smp + hid

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

/// Run the BLE stack.
pub(crate) async fn run_ble<
    'a,
    'b,
    C: Controller + ControllerCmdAsync<LeSetPhy> + ControllerCmdSync<LeReadLocalSupportedFeatures>,
    #[cfg(feature = "storage")] F: AsyncNorFlash,
    #[cfg(not(feature = "_no_usb"))] D: Driver<'static>,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    #[cfg(not(feature = "_no_usb"))] usb_driver: D,
    stack: &'b Stack<'b, C, DefaultPacketPool>,
    #[cfg(feature = "storage")] storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    light_controller: &mut LightController<Out>,
    mut rmk_config: RmkConfig<'static>,
) {
    #[cfg(feature = "_nrf_ble")]
    {
        rmk_config.usb_config.serial_number = crate::hid::get_serial_number();
    }

    // Initialize usb device and usb hid reader/writer
    #[cfg(not(feature = "_no_usb"))]
    let (mut _usb_builder, mut keyboard_reader, mut keyboard_writer, mut other_writer, mut vial_reader_writer) = {
        let mut usb_builder: embassy_usb::Builder<'_, D> = new_usb_builder(usb_driver, rmk_config.usb_config);
        let keyboard_reader_writer = add_usb_reader_writer!(&mut usb_builder, KeyboardReport, 1, 8);
        let other_writer = add_usb_writer!(&mut usb_builder, CompositeReport, 9);
        let vial_reader_writer = add_usb_reader_writer!(&mut usb_builder, ViaReport, 32, 32);
        let (keyboard_reader, keyboard_writer) = keyboard_reader_writer.split();
        (
            usb_builder,
            keyboard_reader,
            keyboard_writer,
            other_writer,
            vial_reader_writer,
        )
    };

    // Optional usb logger initialization
    #[cfg(all(feature = "usb_log", not(feature = "_no_usb")))]
    let usb_logger = add_usb_logger!(&mut _usb_builder);

    #[cfg(not(feature = "_no_usb"))]
    let mut usb_device = _usb_builder.build();

    #[cfg(feature = "controller")]
    let mut controller_pub = unwrap!(CONTROLLER_CHANNEL.publisher());

    // Load current connection type
    #[cfg(feature = "storage")]
    {
        let mut buf: [u8; 16] = [0; 16];
        if let Ok(Some(StorageData::ConnectionType(conn_type))) =
            read_storage!(storage, &(StorageKeys::ConnectionType as u32), buf)
        {
            CONNECTION_TYPE.store(conn_type, Ordering::SeqCst);
        } else {
            // If no saved connection type, return default value
            #[cfg(feature = "_no_usb")]
            CONNECTION_TYPE.store(ConnectionType::Ble.into(), Ordering::SeqCst);
            #[cfg(not(feature = "_no_usb"))]
            CONNECTION_TYPE.store(ConnectionType::Usb.into(), Ordering::SeqCst);
        }

        #[cfg(feature = "controller")]
        send_controller_event(
            &mut controller_pub,
            ControllerEvent::ConnectionType(CONNECTION_TYPE.load(Ordering::SeqCst)),
        );
    }

    // Create profile manager
    let mut profile_manager = ProfileManager::new(
        &stack,
        #[cfg(feature = "controller")]
        controller_pub,
    );

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
        name: rmk_config.usb_config.product_name,
        appearance: &appearance::human_interface_device::KEYBOARD,
    }))
    .unwrap();

    server
        .set(
            &server.device_info_service.pnp_id,
            &PnPID {
                vid_source: VidSource::UsbIF,
                vendor_id: rmk_config.usb_config.vid,
                product_id: rmk_config.usb_config.pid,
                product_version: 0x0001,
            },
        )
        .unwrap();

    server
        .set(
            &server.device_info_service.serial_number,
            &heapless::String::try_from(rmk_config.usb_config.serial_number).unwrap(),
        )
        .unwrap();

    server
        .set(
            &server.device_info_service.manufacturer_name,
            &heapless::String::try_from(rmk_config.usb_config.manufacturer).unwrap(),
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
            let adv_fut = advertise(rmk_config.usb_config.product_name, &mut peripheral, &server);
            // USB + BLE dual mode
            #[cfg(not(feature = "_no_usb"))]
            {
                match get_connection_type() {
                    ConnectionType::Usb => {
                        info!("USB priority mode, waiting for USB enabled or BLE connection");
                        match select4(
                            USB_ENABLED.wait(),
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
                                info!("USB enabled, run USB keyboard");
                                USB_ENABLED.signal(());
                                let usb_fut = run_keyboard(
                                    keymap,
                                    #[cfg(feature = "storage")]
                                    storage,
                                    USB_SUSPENDED.wait(),
                                    light_controller,
                                    UsbLedReader::new(&mut keyboard_reader),
                                    UsbVialReaderWriter::new(&mut vial_reader_writer),
                                    UsbKeyboardWriter::new(&mut keyboard_writer, &mut other_writer),
                                    rmk_config.vial_config,
                                );
                                select(usb_fut, profile_manager.update_profile()).await;
                            }
                            Either4::Second(Ok(conn)) => {
                                info!("No USB, BLE connected, run BLE keyboard");
                                let ble_fut = run_ble_keyboard(
                                    &server,
                                    &conn,
                                    &stack,
                                    light_controller,
                                    keymap,
                                    &mut rmk_config,
                                    #[cfg(feature = "storage")]
                                    storage,
                                );
                                select3(ble_fut, USB_SUSPENDED.wait(), profile_manager.update_profile()).await;
                                continue;
                            }
                            Either4::Second(Err(BleHostError::BleHost(Error::Timeout))) => {
                                warn!("Advertising timeout, sleep and wait for any key");

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
                            keymap,
                            #[cfg(feature = "storage")]
                            storage,
                            core::future::pending::<()>(), // Run forever until BLE connected
                            light_controller,
                            UsbLedReader::new(&mut keyboard_reader),
                            UsbVialReaderWriter::new(&mut vial_reader_writer),
                            UsbKeyboardWriter::new(&mut keyboard_writer, &mut other_writer),
                            rmk_config.vial_config,
                        );
                        match select3(adv_fut, usb_fut, profile_manager.update_profile()).await {
                            Either3::First(Ok(conn)) => {
                                info!("BLE connected, running BLE keyboard");
                                select(
                                    run_ble_keyboard(
                                        &server,
                                        &conn,
                                        &stack,
                                        light_controller,
                                        keymap,
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
                            light_controller,
                            keymap,
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
        // Signal to indicate the stack is started
        #[cfg(feature = "split")]
        crate::split::ble::central::STACK_STARTED.signal(true);

        #[cfg(not(feature = "split"))]
        if let Err(e) = runner.run().await {
            panic!("[ble_task] error: {:?}", e);
        }

        #[cfg(feature = "split")]
        if let Err(e) = runner
            .run_with_handler(&crate::split::ble::central::ScanHandler {})
            .await
        {
            panic!("[ble_task] error: {:?}", e);
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
    let output_via = server.via_service.output_via;
    let input_via = server.via_service.input_via;
    let via_control_point = server.via_service.hid_control_point;
    let battery_level = server.battery_service.level;
    let mouse = server.composite_service.mouse_report;
    let media = server.composite_service.media_report;
    let media_control_point = server.composite_service.hid_control_point;
    let system_control = server.composite_service.system_report;

    CONNECTION_STATE.store(ConnectionState::Connected.into(), Ordering::Release);
    loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => {
                info!("[gatt] disconnected: {:?}", reason);
                break;
            }
            GattConnectionEvent::Bonded { bond_info } => {
                info!("[gatt] bonded: {:?}", bond_info);
                let profile_info = ProfileInfo {
                    slot_num: ACTIVE_PROFILE.load(Ordering::SeqCst),
                    info: bond_info,
                    removed: false,
                    cccd_table: server.get_cccd_table(conn.raw()).unwrap_or_default(),
                };
                UPDATED_PROFILE.signal(profile_info);
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

                        if conn.raw().encrypted() {
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
                        } else if event.handle() == output_via.handle {
                            debug!("Got via packet: {:?}", event.data());
                            if event.data().len() == 32 {
                                let data = unsafe { *(event.data().as_ptr() as *const [u8; 32]) };
                                VIAL_READ_CHANNEL.send(data).await;
                            } else {
                                warn!("Wrong via packet data: {:?}", event.data());
                            }
                        } else if event.handle() == input_keyboard.cccd_handle.expect("No CCCD for input keyboard")
                            || event.handle() == input_via.cccd_handle.expect("No CCCD for input via")
                            || event.handle() == mouse.cccd_handle.expect("No CCCD for mouse report")
                            || event.handle() == media.cccd_handle.expect("No CCCD for media report")
                            || event.handle() == system_control.cccd_handle.expect("No CCCD for system report")
                            || event.handle() == battery_level.cccd_handle.expect("No CCCD for battery level")
                        {
                            // CCCD write event
                            cccd_updated = true;
                        } else if event.handle() == hid_control_point.handle
                            || event.handle() == via_control_point.handle
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
                            debug!("Write GATT Event to Unknown: {:?}", event.handle());
                        }

                        if conn.raw().encrypted() {
                            None
                        } else {
                            Some(AttErrorCode::INSUFFICIENT_ENCRYPTION)
                        }
                    }
                    GattEvent::Other(_) => None,
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
                info!("[gatt] PhyUpdated: {:?}, {:?}", tx_phy, rx_phy);
            }
            GattConnectionEvent::ConnectionParamsUpdated {
                conn_interval,
                peripheral_latency,
                supervision_timeout,
            } => {
                info!(
                    "[gatt] ConnectionParamsUpdated: {:?}ms, {:?}, {:?}ms",
                    conn_interval.as_millis(),
                    peripheral_latency,
                    supervision_timeout.as_millis()
                );
            }
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
            AdStructure::ServiceUuids16(&[BATTERY.to_le_bytes(), HUMAN_INTERFACE_DEVICE.to_le_bytes()]),
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
        &ConnectParams {
            min_connection_interval: Duration::from_millis(15),
            max_connection_interval: Duration::from_millis(15),
            max_latency: 30,
            event_length: Duration::from_secs(0),
            supervision_timeout: Duration::from_secs(6),
        },
    )
    .await;

    embassy_time::Timer::after_secs(5).await;

    // Setting the conn param the second time ensures that we have best performance on all platforms
    update_conn_params(
        stack,
        conn.raw(),
        &ConnectParams {
            min_connection_interval: Duration::from_micros(7500),
            max_connection_interval: Duration::from_micros(7500),
            max_latency: 99,
            event_length: Duration::from_secs(0),
            supervision_timeout: Duration::from_secs(5),
        },
    )
    .await;

    // Wait forever. This is because we want the conn params setting can be interrupted when the connection is lost.
    // So this task shouldn't quit after setting the conn params.
    core::future::pending::<()>().await;
}

/// Run BLE keyboard with connected device
async fn run_ble_keyboard<
    'a,
    'b,
    'c,
    'd,
    C: Controller + ControllerCmdAsync<LeSetPhy> + ControllerCmdSync<LeReadLocalSupportedFeatures>,
    Out: OutputPin,
    #[cfg(feature = "storage")] F: AsyncNorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    server: &'b Server<'_>,
    conn: &GattConnection<'a, 'b, DefaultPacketPool>,
    stack: &Stack<'_, C, DefaultPacketPool>,
    light_controller: &mut LightController<Out>,
    keymap: &'c RefCell<KeyMap<'c, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    rmk_config: &'d mut RmkConfig<'static>,
    #[cfg(feature = "storage")] storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
) {
    let ble_hid_server = BleHidServer::new(&server, &conn);
    let ble_via_server = BleViaServer::new(&server, &conn);
    let ble_led_reader = BleLedReader {};
    let mut ble_battery_server = BleBatteryServer::new(&server, &conn);

    // Load CCCD table from storage
    #[cfg(feature = "storage")]
    if let Ok(Some(bond_info)) = storage
        .read_trouble_bond_info(ACTIVE_PROFILE.load(Ordering::SeqCst))
        .await
    {
        if bond_info.info.identity.match_identity(&conn.raw().peer_identity()) {
            info!("Loading CCCD table from storage: {:?}", bond_info.cccd_table);
            server.set_cccd_table(conn.raw(), bond_info.cccd_table.clone());
        }
    }

    // Use 2M Phy
    update_ble_phy(stack, conn.raw()).await;

    let communication_task = async {
        match select3(
            gatt_events_task(server, conn),
            set_conn_params(stack, conn),
            ble_battery_server.run(),
        )
        .await
        {
            Either3::First(e) => error!("[gatt_events_task] end: {:?}", e),
            _ => {}
        }
    };

    run_keyboard(
        keymap,
        #[cfg(feature = "storage")]
        storage,
        communication_task,
        light_controller,
        ble_led_reader,
        ble_via_server,
        ble_hid_server,
        rmk_config.vial_config,
    )
    .await;
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
    params: &ConnectParams,
) {
    loop {
        match conn.update_connection_params(&stack, params).await {
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
