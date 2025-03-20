use crate::ble::descriptor::BleCompositeReportType;
use crate::ble::descriptor::BleKeyboardReport;
use embassy_futures::join::join;
use embassy_futures::select::select;
use embassy_time::Timer;
use rand_core::{CryptoRng, RngCore};
use trouble_host::prelude::*;
use usbd_hid::descriptor::KeyboardReport;
use usbd_hid::descriptor::SerializedDescriptor;
/// Max number of connections
const CONNECTIONS_MAX: usize = 1;

/// Max number of L2CAP channels.
const L2CAP_CHANNELS_MAX: usize = 2; // Signal + att

// GATT Server definition
#[gatt_server]
struct Server {
    battery_service: BatteryService,
    hid_service: HidService,
}

#[derive(Clone, Copy, Debug)]
struct ReportDesc([u8; 69]);

impl Default for ReportDesc {
    fn default() -> Self {
        ReportDesc([0; 69])
    }
}
use trouble_host::types::gatt_traits::FromGattError;
impl FromGatt for ReportDesc {
    fn from_gatt(value: &[u8]) -> Result<ReportDesc, FromGattError> {
        if value.len() != 69 {
            return Err(FromGattError::InvalidLength);
        }
        Ok(ReportDesc(value.try_into().unwrap()))
    }
}

impl AsGatt for ReportDesc {
    fn as_gatt(&self) -> &[u8] {
        &self.0
    }

    const MIN_SIZE: usize = 69;

    const MAX_SIZE: usize = 69;
}



/// Battery service
#[gatt_service(uuid = service::BATTERY)]
struct BatteryService {
    /// Battery Level
    #[descriptor(uuid = descriptors::VALID_RANGE, read, value = [0, 100])]
    #[descriptor(uuid = descriptors::MEASUREMENT_DESCRIPTION, name = "hello", read, value = "Battery Level")]
    #[characteristic(uuid = characteristic::BATTERY_LEVEL, read, notify, value = 10)]
    level: u8,
    #[characteristic(uuid = "408813df-5dd4-1f87-ec11-cdb001100000", write, read, notify)]
    status: bool,
}

#[gatt_service(uuid = service::HUMAN_INTERFACE_DEVICE)]
struct HidService {
    #[characteristic(uuid = "2a4a", read, value = [0x01, 0x01, 0x00, 0x03])]
    hid_info: [u8; 4],
    #[characteristic(uuid = "2a4b", read, value = unsafe { *(BleKeyboardReport::desc().as_ptr() as *const [u8; 69]) } )]
    report_map: [u8; 69],
    #[characteristic(uuid = "2a4c", write_without_response)]
    hid_control_point: u8,
    #[characteristic(uuid = "2a4e", read, write_without_response, value = 1)]
    protocol_mode: u8,
    #[descriptor(uuid = "2908", read, value = [BleCompositeReportType::Keyboard as u8, 1u8])]
    #[characteristic(uuid = "2a4d", read, notify)]
    input_keyboard: [u8; 8],
    #[descriptor(uuid = "2908", read, value = [BleCompositeReportType::Keyboard as u8, 2u8])]
    #[characteristic(uuid = "2a4d", read, write, write_without_response)]
    output_keyboard: [u8; 1],
}

/// Run the BLE stack.
pub async fn run<C, RNG, const L2CAP_MTU: usize>(controller: C, random_generator: &mut RNG)
where
    C: Controller,
    RNG: RngCore + CryptoRng,
{
    assert_eq!(BleKeyboardReport::desc().len(), 69);
    // Using a fixed "random" address can be useful for testing. In real scenarios, one would
    // use e.g. the MAC 6 byte array as the address (how to get that varies by the platform).
    let address: Address = Address::random([0xff, 0x8f, 0x1a, 0x05, 0xe4, 0xff]);
    info!("Our address = {}", address);

    let mut resources: HostResources<CONNECTIONS_MAX, L2CAP_CHANNELS_MAX, L2CAP_MTU> =
        HostResources::new();
    let stack = trouble_host::new(controller, &mut resources)
        .set_random_address(address)
        .set_random_generator_seed(random_generator);
    let Host {
        mut peripheral,
        runner,
        ..
    } = stack.build();

    info!("Starting advertising and GATT service");
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "TrouBLE",
        appearance: &appearance::human_interface_device::KEYBOARD,
    }))
    .unwrap();

    let _ = join(ble_task(runner), async {
        loop {
            match advertise("Trouble Example", &mut peripheral, &server).await {
                Ok(conn) => {
                    // set up tasks when the connection is established to a central, so they don't run when no one is connected.
                    let a = gatt_events_task(&server, &conn);
                    let b = custom_task(&server, &conn, &stack);
                    // run until any task ends (usually because the connection has been closed),
                    // then return to advertising state.
                    select(a, b).await;
                }
                Err(e) => {
                    // #[cfg(feature = "defmt")]
                    // let e = defmt::Debug2Format(&e);
                    panic!("[adv] error: {:?}", e);
                }
            }
        }
    })
    .await;
}

/// This is a background task that is required to run forever alongside any other BLE tasks.
///
/// ## Alternative
///
/// If you didn't require this to be generic for your application, you could statically spawn this with i.e.
///
/// ```rust,ignore
///
/// #[embassy_executor::task]
/// async fn ble_task(mut runner: Runner<'static, SoftdeviceController<'static>>) {
///     runner.run().await;
/// }
///
/// spawner.must_spawn(ble_task(runner));
/// ```
async fn ble_task<C: Controller>(mut runner: Runner<'_, C>) {
    loop {
        if let Err(e) = runner.run().await {
            // #[cfg(feature = "defmt")]
            // let e = defmt::Debug2Format(&e);
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
    let report_map = server.hid_service.report_map;
    let hid_control_point = server.hid_service.hid_control_point;
    let input_keyboard = server.hid_service.input_keyboard;
    let output_keyboard = server.hid_service.output_keyboard;
    loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => {
                info!("[gatt] disconnected: {:?}", reason);
                break;
            }
            GattConnectionEvent::Gatt { event } => {
                match event {
                    Ok(event) => {
                        let result = match &event {
                            GattEvent::Read(event) => {
                                if event.handle() == level.handle {
                                    let value = server.get(&level);
                                    info!("[gatt] Read Event to Level Characteristic: {:?}", value);
                                } else if event.handle() == report_map.handle {
                                    let value = server.get(&report_map);
                                    info!(
                                        "[gatt] Read Event to Report Map Characteristic: {:?}",
                                        value
                                    );
                                } else if event.handle() == hid_control_point.handle {
                                    let value = server.get(&hid_control_point);
                                    info!("[gatt] Read Event to HID Control Point Characteristic: {:?}", value);
                                } else if event.handle() == input_keyboard.handle {
                                    let value = server.get(&input_keyboard);
                                    info!(
                                        "[gatt] Read Event to Input Keyboard Characteristic: {:?}",
                                        value
                                    );
                                } else if event.handle() == output_keyboard.handle {
                                    let value = server.get(&output_keyboard);
                                    info!(
                                        "[gatt] Read Event to Output Keyboard Characteristic: {:?}",
                                        value
                                    );
                                } else {
                                    info!(
                                        "[gatt] Read Event to Unknown Characteristic: {:?}",
                                        event.handle()
                                    );
                                }

                                if conn.raw().encrypted() {
                                    None
                                } else {
                                    Some(AttErrorCode::INSUFFICIENT_ENCRYPTION)
                                }
                            }
                            GattEvent::Write(event) => {
                                if event.handle() == level.handle {
                                    info!(
                                        "[gatt] Write Event to Level Characteristic: {:?}",
                                        event.data()
                                    );
                                } else if event.handle() == report_map.handle {
                                    info!(
                                        "[gatt] Write Event to Report Map Characteristic: {:?}",
                                        event.data()
                                    );
                                } else if event.handle() == output_keyboard.handle {
                                    info!(
                                    "[gatt] Write Event to Output Keyboard Characteristic: {:?}",
                                    event.data()
                                );
                                } else {
                                    info!(
                                        "[gatt] Write Event to Unknown Characteristic: {:?}",
                                        event.handle()
                                    );
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

use ssmarshal::serialize;
/// Example task to use the BLE notifier interface.
/// This task will notify the connected central of a counter value every 2 seconds.
/// It will also read the RSSI value every 2 seconds.
/// and will stop when the connection is closed by the central or an error occurs.
async fn custom_task<C: Controller>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_>,
    stack: &Stack<'_, C>,
) {
    let mut tick: u8 = 0;
    let level = server.battery_service.level;
    loop {
        tick = tick.wrapping_add(1);
        info!("[custom_task] notifying connection of tick {}", tick);
        if level.notify(conn, &tick).await.is_err() {
            info!("[custom_task] error notifying connection");
            break;
        };
        let report = server.hid_service.input_keyboard;
        let pressed_report = KeyboardReport {
            modifier: 0,
            reserved: 0,
            leds: 0,
            keycodes: [4, 0, 0, 0, 0, 0],
        };
        let released_report = KeyboardReport {
            modifier: 0,
            reserved: 0,
            leds: 0,
            keycodes: [0, 0, 0, 0, 0, 0],
        };
        let mut buf = [0u8; 8];
        let n = serialize(&mut buf, &pressed_report).unwrap();

        if server
            .hid_service
            .input_keyboard
            .notify(conn, &buf)
            .await
            .is_err()
        {
            info!("[custom_task] error notifying connection");
            break;
        };
        Timer::after_millis(200).await;

        let n = serialize(&mut buf, &released_report).unwrap();
        if server
            .hid_service
            .input_keyboard
            .notify(conn, &buf)
            .await
            .is_err()
        {
            info!("[custom_task] error notifying connection");
            break;
        };

        // read RSSI (Received Signal Strength Indicator) of the connection.
        // if let Ok(rssi) = conn.raw().rssi(stack).await {
        //     info!("[custom_task] RSSI: {:?}", rssi);
        // } else {
        //     info!("[custom_task] error getting RSSI");
        //     break;
        // };
        Timer::after_secs(5).await;
    }
}
