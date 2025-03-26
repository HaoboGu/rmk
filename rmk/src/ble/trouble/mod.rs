use crate::ble::led::BleLedReader;
use crate::channel::{FLASH_CHANNEL, LED_SIGNAL, VIAL_READ_CHANNEL};
use crate::config::RmkConfig;
use crate::hid::{DummyWriter, RunnableHidWriter};
use crate::keymap::KeyMap;
use crate::light::{LedIndicator, LightController};
use crate::storage::Storage;
use crate::{LightService, VialService, CONNECTION_STATE};
use ble_server::{BleHidServer, BleViaServer, Server};
use core::cell::RefCell;
use core::sync::atomic::{AtomicU8, Ordering};
use embassy_futures::join::join;
use embassy_futures::select::{select, select4};
use embassy_time::{Duration, Timer};
use embedded_hal::digital::OutputPin;
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;

use rand_core::{CryptoRng, RngCore};
use trouble_host::prelude::appearance::human_interface_device::KEYBOARD;
use trouble_host::prelude::service::{BATTERY, HUMAN_INTERFACE_DEVICE};
use trouble_host::prelude::*;

#[cfg(not(feature = "_no_usb"))]
use {
    crate::usb::descriptor::{CompositeReport, KeyboardReport, ViaReport},
    crate::usb::{add_usb_reader_writer, new_usb_builder, register_usb_writer},
    crate::usb::{wait_for_usb_suspend, UsbState, USB_STATE},
    crate::via::UsbVialReaderWriter,
    crate::{
        run_keyboard, run_usb_device, ConnectionType, UsbKeyboardWriter, UsbLedReader,
        CONNECTION_TYPE,
    },
    embassy_futures::select::{select3, Either3},
    embassy_usb::driver::Driver,
    profile::update_profile,
};

pub(crate) mod ble_server;
pub(crate) mod bonder;
pub(crate) mod profile;

/// Maximum number of bonded devices
pub const BONDED_DEVICE_NUM: usize = 8;

/// The number of the active profile
pub static ACTIVE_PROFILE: AtomicU8 = AtomicU8::new(0);

/// Max number of connections
const CONNECTIONS_MAX: usize = 1;

/// Max number of L2CAP channels.
const L2CAP_CHANNELS_MAX: usize = 2; // Signal + att

/// Run the BLE stack.
pub async fn run<
    'a,
    C: Controller,
    F: AsyncNorFlash,
    RNG: RngCore + CryptoRng,
    #[cfg(not(feature = "_no_usb"))] D: Driver<'static>,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>>,
    storage: &mut Storage<F, ROW, COL, NUM_LAYER>,
    #[cfg(not(feature = "_no_usb"))] usb_driver: D,
    controller: C,
    random_generator: &mut RNG,
    light_controller: &mut LightController<Out>,
    rmk_config: RmkConfig<'static>,
) {
    // Initialize usb device and usb hid reader/writer
    #[cfg(not(feature = "_no_usb"))]
    let (
        mut usb_device,
        mut keyboard_reader,
        mut keyboard_writer,
        mut other_writer,
        mut vial_reader_writer,
    ) = {
        let mut usb_builder: embassy_usb::Builder<'_, D> =
            new_usb_builder(usb_driver, rmk_config.usb_config);
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

    // Using a fixed "random" address can be useful for testing. In real scenarios, one would
    // use e.g. the MAC 6 byte array as the address (how to get that varies by the platform).
    let address: Address = Address::random([0xff, 0x8f, 0x1a, 0x05, 0xe4, 0xff]);
    info!("Our address = {}", address);

    // Initialize trouble host stack
    let mut resources: HostResources<CONNECTIONS_MAX, L2CAP_CHANNELS_MAX, 255> =
        HostResources::new();
    let stack = trouble_host::new(controller, &mut resources)
        .set_random_address(address)
        .set_random_generator_seed(random_generator);

    // Load saved bond info
    for slot_num in 0..BONDED_DEVICE_NUM {
        if let Ok(Some(info)) = storage.read_trouble_bond_info(slot_num as u8).await {
            stack.add_bond_information(info.info.clone()).unwrap();
            debug!("Loaded bond info: {:?}", info);
        }
    }

    // Build trouble host stack
    let Host {
        mut peripheral,
        runner,
        ..
    } = stack.build();

    info!("Starting advertising and GATT service");
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: rmk_config.usb_config.product_name,
        appearance: &appearance::human_interface_device::KEYBOARD,
    }))
    .unwrap();

    // Main loop
    join(ble_task(runner), async {
        loop {
            let adv_fut = advertise(rmk_config.usb_config.product_name, &mut peripheral, &server);
            // USB + BLE dual mode
            #[cfg(not(feature = "_no_usb"))]
            {
                debug!(
                    "usb state: {}, connection type: {}",
                    USB_STATE.load(Ordering::SeqCst),
                    CONNECTION_TYPE.load(Ordering::Acquire)
                );
                // Check whether the USB is connected
                if USB_STATE.load(Ordering::SeqCst) != UsbState::Disabled as u8 {
                    let usb_fut = run_keyboard(
                        keymap,
                        storage,
                        run_usb_device(&mut usb_device),
                        light_controller,
                        UsbLedReader::new(&mut keyboard_reader),
                        UsbVialReaderWriter::new(&mut vial_reader_writer),
                        UsbKeyboardWriter::new(&mut keyboard_writer, &mut other_writer),
                        rmk_config.vial_config,
                    );
                    match ConnectionType::current() {
                        ConnectionType::Usb => {
                            // USB priority mode
                            match select3(usb_fut, wait_for_usb_suspend(), update_profile()).await {
                                Either3::Third(_) => {
                                    Timer::after_millis(10).await;
                                    continue;
                                }
                                _ => (),
                            }
                        }
                        ConnectionType::Ble => {
                            // BLE priority mode, try to connect to the BLE device while running USB keyboard
                            info!("Running USB keyboard, while advertising");
                            match select3(adv_fut, usb_fut, update_profile()).await {
                                Either3::First(Ok(conn)) => {
                                    // BLE connected
                                    let mut ble_hid_server = BleHidServer::new(&server, &conn);
                                    let ble_via_server = BleViaServer::new(&server, &conn);
                                    let ble_led_reader = BleLedReader {};
                                    let mut light_service =
                                        LightService::new(light_controller, ble_led_reader);
                                    let mut vial_service = VialService::new(
                                        keymap,
                                        rmk_config.vial_config,
                                        ble_via_server,
                                    );
                                    let led_fut = light_service.run();
                                    let via_fut = vial_service.run();
                                    let storage_fut = storage.run();

                                    select4(
                                        gatt_events_task(&server, &conn, &stack),
                                        select(storage_fut, via_fut),
                                        led_fut,
                                        ble_hid_server.run_writer(),
                                    )
                                    .await;
                                }
                                _ => {
                                    debug!("USB disconnected or profile updated");
                                    Timer::after_millis(10).await;
                                    continue;
                                }
                            }
                        }
                    }
                } else {
                    // USB isn't connected, wait for any of BLE/USB connection
                    let dummy_task = run_dummy_keyboard(storage);

                    match select3(adv_fut, wait_for_status_change(), dummy_task).await {
                        Either3::First(Ok(conn)) => {
                            // BLE connected
                            let mut ble_hid_server = BleHidServer::new(&server, &conn);
                            let ble_via_server = BleViaServer::new(&server, &conn);
                            let ble_led_reader = BleLedReader {};
                            let mut light_service =
                                LightService::new(light_controller, ble_led_reader);
                            let mut vial_service =
                                VialService::new(keymap, rmk_config.vial_config, ble_via_server);
                            let led_fut = light_service.run();
                            let via_fut = vial_service.run();
                            let storage_fut = storage.run();

                            select4(
                                gatt_events_task(&server, &conn, &stack),
                                select(storage_fut, via_fut),
                                led_fut,
                                ble_hid_server.run_writer(),
                            )
                            .await;
                        }
                        _ => {
                            // Wait 10ms for usb resuming/switching profile/advertising error
                            Timer::after_millis(10).await;
                        }
                    }
                }
            }

            #[cfg(feature = "_no_usb")]
            match adv_fut.await {
                Ok(conn) => {
                    // BLE connected
                    let mut ble_hid_server = BleHidServer::new(&server, &conn);
                    let ble_via_server = BleViaServer::new(&server, &conn);
                    let ble_led_reader = BleLedReader {};
                    let mut light_service = LightService::new(light_controller, ble_led_reader);
                    let mut vial_service =
                        VialService::new(keymap, rmk_config.vial_config, ble_via_server);
                    let led_fut = light_service.run();
                    let via_fut = vial_service.run();
                    let storage_fut = storage.run();

                    select4(
                        gatt_events_task(&server, &conn, &stack),
                        select(storage_fut, via_fut),
                        led_fut,
                        ble_hid_server.run_writer(),
                    )
                    .await;
                }
                Err(e) => error!("Advertise error: {:?}", e),
            }

            // Retry after 200 ms
            Timer::after_millis(200).await;
        }
    })
    .await;
}

/// This is a background task that is required to run forever alongside any other BLE tasks.
async fn ble_task<C: Controller>(mut runner: Runner<'_, C>) {
    loop {
        if let Err(e) = runner.run().await {
            panic!("[ble_task] error: {:?}", e);
        }
    }
}

/// Stream Events until the connection closes.
///
/// This function will handle the GATT events and process them.
/// This is how we interact with read and write requests.
async fn gatt_events_task<C: Controller>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_>,
    stack: &Stack<'_, C>,
) -> Result<(), Error> {
    let level = server.battery_service.level;
    let input_keyboard = server.hid_service.input_keyboard;
    let output_keyboard = server.hid_service.output_keyboard;
    let input_via = server.via_service.input_via;
    let output_via = server.via_service.output_via;
    loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => {
                info!("[gatt] disconnected: {:?}", reason);
                let bond_info = stack.get_bond_information();
                info!("saving bond_info: {:?}", bond_info);
                if bond_info.len() >= 1 {
                    FLASH_CHANNEL
                        .send(crate::storage::FlashOperationMessage::TroubleBondInfo(
                            bonder::BondInfo {
                                slot_num: 0,
                                info: bond_info[0].clone(),
                            },
                        ))
                        .await;
                }
                break;
            }
            GattConnectionEvent::Bonded { bond_info } => {
                info!("[gatt] bonded: {:?}", bond_info);
                FLASH_CHANNEL
                    .send(crate::storage::FlashOperationMessage::TroubleBondInfo(
                        bonder::BondInfo {
                            slot_num: 0,
                            info: bond_info,
                        },
                    ))
                    .await;
            }
            GattConnectionEvent::Gatt { event } => {
                match event {
                    Ok(event) => {
                        let result = match &event {
                            GattEvent::Read(event) => {
                                if event.handle() == level.handle {
                                    let value = server.get(&level);
                                    info!("[gatt] Read Event to Level: {:?}", value);
                                } else if event.handle() == input_keyboard.handle {
                                    let value = server.get(&input_keyboard);
                                    info!("[gatt] Read Event to Input Keyboard  {:?}", value);
                                } else if event.handle() == output_keyboard.handle {
                                    let value = server.get(&output_keyboard);
                                    info!("[gatt] Read Event to Output Keyboard: {:?}", value);
                                } else if event.handle() == input_via.handle {
                                    let value = server.get(&input_via);
                                    info!("[gatt] Read Event to Input Via : {:?}", value);
                                } else if event.handle() == output_via.handle {
                                    let value = server.get(&output_via);
                                    info!("[gatt] Read Event to Output Via : {:?}", value);
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
                                if event.handle() == level.handle {
                                    info!("[gatt] Write Event to Level: {:?}", event.data());
                                } else if event.handle() == output_keyboard.handle {
                                    info!(
                                        "[gatt] Write Event to Output Keyboard: {:?}",
                                        event.data()
                                    );
                                    let led_indicator = LedIndicator::from_bits(event.data()[0]);
                                    LED_SIGNAL.signal(led_indicator);
                                } else if event.handle() == output_via.handle {
                                    info!("[gatt] Write Event to Output Via: {:?}", event.data());
                                    let data =
                                        unsafe { *(event.data().as_ptr() as *const [u8; 32]) };
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
    let mut advertiser_data = [0; 31];
    AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids16(&[
                BATTERY.to_le_bytes(),
                HUMAN_INTERFACE_DEVICE.to_le_bytes(),
            ]),
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
        interval_min: Duration::from_millis(500),
        interval_max: Duration::from_millis(500),
        ..Default::default()
    };

    let advertiser = peripheral
        .advertise(
            &advertise_config,
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..],
                scan_data: &[],
            },
        )
        .await?;
    info!("[adv] advertising");
    let conn = advertiser.accept().await?.with_attribute_server(server)?;
    info!("[adv] connection established");
    Ok(conn)
}

// Dummy keyboard service is used to monitoring keys when there's no actual connection.
// It's useful for functions like switching active profiles when there's no connection.
pub(crate) async fn run_dummy_keyboard<
    'a,
    'b,
    F: AsyncNorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    storage: &mut Storage<F, ROW, COL, NUM_LAYER>,
) {
    CONNECTION_STATE.store(false, Ordering::Release);
    let storage_fut = storage.run();
    let mut dummy_writer = DummyWriter {};
    select(storage_fut, dummy_writer.run_writer()).await;
}

#[cfg(not(feature = "_no_usb"))]
// Wait for USB enabled or BLE state changed
pub(crate) async fn wait_for_status_change() {
    use crate::usb::wait_for_usb_enabled;

    if CONNECTION_TYPE.load(Ordering::Relaxed) == 0 {
        // Connection type is USB, USB has higher priority
        select(wait_for_usb_enabled(), update_profile()).await;
    } else {
        // Connection type is BLE, so we don't consider USB
        update_profile().await;
    }
}
