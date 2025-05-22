use crate::usb_interrupt_map::get_usb_info;
use crate::{BleConfig, ChipModel, CommunicationConfig, KeyboardConfig, StorageConfig};

pub fn default_nrf52833(chip: ChipModel) -> KeyboardConfig {
    KeyboardConfig {
        chip,
        communication: CommunicationConfig::Both(
            get_usb_info("nrf52833").unwrap(),
            BleConfig {
                enabled: true,
                adc_divider_measured: Some(2000),
                adc_divider_total: Some(2806),
                ..Default::default()
            },
        ),
        storage: StorageConfig {
            start_addr: Some(0x60000),
            num_sectors: Some(16),
            enabled: true,
            ..Default::default()
        },
        ..Default::default()
    }
}
