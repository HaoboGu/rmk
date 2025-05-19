use crate::{BleConfig, ChipModel, CommunicationConfig, KeyboardConfig, StorageConfig};

pub fn default_nrf52810(chip: ChipModel) -> KeyboardConfig {
    KeyboardConfig {
        chip,
        communication: CommunicationConfig::Ble(BleConfig {
            enabled: true,
            ..Default::default()
        }),
        storage: StorageConfig {
            start_addr: Some(0x28000),
            num_sectors: Some(8),
            enabled: true,
            ..Default::default()
        },
        ..Default::default()
    }
}
