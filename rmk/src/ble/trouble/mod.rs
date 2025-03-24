use crate::ble::led::BleLedReader;
use crate::channel::{FLASH_CHANNEL, LED_SIGNAL, VIAL_READ_CHANNEL};
use crate::config::RmkConfig;
use crate::hid::{HidReaderTrait, HidWriterTrait, RunnableHidWriter};
use crate::keymap::KeyMap;
use crate::light::{LedIndicator, LightController};
use crate::storage::Storage;
use crate::{run_keyboard, LightService, VialService, CONNECTION_STATE};
use ble_server::{BleHidServer, BleViaServer, Server};
use core::cell::RefCell;
use core::sync::atomic::AtomicU8;
use embassy_futures::join::join;
use embassy_futures::select::{select, select4};
use embassy_time::Timer;
use embassy_usb::driver::Driver;
use embedded_hal::digital::OutputPin;
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
use heapless::Vec;
use rand_core::{CryptoRng, RngCore};
use trouble_host::{prelude::*, BondInformation, LongTermKey};
use usbd_hid::descriptor::{MediaKeyboardReport, SerializedDescriptor};

pub(crate) mod ble_server;
pub(crate) mod bonder;

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
    // Using a fixed "random" address can be useful for testing. In real scenarios, one would
    // use e.g. the MAC 6 byte array as the address (how to get that varies by the platform).
    let address: Address = Address::random([0xff, 0x8f, 0x1a, 0x05, 0xe4, 0xff]);
    info!("Our address = {}", address);

    let mut resources: HostResources<CONNECTIONS_MAX, L2CAP_CHANNELS_MAX, 255> =
        HostResources::new();
    let stack = trouble_host::new(controller, &mut resources)
        .set_random_address(address)
        .set_random_generator_seed(random_generator);

    let mut bond_info: Vec<bonder::BondInfo, BONDED_DEVICE_NUM> = Vec::new();
    for slot_num in 0..BONDED_DEVICE_NUM {
        if let Ok(Some(info)) = storage.read_trouble_bond_info(slot_num as u8).await {
            stack.add_bond_information(info.info.clone()).unwrap();
            bond_info.push(info).unwrap();
        }
    }

    info!(
        "Loaded {} bond information: {:?}",
        bond_info.len(),
        bond_info
    );

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

    let _ = join(ble_task(runner), async {
        loop {
            match advertise(rmk_config.usb_config.product_name, &mut peripheral, &server).await {
                Ok(conn) => {
                    CONNECTION_STATE.store(true, core::sync::atomic::Ordering::SeqCst);
                    let mut ble_hid_server = BleHidServer::new(&server, &conn);
                    let ble_via_server = BleViaServer::new(&server, &conn);
                    let ble_led_reader = BleLedReader {};
                    let mut light_service = LightService::new(light_controller, ble_led_reader);
                    let mut vial_service =
                        VialService::new(keymap, rmk_config.vial_config, ble_via_server);
                    let led_fut = light_service.run();
                    let via_fut = vial_service.run();

                    #[cfg(any(feature = "_ble", not(feature = "_no_external_storage")))]
                    let storage_fut = storage.run();
                    select4(
                        gatt_events_task(&server, &conn, &stack),
                        select(storage_fut, via_fut),
                        led_fut,
                        ble_hid_server.run_writer(),
                    )
                    .await;
                    let bond_info = stack.get_bond_information();
                    info!("saving bond_info: {:?}", bond_info);
                    FLASH_CHANNEL
                        .send(crate::storage::FlashOperationMessage::TroubleBondInfo(
                            bonder::BondInfo {
                                slot_num: 0,
                                info: bond_info[0].clone(),
                            },
                        ))
                        .await;
                }
                Err(e) => {
                    panic!("[adv] error: {:?}", e);
                }
            }
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

// async fn ble_keyboard_task<C: Controller>(
//     server: &Server<'_>,
//     conn: &GattConnection<'_, '_>,
//     stack: &Stack<'_, C>,
//     storage: &mut F,
// ) {
//     match select(
//         gatt_events_task(server, conn),
//         custom_task(server, conn, stack, storage),
//     )
//     .await
//     {
//         embassy_futures::select::Either::First(e) => {
//             info!("[ble_keyboard_task] gatt_events_task finished: {:?}", e);
//         }
//         embassy_futures::select::Either::Second(_) => {
//             info!("[ble_keyboard_task] custom_task finished");
//         }
//     }
// }

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
                FLASH_CHANNEL
                    .send(crate::storage::FlashOperationMessage::TroubleBondInfo(
                        bonder::BondInfo {
                            slot_num: 0,
                            info: bond_info[0].clone(),
                        },
                    ))
                    .await;
                break;
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
            AdStructure::ServiceUuids16(&[[0x0f, 0x18], [0x12, 0x18]]),
            AdStructure::CompleteLocalName(name.as_bytes()),
            AdStructure::Unknown {
                ty: 0x19, // Appearance
                data: &[0xC1, 0x03],
            },
        ],
        &mut advertiser_data[..],
    )?;
    let advertiser = peripheral
        .advertise(
            &Default::default(),
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

// use usbd_hid::descriptor::MediaKeyboardReport;
use ssmarshal::serialize;
use usbd_hid::descriptor::MediaKey;
/// Example task to use the BLE notifier interface.
/// This task will notify the connected central of a counter value every 2 seconds.
/// It will also read the RSSI value every 2 seconds.
/// and will stop when the connection is closed by the central or an error occurs.
async fn custom_task<C: Controller, F: AsyncNorFlash>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_>,
    stack: &Stack<'_, C>,
    storage: &mut F,
) {
    let mut tick: u8 = 0;
    let level = server.battery_service.level;
    let mut last_bond_info = stack.get_bond_information();
    loop {
        tick = tick.wrapping_add(1);
        info!("[custom_task] notifying connection of tick {}", tick);
        if level.notify(conn, &tick).await.is_err() {
            info!("[custom_task] error notifying connection");
            break;
        };
        // let report = server.hid_service.input_keyboard;
        let report = server.composite_service.media_report;
        let pressed_report = MediaKeyboardReport {
            usage_id: MediaKey::VolumeDecrement as u16,
        };
        let released_report = MediaKeyboardReport { usage_id: 0 };
        let mut buf = [0u8; 2];
        let n = serialize(&mut buf, &pressed_report).unwrap();

        if report.notify(conn, &buf).await.is_err() {
            info!("[custom_task] error notifying connection");
            break;
        };
        Timer::after_millis(200).await;

        let n = serialize(&mut buf, &released_report).unwrap();
        if report.notify(conn, &buf).await.is_err() {
            info!("[custom_task] error notifying connection");
            break;
        };

        // read RSSI (Received Signal Strength Indicator) of the connection.
        if let Ok(rssi) = conn.raw().rssi(stack).await {
            info!("[custom_task] RSSI: {:?}", rssi);
        } else {
            info!("[custom_task] error getting RSSI");
            break;
        };
        Timer::after_secs(5).await;
        let bond_info = stack.get_bond_information();
        if bond_info != last_bond_info {
            last_bond_info = bond_info;
            if last_bond_info.len() >= 1 {
                let mut buf = [0u8; 32];
                let ltk = last_bond_info[0].ltk.to_le_bytes();
                let address = last_bond_info[0].address;
                buf[0..16].copy_from_slice(&ltk);
                buf[16..22].copy_from_slice(address.raw());
                if let Err(e) = storage.write(0, &buf).await {
                    info!("[custom_task] error writing bond info: {:?}", e);
                }
                // Saving bond info
                info!("Saving Bond information: {:?}", last_bond_info);
            }
        }
    }
}
