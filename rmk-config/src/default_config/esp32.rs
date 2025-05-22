use crate::{BleConfig, ChipModel, CommunicationConfig, KeyboardConfig, StorageConfig};

pub fn default_esp32(chip: ChipModel) -> KeyboardConfig {
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
