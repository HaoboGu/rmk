use nrf_softdevice::ble::advertisement_builder::{
    AdvertisementDataType, Error, Flag, LegacyAdvertisementBuilder, LegacyAdvertisementPayload,
    ServiceList, ServiceUuid16,
};

pub(crate) fn create_advertisement_data(keyboard_name: &str) -> LegacyAdvertisementPayload {
    LegacyAdvertisementBuilder::new()
        .flags(&[Flag::GeneralDiscovery, Flag::LE_Only])
        .services_16(
            ServiceList::Incomplete,
            &[
                ServiceUuid16::BATTERY,
                ServiceUuid16::HUMAN_INTERFACE_DEVICE,
            ],
        )
        .full_name(keyboard_name)
        // Change the appearance (icon of the bluetooth device) to a keyboard
        .raw(AdvertisementDataType::APPEARANCE, &[0xC1, 0x03])
        .try_build()
        .unwrap_or_else(|Error::Oversize { expected }| {
            panic!(
                "keyboard name is {} characters oversize",
                expected - 31 /* for some reason LEGACY_PAYLOAD_LEN is private */
            )
        });
}

pub(crate) static SCAN_DATA: LegacyAdvertisementPayload = LegacyAdvertisementBuilder::new()
    .services_16(
        ServiceList::Complete,
        &[
            ServiceUuid16::DEVICE_INFORMATION,
            ServiceUuid16::BATTERY,
            ServiceUuid16::HUMAN_INTERFACE_DEVICE,
        ],
    )
    .build();
