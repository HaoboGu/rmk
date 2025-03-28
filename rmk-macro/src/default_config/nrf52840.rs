use crate::config::{BleConfig, StorageConfig};
use crate::keyboard_config::{CommunicationConfig, KeyboardConfig};
use crate::usb_interrupt_map::get_usb_info;
use crate::ChipModel;

// Default config for nRF52840
pub(crate) fn default_nrf52840(chip: ChipModel) -> KeyboardConfig {
    KeyboardConfig {
        chip,
        communication: CommunicationConfig::Both(
            get_usb_info("nrf52840").unwrap(),
            BleConfig {
                enabled: true,
                // Use nice!nano's default divider config
                adc_divider_measured: Some(2000),
                adc_divider_total: Some(2806),
                ..Default::default()
            },
        ),
        storage: StorageConfig {
            // Special default config for nRF52
            // It's common to use [Adafruit_nRF52_Bootloader](https://github.com/adafruit/Adafruit_nRF52_Bootloader) for nRF52 chips, we don't want our default storage config breaks the bootloader
            start_addr: Some(0x60000),
            num_sectors: Some(16),
            enabled: true,
            ..Default::default()
        },
        ..Default::default()
    }
}
