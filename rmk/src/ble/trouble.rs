use defmt::{error, info};
use embassy_futures::select::select3;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use static_cell::StaticCell;
use trouble_host::advertise::{
    AdStructure, Advertisement, BR_EDR_NOT_SUPPORTED, LE_GENERAL_DISCOVERABLE,
};
use trouble_host::attribute::{AttributeTable, Characteristic, CharacteristicProp, Service, Uuid};
use trouble_host::gatt::GattEvent;
use trouble_host::gatt::GattServer;
use trouble_host::{Address, BleHost, BleHostError, BleHostResources, Controller, PacketQos};
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor};

use crate::ble::device_info::{DeviceInformation, PnPID, VidSource};
use crate::ble::{
    descriptor::BleCompositeReportType,
    nrf::spec::{BleCharacteristics, BleDescriptor, BleSpecification},
};

/// Size of L2CAP packets (ATT MTU is this - 4)
const L2CAP_MTU: usize = 251;

/// Max number of connections
const CONNECTIONS_MAX: usize = 2;

/// Max number of L2CAP channels.
const L2CAP_CHANNELS_MAX: usize = 2; // Signal + att

const MAX_ATTRIBUTES: usize = 50;

pub async fn run_ble_task<C: Controller>(controller: C) {
    static HOST_RESOURCES: StaticCell<
        BleHostResources<CONNECTIONS_MAX, L2CAP_CHANNELS_MAX, L2CAP_MTU>,
    > = StaticCell::new();
    let resources = HOST_RESOURCES.init(BleHostResources::new(PacketQos::None));

    let mut ble: BleHost<'_, _> = BleHost::new(controller, resources);

    //let address: Address = Address::random([0xff, 0x8f, 0x1a, 0x05, 0xe4, 0xff]);
    let address = Address::random([0x41, 0x5A, 0xE3, 0x1E, 0x83, 0xE7]);
    info!("Our address = {:?}", address);
    ble.set_random_address(address);

    let mut table: AttributeTable<'_, NoopRawMutex, MAX_ATTRIBUTES> = AttributeTable::new();

    // Generic Access Service (mandatory)
    let id = b"Trouble";
    let appearance = [0x80, 0x07];
    let mut bat_level = [23; 1];
    let mut svc = table.add_service(Service::new(0x1800));
    let _ = svc.add_characteristic_ro(0x2a00, id);
    let _ = svc.add_characteristic_ro(0x2a01, &appearance[..]);
    svc.build();

    // Generic attribute service (mandatory)
    table.add_service(Service::new(0x1801));

    // Battery service
    let level_handle = table
        .add_service(Service::new(0x180f))
        .add_characteristic(
            0x2a19,
            &[CharacteristicProp::Read, CharacteristicProp::Notify],
            &mut bat_level,
        )
        .build();

    // Device info service
    let mut device_info_handle =
        table.add_service(Service::new(BleSpecification::DeviceInformation as u16));
    let pnp_id = PnPID {
        vid_source: VidSource::UsbIF,
        vendor_id: 0x4C4B,
        product_id: 0x4643,
        product_version: 0x0000,
    };
    let device_information = DeviceInformation {
        manufacturer_name: Some("Haobo"),
        model_number: Some("0"),
        serial_number: Some("0"),
        // manufacturer_name: Some(usb_config.manufacturer),
        // model_number: Some(usb_config.product_name),
        // serial_number: Some(usb_config.serial_number),
        ..Default::default()
    };
    // SAFETY: `PnPID` is `repr(C, packed)` so viewing it as an immutable slice of bytes is safe.
    let pnp_id_data = unsafe {
        core::slice::from_raw_parts(
            &pnp_id as *const _ as *const u8,
            core::mem::size_of::<PnPID>(),
        )
    };
    device_info_handle.add_characteristic_ro(BleCharacteristics::PnpId as u16, pnp_id_data);
    device_info_handle.add_characteristic_ro(
        BleCharacteristics::ManufacturerName as u16,
        device_information
            .manufacturer_name
            .unwrap_or("")
            .as_bytes(),
    );
    device_info_handle.add_characteristic_ro(
        BleCharacteristics::ModelNumber as u16,
        device_information.model_number.unwrap_or("").as_bytes(),
    );
    device_info_handle.add_characteristic_ro(
        BleCharacteristics::SerialNumber as u16,
        device_information.serial_number.unwrap_or("").as_bytes(),
    );
    device_info_handle.add_characteristic_ro(
        BleCharacteristics::HardwareRevision as u16,
        device_information.hw_rev.unwrap_or("").as_bytes(),
    );
    device_info_handle.add_characteristic_ro(
        BleCharacteristics::FirmwareRevision as u16,
        device_information.fw_rev.unwrap_or("").as_bytes(),
    );
    device_info_handle.add_characteristic_ro(
        BleCharacteristics::SoftwareRevision as u16,
        device_information.sw_rev.unwrap_or("").as_bytes(),
    );
    device_info_handle.build();

    // Hid service
    let _hid_info_handle = table
        .add_service(Service::new(BleSpecification::HidService as u16))
        .add_characteristic_ro(
            BleCharacteristics::HidInfo as u16,
            &[
                0x1u8, 0x1u8,  // HID version: 1.1
                0x00u8, // Country Code
                0x03u8, // Remote wake + Normally Connectable
            ],
        )
        .build();

    let _report_map_handle = table
        .add_service(Service::new(BleSpecification::HidService as u16))
        .add_characteristic_ro(BleCharacteristics::ReportMap as u16, KeyboardReport::desc())
        .build();

    let mut hid_control_data = [0u8];
    let _hid_control_handle = table
        .add_service(Service::new(BleSpecification::HidService as u16))
        .add_characteristic(
            BleCharacteristics::HidControlPoint as u16,
            &[
                CharacteristicProp::Read,
                CharacteristicProp::WriteWithoutResponse,
            ],
            &mut hid_control_data,
        )
        .build();

    let mut protocol_mode_data = [1u8];
    let _protocol_mode_handle = table
        .add_service(Service::new(BleSpecification::HidService as u16))
        .add_characteristic(
            BleCharacteristics::ProtocolMode as u16,
            &[
                CharacteristicProp::Read,
                CharacteristicProp::WriteWithoutResponse,
            ],
            &mut protocol_mode_data,
        )
        .build();

    let mut input_keyboard_desc_data = [BleCompositeReportType::Keyboard as u8, 1u8];
    let mut input_keyboard_data = [0u8; 8];
    let mut hid_service = table.add_service(Service::new(BleSpecification::HidService as u16));
    let mut input_keyboard_handle = hid_service.add_characteristic(
        BleCharacteristics::HidReport as u16,
        &[
            CharacteristicProp::Read,
            CharacteristicProp::Write,
            CharacteristicProp::Notify,
        ],
        &mut input_keyboard_data,
    );

    let _input_keyboard_desc_handle = input_keyboard_handle.add_descriptor_ro(
        BleDescriptor::ReportReference as u16,
        &mut input_keyboard_desc_data,
    );

    let input_keyboard_handle = input_keyboard_handle.build();
    hid_service.build();

    let server = ble.gatt_server::<NoopRawMutex, MAX_ATTRIBUTES, L2CAP_MTU>(&table);

    info!("Starting advertising and GATT service");
    loop {
        let _ = select3(
            ble.run(),
            gatt_task(&server, &table),
            advertise_task(&ble, &server, level_handle, input_keyboard_handle),
            // advertise_task(&ble, &server, input_keyboard_handle),
        )
        .await;
        error!("Restarting BLE stack");
        embassy_time::Timer::after_secs(5).await;
    }
}

async fn gatt_task(
    server: &GattServer<'_, '_, NoopRawMutex, MAX_ATTRIBUTES, L2CAP_MTU>,
    table: &AttributeTable<'_, NoopRawMutex, MAX_ATTRIBUTES>,
) {
    loop {
        match server.next().await {
            Ok(GattEvent::Write {
                handle,
                connection: _,
            }) => {
                if let Err(e) = table.get(handle, |value| {
                    info!(
                        "[gatt] Write event on {:?}. Value written: {:?}",
                        handle, value
                    );
                }) {
                    error!("[gatt] Error reading value: {:?}, handle: {}", e, handle);
                };
            }
            Ok(GattEvent::Read {
                handle,
                connection: _,
            }) => {
                info!("[gatt] Read event on {:?}", handle);
            }
            Err(e) => {
                error!("[gatt] Error processing GATT events: {:?}", e);
            }
        }
    }
}

async fn advertise_task<C: Controller>(
    ble: &BleHost<'_, C>,
    server: &GattServer<'_, '_, NoopRawMutex, MAX_ATTRIBUTES, L2CAP_MTU>,
    handle: Characteristic,
    input_keyboard_handle: Characteristic,
) -> Result<(), BleHostError<C::Error>> {
    let mut adv_data = [0; 31];
    AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids16(&[
                Uuid::Uuid16([0x0a, 0x18]),
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
        let mut advertiser = ble
            .advertise(
                &Default::default(),
                Advertisement::ConnectableScannableUndirected {
                    adv_data: &adv_data[..],
                    scan_data: &[],
                },
            )
            .await?;
        let conn = advertiser.accept().await?;
        info!("Connected");
        // Keep connection alive
        embassy_time::Timer::after_secs(5).await;
        let mut value: u8 = 20;
        while conn.is_connected() {
            value = 100 - value;
            info!("Notifying data");
            // Write battery
            server.notify(&ble, handle, &conn, &[value]).await?;

            // input keyboard handle
            server
                .notify(
                    &ble,
                    input_keyboard_handle,
                    &conn,
                    &[0x04, 0, 0, 0, 0, 0, 0, 0],
                )
                .await?;
            embassy_time::Timer::after_millis(50).await;
            server
                .notify(
                    &ble,
                    input_keyboard_handle,
                    &conn,
                    &[0, 0, 0, 0, 0, 0, 0, 0],
                )
                .await?;
            embassy_time::Timer::after_secs(5).await;
        }
    }
}
