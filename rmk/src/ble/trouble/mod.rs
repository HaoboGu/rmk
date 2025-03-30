use core::cell::RefCell;
use core::sync::atomic::{AtomicU8, Ordering};

use ble_server::{BleBatteryServer, BleHidServer, BleViaServer, Server};
use embassy_futures::join::join;
use embassy_futures::select::{select, select3, Either3};
use embassy_time::{Duration, Timer};
use embedded_hal::digital::OutputPin;
use profile::UPDATED_PROFILE;
use rand_core::{CryptoRng, RngCore};
use trouble_host::prelude::appearance::human_interface_device::KEYBOARD;
use trouble_host::prelude::service::{BATTERY, HUMAN_INTERFACE_DEVICE};
use trouble_host::prelude::*;
#[cfg(not(feature = "_no_usb"))]
use {
    crate::light::UsbLedReader,
    crate::state::get_connection_type,
    crate::usb::descriptor::{CompositeReport, KeyboardReport, ViaReport},
    crate::usb::UsbKeyboardWriter,
    crate::usb::{add_usb_reader_writer, new_usb_builder, register_usb_writer},
    crate::usb::{USB_ENABLED, USB_SUSPENDED},
    crate::via::UsbVialReaderWriter,
    embassy_futures::select::{select4, Either4},
    embassy_usb::driver::Driver,
};
#[cfg(feature = "storage")]
use {
    crate::storage::{Storage, StorageData, StorageKeys},
    crate::{read_storage, state::CONNECTION_TYPE},
    embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash,
};

use crate::ble::led::BleLedReader;
use crate::ble::trouble::profile::{ProfileInfo, ProfileManager};
use crate::channel::{LED_SIGNAL, VIAL_READ_CHANNEL};
use crate::config::RmkConfig;
use crate::hid::{DummyWriter, RunnableHidWriter};
use crate::keymap::KeyMap;
use crate::light::{LedIndicator, LightController};
use crate::state::{ConnectionState, ConnectionType};
use crate::{run_keyboard, CONNECTION_STATE};

pub(crate) mod ble_server;
pub(crate) mod profile;

/// Maximum number of bonded devices
pub const BONDED_DEVICE_NUM: usize = 8;

/// The number of the active profile
pub static ACTIVE_PROFILE: AtomicU8 = AtomicU8::new(0);

/// Max number of connections
const CONNECTIONS_MAX: usize = 4;

/// Max number of L2CAP channels.
const L2CAP_CHANNELS_MAX: usize = 8; // Signal + att

/// Build the BLE stack.
pub async fn build_ble_stack<'a, C: Controller, RNG: RngCore + CryptoRng>(
    controller: C,
    host_address: [u8; 6],
    random_generator: &mut RNG,
    resources: &'a mut HostResources<CONNECTIONS_MAX, L2CAP_CHANNELS_MAX, 255>,
) -> Stack<'a, C> {
    // Using a fixed "random" address can be useful for testing. In real scenarios, one would
    // use e.g. the MAC 6 byte array as the address (how to get that varies by the platform).
    let address: Address = Address::random(host_address);

    // Initialize trouble host stack
    trouble_host::new(controller, resources)
        .set_random_address(address)
        .set_random_generator_seed(random_generator)
}

/// Run the BLE stack.
pub(crate) async fn run_ble<
    'a,
    'b,
    C: Controller,
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
    stack: &'b Stack<'b, C>,
    #[cfg(feature = "storage")] storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    light_controller: &mut LightController<Out>,
    rmk_config: RmkConfig<'static>,
) {
    // Initialize usb device and usb hid reader/writer
    #[cfg(not(feature = "_no_usb"))]
    let (mut usb_device, mut keyboard_reader, mut keyboard_writer, mut other_writer, mut vial_reader_writer) = {
        let mut usb_builder: embassy_usb::Builder<'_, D> = new_usb_builder(usb_driver, rmk_config.usb_config);
        let keyboard_reader_writer = add_usb_reader_writer!(&mut usb_builder, KeyboardReport, 1, 8);
        let other_writer = register_usb_writer!(&mut usb_builder, CompositeReport, 9);
        let vial_reader_writer = add_usb_reader_writer!(&mut usb_builder, ViaReport, 32, 32);
        let (keyboard_reader, keyboard_writer) = keyboard_reader_writer.split();
        let usb_device = usb_builder.build();
        (
            usb_device,
            keyboard_reader,
            keyboard_writer,
            other_writer,
            vial_reader_writer,
        )
    };

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
    }

    // Create profile manager
    let mut profile_manager = ProfileManager::new(&stack);

    #[cfg(feature = "storage")]
    // Load saved bonding information
    profile_manager.load_bonded_devices(storage).await;
    // Update bonding information in the stack
    profile_manager.update_stack_bonds();

    // Build trouble host stack
    let Host {
        mut peripheral, runner, ..
    } = stack.build();

    // Set conn param
    info!("Starting advertising and GATT service");
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: rmk_config.usb_config.product_name,
        appearance: &appearance::human_interface_device::KEYBOARD,
    }))
    .unwrap();

    #[cfg(not(feature = "_no_usb"))]
    let background_task = join(ble_task(runner), usb_device.run());
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
                                    &rmk_config,
                                    #[cfg(feature = "storage")]
                                    storage,
                                );
                                select3(ble_fut, USB_SUSPENDED.wait(), profile_manager.update_profile()).await;
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
                                        &rmk_config,
                                        #[cfg(feature = "storage")]
                                        storage,
                                    ),
                                    profile_manager.update_profile(),
                                )
                                .await;
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
                            &rmk_config,
                            #[cfg(feature = "storage")]
                            storage,
                        ),
                        profile_manager.update_profile(),
                    )
                    .await;
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
pub(crate) async fn ble_task<C: Controller>(mut runner: Runner<'_, C>) {
    loop {
        // Signal to indicate the stack is started
        #[cfg(feature = "split")]
        crate::split::ble::central::STACK_STARTED.signal(true);

        if let Err(e) = runner.run().await {
            panic!("[ble_task] error: {:?}", e);
        }
    }
}

/// Stream Events until the connection closes.
///
/// This function will handle the GATT events and process them.
/// This is how we interact with read and write requests.
async fn gatt_events_task(server: &Server<'_>, conn: &GattConnection<'_, '_>) -> Result<(), Error> {
    let level = server.battery_service.level;
    let output_keyboard = server.hid_service.output_keyboard;
    let output_via = server.via_service.output_via;
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
                };
                UPDATED_PROFILE.signal(profile_info);
            }
            GattConnectionEvent::Gatt { event } => {
                match event {
                    Ok(event) => {
                        let result = match &event {
                            GattEvent::Read(event) => {
                                if event.handle() == level.handle {
                                    let value = server.get(&level);
                                    info!("[gatt] Read Event to Level: {:?}", value);
                                } else {
                                    info!("[gatt] Read Event to Unknown : {:?}", event.handle());
                                }

                                if conn.raw().encrypted() {
                                    None
                                } else {
                                    Some(AttErrorCode::INSUFFICIENT_ENCRYPTION)
                                }
                            }
                            GattEvent::Write(event) => {
                                if event.handle() == output_keyboard.handle {
                                    let led_indicator = LedIndicator::from_bits(event.data()[0]);
                                    info!("Read keyboard state: {:?}", led_indicator);
                                    LED_SIGNAL.signal(led_indicator);
                                } else if event.handle() == output_via.handle {
                                    info!("[gatt] Write Event to Output Via: {:?}", event.data());
                                    let data = unsafe { *(event.data().as_ptr() as *const [u8; 32]) };
                                    VIAL_READ_CHANNEL.send(data).await;
                                } else {
                                    info!("[gatt] Write Event to Unknown: {:?}", event.handle());
                                }
                                if conn.raw().encrypted() {
                                    None
                                } else {
                                    Some(AttErrorCode::INSUFFICIENT_ENCRYPTION)
                                }
                            }
                        };

                        // This step is also performed at drop(), but writing it explicitly is necessary
                        // in order to ensure reply is sent.
                        let result = if let Some(code) = result {
                            event.reject(code)
                        } else {
                            event.accept()
                        };
                        match result {
                            Ok(reply) => {
                                reply.send().await;
                            }
                            Err(e) => {
                                warn!("[gatt] error sending response: {:?}", e);
                            }
                        }
                    }
                    Err(e) => warn!("[gatt] error processing event: {:?}", e),
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
                    "[gatt] ConnectionParamsUpdated: {:?}, {:?}, {:?}",
                    conn_interval, peripheral_latency, supervision_timeout
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
    peripheral: &mut Peripheral<'a, C>,
    server: &'b Server<'_>,
) -> Result<GattConnection<'a, 'b>, BleHostError<C::Error>> {
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
        timeout: Some(Duration::from_secs(120)),
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
    let conn = advertiser.accept().await?.with_attribute_server(server)?;
    info!("[adv] connection established");
    Ok(conn)
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

pub(crate) async fn set_conn_params<'a, 'b, C: Controller>(stack: &Stack<'_, C>, conn: &GattConnection<'a, 'b>) {
    // Wait for 5 seconds before setting connection parameters to avoid connection drop
    embassy_time::Timer::after_secs(5).await;

    // For macOS/iOS(aka Apple devices), both interval should be set to 15ms
    if let Err(e) = conn
        .raw()
        .update_connection_params(
            &stack,
            &ConnectParams {
                min_connection_interval: Duration::from_millis(15),
                max_connection_interval: Duration::from_millis(15),
                max_latency: 99,
                event_length: Duration::from_secs(0),
                supervision_timeout: Duration::from_secs(5),
            },
        )
        .await
    {
        #[cfg(feature = "defmt")]
        let e = defmt::Debug2Format(&e);
        error!("[set_conn_params] error: {:?}", e);
    }

    embassy_time::Timer::after_secs(5).await;

    // Setting the conn param the second time ensures that we have best performance on all platforms
    loop {
        match conn
            .raw()
            .update_connection_params(
                &stack,
                &ConnectParams {
                    min_connection_interval: Duration::from_micros(7500),
                    max_connection_interval: Duration::from_micros(7500),
                    max_latency: 99,
                    event_length: Duration::from_secs(0),
                    supervision_timeout: Duration::from_secs(5),
                },
            )
            .await
        {
            Err(BleHostError::BleHost(Error::Hci(error))) => {
                if 0x2A == error.to_status().into_inner() {
                    // Busy, retry
                    continue;
                } else {
                    error!("[set_conn_params] 2nd time HCI error: {:?}", error);
                    break;
                }
            }
            _ => break,
        };
    }

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
    C: Controller,
    Out: OutputPin,
    #[cfg(feature = "storage")] F: AsyncNorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    server: &'b Server<'_>,
    conn: &GattConnection<'a, 'b>,
    stack: &Stack<'_, C>,
    light_controller: &mut LightController<Out>,
    keymap: &'c RefCell<KeyMap<'c, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    rmk_config: &'d RmkConfig<'static>,
    #[cfg(feature = "storage")] storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
) {
    let ble_hid_server = BleHidServer::new(&server, &conn);
    let ble_via_server = BleViaServer::new(&server, &conn);
    let ble_led_reader = BleLedReader {};
    let ble_battery_server = BleBatteryServer::new(&server, &conn);

    let communication_task = async {
        match select3(
            gatt_events_task(&server, &conn),
            set_conn_params(&stack, &conn),
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
