use defmt::info;
use embassy_futures::join::join3;
use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Timer};
use trouble_host::prelude::*;

use crate::ble::descriptor::BleCompositeReportType;

/// Size of L2CAP packets (ATT MTU is this - 4)
const L2CAP_MTU: usize = 251;

/// Max number of connections
const CONNECTIONS_MAX: usize = 1;

/// Max number of L2CAP channels.
const L2CAP_CHANNELS_MAX: usize = 2; // Signal + att

const MAX_ATTRIBUTES: usize = 65;

type Resources<C> = HostResources<C, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX, L2CAP_MTU>;

// GATT Server definition
#[gatt_server(attribute_data_size = MAX_ATTRIBUTES)]
struct Server {
    battery_service: BatteryService,
    hid_service: HidService,
}

// Battery service
#[gatt_service(uuid = "180f")]
struct BatteryService {
    #[characteristic(uuid = "2a19", read, write, notify, on_read = battery_level_on_read, on_write = battery_level_on_write)]
    level: u8,
}

fn battery_level_on_read(_connection: &Connection) {
    info!("[gatt] Read event on battery level characteristic");
}

fn battery_level_on_write(_connection: &Connection, data: &[u8]) -> Result<(), ()> {
    info!(
        "[gatt] Write event on battery level characteristic: {:?}",
        data
    );
    Ok(())
}

const report_desc: [u8; 90] = [
    5u8, 1u8, 9u8, 6u8, 161u8, 1u8, 133u8, 1u8, 5u8, 7u8, 25u8, 224u8, 41u8, 231u8, 21u8, 0u8,
    37u8, 1u8, 117u8, 1u8, 149u8, 8u8, 129u8, 2u8, 25u8, 0u8, 41u8, 255u8, 38u8, 255u8, 0u8, 117u8,
    8u8, 149u8, 1u8, 129u8, 3u8, 5u8, 8u8, 25u8, 1u8, 41u8, 5u8, 37u8, 1u8, 117u8, 1u8, 149u8, 5u8,
    145u8, 2u8, 149u8, 3u8, 145u8, 3u8, 5u8, 7u8, 25u8, 0u8, 41u8, 221u8, 38u8, 255u8, 0u8, 117u8,
    8u8, 149u8, 6u8, 129u8, 0u8, 192u8, 5u8, 1u8, 9u8, 128u8, 161u8, 1u8, 133u8, 4u8, 25u8, 129u8,
    41u8, 183u8, 21u8, 1u8, 149u8, 1u8, 129u8, 0u8, 192u8,
];

#[gatt_service(uuid = "1812")]
struct NHidService {
    #[characteristic(uuid = "2a4a", read, on_read = hid_info_on_read)]
    hid_info: [u8; 4],
    #[characteristic(uuid = "2a4b", read, on_read = report_map_on_read)]
    report_map: [u8; 90],
    #[characteristic(uuid = "2a4c", write_without_response, on_write = hid_control_point_on_write)]
    hid_control_point: u8,
    #[characteristic(uuid = "2a4d", read, notify, on_read = input_keyboard_on_read, on_write = input_keyboard_on_write)]
    input_keyboard: [u8; 8],
    #[characteristic(uuid = "2a4d", read, write, write_without_response, on_read = output_keyboard_on_read, on_write = output_keyboard_on_write)]
    output_keyboard: [u8; 1],
    #[characteristic(uuid = "2a4d", read, notify, on_read = sysetm_keyboard_on_read, on_write = system_keyboard_on_write)]
    system_keyboard: [u8; 1],
}

struct HidService {
    handle: AttributeHandle,
    hid_info: Characteristic<[u8; 4]>,
    report_map: Characteristic<[u8; 1]>,
    hid_control_point: Characteristic<[u8; 1]>,
    protocol_mode: Characteristic<[u8; 1]>,
    hid_report: Characteristic<[u8; 8]>,
    hid_report_desc: DescriptorHandle,
    output_report: Characteristic<[u8; 1]>,
    output_report_desc: DescriptorHandle,
    system_report: Characteristic<[u8; 1]>,
    system_report_desc: DescriptorHandle,
}

#[allow(unused)]
impl HidService {
    fn new<M, const MAX_ATTRIBUTES: usize>(
        table: &mut AttributeTable<'_, M, MAX_ATTRIBUTES>,
    ) -> Self
    where
        M: embassy_sync::blocking_mutex::raw::RawMutex,
    {
        let mut service = table.add_service(Service::new(
            ::trouble_host::types::uuid::Uuid::new_short(6162u16),
        ));
        let hid_info = {
            static HID_INFO: static_cell::StaticCell<[u8; size_of::<[u8; 4]>()]> =
                static_cell::StaticCell::new();
            let store = HID_INFO.init([
                0x1u8, 0x1u8,  // HID version: 1.1
                0x00u8, // Country Code
                0x03u8, // Remote wake + Normally Connectable
            ]);
            let mut builder = service.add_characteristic(
                ::trouble_host::types::uuid::Uuid::new_short(10826u16),
                &[CharacteristicProp::Read],
                store,
            );
            builder.set_read_callback(hid_info_on_read);
            builder.build()
        };
        let report_map = {
            static REPORT_MAP: static_cell::StaticCell<[u8; 90]> = static_cell::StaticCell::new();
            let mut store = REPORT_MAP.init(report_desc);
            // let mut store = KeyboardReport::desc();
            let mut builder = service.add_characteristic(
                ::trouble_host::types::uuid::Uuid::new_short(0x2a4b),
                &[CharacteristicProp::Read],
                store,
            );
            builder.set_read_callback(report_map_on_read);
            builder.build()
        };
        let hid_control_point = {
            static HID_CONTROL_POINT: static_cell::StaticCell<[u8; size_of::<[u8; 1]>()]> =
                static_cell::StaticCell::new();
            let store = HID_CONTROL_POINT.init([0; size_of::<[u8; 1]>()]);
            let mut builder = service.add_characteristic(
                ::trouble_host::types::uuid::Uuid::new_short(10828u16),
                &[CharacteristicProp::WriteWithoutResponse],
                store,
            );
            builder.set_write_callback(hid_control_point_on_write);
            builder.build()
        };
        let protocol_mode = {
            static PROTOCOL_MODE: static_cell::StaticCell<[u8; size_of::<[u8; 1]>()]> =
                static_cell::StaticCell::new();
            let store = PROTOCOL_MODE.init([1; size_of::<[u8; 1]>()]);
            let mut builder = service.add_characteristic(
                ::trouble_host::types::uuid::Uuid::new_short(10830u16),
                &[
                    CharacteristicProp::Read,
                    CharacteristicProp::WriteWithoutResponse,
                ],
                store,
            );
            builder.set_read_callback(protocol_mode_on_read);
            builder.set_write_callback(protocol_mode_on_write);
            builder.build()
        };

        let (hid_report, hid_report_desc) = {
            static HID_REPORT: static_cell::StaticCell<[u8; 8]> = static_cell::StaticCell::new();
            let store = HID_REPORT.init([0; 8]);
            static INPUT_KEYBOARD_DESC: static_cell::StaticCell<[u8; size_of::<[u8; 2]>()]> =
                static_cell::StaticCell::new();
            let mut input_keyboard_desc_data =
                INPUT_KEYBOARD_DESC.init([BleCompositeReportType::Keyboard as u8, 1u8]);
            let mut builder = service.add_characteristic(
                ::trouble_host::types::uuid::Uuid::new_short(10829u16),
                &[CharacteristicProp::Read, CharacteristicProp::Notify],
                store,
            );
            builder.set_read_callback(input_keyboard_on_read);
            builder.set_write_callback(input_keyboard_on_write);
            let desc_builder = builder.add_descriptor(
                ::trouble_host::types::uuid::Uuid::new_short(10504u16),
                &[CharacteristicProp::Read, CharacteristicProp::Notify],
                input_keyboard_desc_data,
                Some(keyboard_desc_on_read),
                None,
            );

            (builder.build(), desc_builder)
        };
        let (output_report, output_report_desc) = {
            static OUTPUT_REPORT: static_cell::StaticCell<[u8; 1]> = static_cell::StaticCell::new();
            let store = OUTPUT_REPORT.init([0; 1]);
            let mut builder: CharacteristicBuilder<'_, '_, [u8; 1], M, MAX_ATTRIBUTES> = service
                .add_characteristic(
                    ::trouble_host::types::uuid::Uuid::new_short(10829u16),
                    &[
                        CharacteristicProp::Read,
                        CharacteristicProp::Write,
                        CharacteristicProp::WriteWithoutResponse,
                    ],
                    store,
                );
            static OUTPUT_KEYBOARD_DESC: static_cell::StaticCell<[u8; size_of::<[u8; 2]>()]> =
                static_cell::StaticCell::new();
            let mut input_keyboard_desc_data =
                OUTPUT_KEYBOARD_DESC.init([BleCompositeReportType::Keyboard as u8, 2u8]);
            let desc_builder = builder.add_descriptor(
                ::trouble_host::types::uuid::Uuid::new_short(10504u16),
                &[
                    CharacteristicProp::Read,
                    CharacteristicProp::Write,
                    CharacteristicProp::WriteWithoutResponse,
                ],
                input_keyboard_desc_data,
                Some(keyboard_desc_on_read),
                None,
            );
            (builder.build(), desc_builder)
        };
        let (system_report, system_report_desc) = {
            static SYSTEM_REPORT: static_cell::StaticCell<[u8; 1]> = static_cell::StaticCell::new();
            let store = SYSTEM_REPORT.init([0; 1]);
            let mut builder = service.add_characteristic(
                ::trouble_host::types::uuid::Uuid::new_short(10829u16),
                &[CharacteristicProp::Read, CharacteristicProp::Notify],
                store,
            );
            static SYSTEM_DESC: static_cell::StaticCell<[u8; size_of::<[u8; 2]>()]> =
                static_cell::StaticCell::new();
            let mut input_keyboard_desc_data =
                SYSTEM_DESC.init([BleCompositeReportType::System as u8, 1u8]);
            let system_report_desc = builder.add_descriptor(
                ::trouble_host::types::uuid::Uuid::new_short(10504u16),
                &[CharacteristicProp::Read, CharacteristicProp::Notify],
                input_keyboard_desc_data,
                Some(keyboard_desc_on_read),
                None,
            );
            (builder.build(), system_report_desc)
        };

        Self {
            handle: service.build(),
            hid_info,
            report_map,
            hid_control_point,
            protocol_mode,
            hid_report,
            hid_report_desc,
            output_report,
            output_report_desc,
            system_report,
            system_report_desc,
        }
    }
}

fn hid_info_on_read(_connection: &Connection) {
    info!("[gatt] Read event on hid info characteristic");
}

fn report_map_on_read(_connection: &Connection) {
    info!("[gatt] Read event on report map characteristic");
}

fn hid_control_point_on_read(_connection: &Connection) {
    info!("[gatt] Read event on hid control point characteristic");
}

fn hid_control_point_on_write(_connection: &Connection, data: &[u8]) -> Result<(), ()> {
    info!(
        "[gatt] Write event on hid control point characteristic: {:?}",
        data
    );
    Ok(())
}

fn input_keyboard_on_read(_connection: &Connection) {
    info!("[gatt] Read event on input keyboard characteristic");
}

fn input_keyboard_on_write(_connection: &Connection, data: &[u8]) -> Result<(), ()> {
    info!(
        "[gatt] Write event on input keyboard characteristic: {:?}",
        data
    );
    Ok(())
}

fn output_keyboard_on_read(_connection: &Connection) {
    info!("[gatt] Read event on output keyboard characteristic");
}

fn output_keyboard_on_write(_connection: &Connection, data: &[u8]) -> Result<(), ()> {
    info!(
        "[gatt] Write event on output keyboard characteristic: {:?}",
        data
    );
    Ok(())
}

fn sysetm_keyboard_on_read(_connection: &Connection) {
    info!("[gatt] Read event on system keyboard characteristic");
}

fn system_keyboard_on_write(_connection: &Connection, data: &[u8]) -> Result<(), ()> {
    info!(
        "[gatt] Write event on system keyboard characteristic: {:?}",
        data
    );
    Ok(())
}

fn protocol_mode_on_read(_connection: &Connection) {
    info!("[gatt] Read event on protocol mode characteristic");
}

fn protocol_mode_on_write(_connection: &Connection, data: &[u8]) -> Result<(), ()> {
    info!(
        "[gatt] Write event on protocol mode characteristic: {:?}",
        data
    );
    Ok(())
}

fn keyboard_desc_on_read(_connection: &Connection) {
    info!("[gatt] Read event on keyboard descriptor");
}

pub async fn run_ble_task<C: Controller>(controller: C) {
    let address = Address::random([0x41, 0x5A, 0xE3, 0x1E, 0x83, 0xE7]);
    info!("Our address = {:?}", address);

    let mut resources = Resources::new(PacketQos::None);
    let (stack, peripheral, _, runner) = trouble_host::new(controller, &mut resources)
        .set_random_address(address)
        .build();

    let server = Server::new_with_config(
        stack,
        GapConfig::Peripheral(PeripheralConfig {
            name: "TrouBLE",
            appearance: &appearance::KEYBOARD,
        }),
    )
    .unwrap();

    info!("Starting advertising and GATT service");
    let _ = join3(
        ble_task(runner),
        gatt_task(&server),
        advertise_task(peripheral, &server),
    )
    .await;
}

async fn advertise_task<C: Controller>(
    mut peripheral: Peripheral<'_, C>,
    server: &Server<'_, '_, C>,
) -> Result<(), BleHostError<C::Error>> {
    let mut adv_data = [0; 31];
    AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids16(&[
                // Uuid::Uuid16([0x0a, 0x18]),
                Uuid::Uuid16([0x12, 0x18]),
                Uuid::Uuid16([0x0f, 0x18]),
            ]),
            AdStructure::CompleteLocalName(b"Trouble"),
            AdStructure::Unknown {
                ty: 0x19, // Appearance
                data: &[0xC1, 0x03],
            },
        ],
        &mut adv_data[..],
    )?;
    loop {
        info!("[adv] advertising");
        let mut advertiser = peripheral
            .advertise(
                &Default::default(),
                Advertisement::ConnectableScannableUndirected {
                    adv_data: &adv_data[..],
                    scan_data: &[],
                },
            )
            .await?;
        let conn = advertiser.accept().await?;
        info!("[adv] connection established");
        let mut tick: u8 = 0;
        let level = server.battery_service.level;
        loop {
            match select(conn.next(), Timer::after(Duration::from_secs(2))).await {
                Either::First(event) => match event {
                    ConnectionEvent::Disconnected { reason } => {
                        info!("[adv] disconnected: {:?}", reason);
                        break;
                    }
                    ConnectionEvent::Gatt { event, .. } => match event {
                        GattEvent::Read { value_handle } => {
                            if value_handle == level.handle {
                                let value = server.get(&level);
                                info!("[gatt] Read Event to Level Characteristic: {:?}", value);
                            }
                        }
                        GattEvent::Write { value_handle } => {
                            if value_handle == level.handle {
                                let value = server.get(&level);
                                info!("[gatt] Write Event to Level Characteristic: {:?}", value);
                            }
                        }
                    },
                },
                Either::Second(_) => {
                    tick = tick.wrapping_add(1);
                    info!("[adv] notifying connection of tick {}", tick);
                    // Write battery
                    let _ = server
                        .notify(&server.battery_service.level, &conn, &tick)
                        .await;
                    // input keyboard handle
                    info!("Notifying input_keyboard");
                    server
                        .notify(
                            &server.hid_service.hid_report,
                            &conn,
                            &[0x04, 0, 0, 0, 0, 0, 0, 0],
                        )
                        .await?;
                    embassy_time::Timer::after_millis(50).await;
                    server
                        .notify(
                            &server.hid_service.hid_report,
                            &conn,
                            &[0, 0, 0, 0, 0, 0, 0, 0],
                        )
                        .await?;
                }
            }
        }
    }
}

async fn ble_task<C: Controller>(mut runner: Runner<'_, C>) -> Result<(), BleHostError<C::Error>> {
    runner.run().await
}

async fn gatt_task<C: Controller>(
    server: &Server<'_, '_, C>,
) -> Result<(), BleHostError<C::Error>> {
    server.run().await
}
