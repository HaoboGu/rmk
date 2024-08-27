use defmt::info;
use embassy_futures::join::join3;
use embassy_nrf::pac::ficr::info;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use static_cell::StaticCell;
use trouble_host::advertise::{
    AdStructure, Advertisement, BR_EDR_NOT_SUPPORTED, LE_GENERAL_DISCOVERABLE,
};
use trouble_host::attribute::{AttributeTable, CharacteristicProp, Service, Uuid};
use trouble_host::gatt::GattEvent;
use trouble_host::{Address, BleHost, BleHostResources, Controller, PacketQos};
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor};

use crate::ble::{
    descriptor::BleCompositeReportType,
    nrf::spec::{BleCharacteristics, BleDescriptor, BleSpecification},
};

/// Size of L2CAP packets (ATT MTU is this - 4)
const L2CAP_MTU: usize = 128;

/// Max number of connections
const CONNECTIONS_MAX: usize = 1;

/// Max number of L2CAP channels.
const L2CAP_CHANNELS_MAX: usize = 2; // Signal + att

pub async fn run_ble_task<C: Controller>(controller: C) {
    static HOST_RESOURCES: StaticCell<
        BleHostResources<CONNECTIONS_MAX, L2CAP_CHANNELS_MAX, L2CAP_MTU>,
    > = StaticCell::new();
    let resources = HOST_RESOURCES.init(BleHostResources::new(PacketQos::None));

    let mut ble: BleHost<'_, _> = BleHost::new(controller, resources);

    let address: Address = Address::random([0xff, 0x8f, 0x1a, 0x05, 0xe4, 0xff]);
    // info!("BLE host address = {:?}", address);
    ble.set_random_address(address);

    let mut table: AttributeTable<'_, NoopRawMutex, 50> = AttributeTable::new();

    // Generic Access Service (mandatory)
    let id = b"Trouble";
    let appearance = [0x80, 0x07];
    let mut bat_level = [23; 1];
    let mut svc = table.add_service(Service::new(0x1800));
    let _ = svc.add_characteristic_ro(0x2a00, id);
    let _ = svc.add_characteristic_ro(0x2a01, &appearance[..]);
    drop(svc);

    // Generic attribute service (mandatory)
    table.add_service(Service::new(0x1801));

    // Battery service
    let level_handle = table
        .add_service(Service::new(BleSpecification::BatteryService as u16))
        .add_characteristic(
            BleCharacteristics::BatteryLevel as u16,
            &[CharacteristicProp::Read, CharacteristicProp::Notify],
            &mut bat_level,
        );

    // Hid service
    let hid_info_handle = table
        .add_service(Service::new(BleSpecification::HidService as u16))
        .add_characteristic_ro(
            BleCharacteristics::HidInfo as u16,
            &[
                0x1u8, 0x1u8,  // HID version: 1.1
                0x00u8, // Country Code
                0x03u8, // Remote wake + Normally Connectable
            ],
        );

    let report_map_handle = table
        .add_service(Service::new(BleSpecification::HidService as u16))
        .add_characteristic_ro(BleCharacteristics::ReportMap as u16, KeyboardReport::desc());

    let mut hid_control_data = [0u8];
    let hid_control_handle = table
        .add_service(Service::new(BleSpecification::HidService as u16))
        .add_characteristic(
            BleCharacteristics::HidControlPoint as u16,
            &[
                CharacteristicProp::Read,
                CharacteristicProp::WriteWithoutResponse,
            ],
            &mut hid_control_data,
        );

    let mut protocol_mode_data = [1u8];
    let protocol_mode_handle = table
        .add_service(Service::new(BleSpecification::HidService as u16))
        .add_characteristic(
            BleCharacteristics::ProtocolMode as u16,
            &[
                CharacteristicProp::Read,
                CharacteristicProp::WriteWithoutResponse,
            ],
            &mut protocol_mode_data,
        );

    let mut input_keyboard_desc_data = [BleCompositeReportType::Keyboard as u8, 1u8];
    let mut input_keyboard_data = [0u8; 8];
    let input_keyboard_handle = table
        .add_service(Service::new(BleSpecification::HidService as u16))
        .add_characteristic(
            BleCharacteristics::HidReport as u16,
            &[
                CharacteristicProp::Read,
                CharacteristicProp::Write,
                CharacteristicProp::Notify,
            ],
            &mut input_keyboard_data,
        );

    let input_keyboard_desc_handle = table
        .add_service(Service::new(BleSpecification::HidService as u16))
        .add_characteristic(
            BleDescriptor::ReportReference as u16,
            &[
                CharacteristicProp::Read,
                CharacteristicProp::Write,
                CharacteristicProp::Notify,
            ],
            &mut input_keyboard_desc_data,
        );

    let server = ble.gatt_server::<NoopRawMutex, 50, L2CAP_MTU>(&table);

    let mut adv_data = [0; 31];
    AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids16(&[Uuid::Uuid16([0x0f, 0x18]), Uuid::Uuid16([0x12, 0x18])]),
            AdStructure::CompleteLocalName(b"Trouble"),
            AdStructure::Unknown {
                ty: 0x19, // Appearance
                data: &[0xC1, 0x03],
            },
        ],
        &mut adv_data[..],
    )
    .unwrap();

    info!("Starting advertising and GATT service");
    let _ = join3(
        ble.run(),
        async {
            loop {
                let re = server.next().await;
                info!("GATT next event");
                match re {
                    Ok(GattEvent::Write {
                        handle,
                        connection: _,
                    }) => {
                        let _ = table.get(handle, |value| {
                            info!("Write event. Value written: {:?}", value);
                        });
                    }
                    Ok(GattEvent::Read {
                        handle,
                        connection: _,
                    }) => {
                        if handle == level_handle {
                            info!("Battery level read");
                        } else if handle == hid_info_handle {
                            info!("HID info read");
                        } else if handle == report_map_handle {
                            info!("Report map read");
                        } else if handle == hid_control_handle {
                            info!("HID control read");
                        } else if handle == protocol_mode_handle {
                            info!("Protocol mode read");
                        } else if handle == input_keyboard_handle {
                            info!("Input keyboard read");
                        } else if handle == input_keyboard_desc_handle {
                            info!("Input keyboard desc read");
                        }
                    }
                    Err(e) => {
                        defmt::error!("Error processing GATT events: {}", e);
                    }
                }
            }
        },
        async {
            let mut advertiser = ble
                .advertise(
                    &Default::default(),
                    Advertisement::ConnectableScannableUndirected {
                        adv_data: &adv_data[..],
                        scan_data: &[],
                    },
                )
                .await
                .unwrap();
            let conn = advertiser.accept().await.unwrap();
            info!("Connected");
            // Keep connection alive
            let mut tick: u8 = 0;
            loop {
                tick += 1;
                server
                    .notify(&ble, level_handle, &conn, &[tick])
                    .await
                    .unwrap();
            }
        },
    )
    .await;
}
