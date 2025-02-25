use crate::config::{BleConfig, StorageConfig};
use crate::{
    ChipModel,
    keyboard_config::{CommunicationConfig, KeyboardConfig},
};

// Default config for nRF52832
pub(crate) fn default_nrf52832(chip: ChipModel) -> KeyboardConfig {
    KeyboardConfig {
        chip,
        communication: CommunicationConfig::Ble(BleConfig {
            enabled: true,
            ..Default::default()
        }),
        storage: StorageConfig {
            start_addr: Some(0x60000),
            num_sectors: Some(16),
            enabled: true,
            ..Default::default()
        },
        ..Default::default()
    }
}
