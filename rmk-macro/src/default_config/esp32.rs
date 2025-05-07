use rmk_config::{BleConfig, StorageConfig};

use rmk_config::CommunicationConfig;
use crate::keyboard_config::KeyboardConfig;
use rmk_config::ChipModel;

// Default config for esp32
pub(crate) fn default_esp32(chip: ChipModel) -> KeyboardConfig {
    KeyboardConfig {
        chip,
        communication: CommunicationConfig::Ble(BleConfig {
            enabled: true,
            ..Default::default()
        }),
        storage: StorageConfig {
            start_addr: Some(0),
            num_sectors: Some(16),
            enabled: true,
            ..Default::default()
        },
        ..Default::default()
    }
}
