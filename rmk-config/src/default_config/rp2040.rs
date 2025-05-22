use crate::usb_interrupt_map::get_usb_info;
use crate::{ChipModel, CommunicationConfig, KeyboardConfig, StorageConfig};

pub fn default_rp2040(chip: ChipModel) -> KeyboardConfig {
    KeyboardConfig {
        chip,
        communication: CommunicationConfig::Usb(get_usb_info("rp2040").unwrap()),
        storage: StorageConfig {
            start_addr: Some(1024 * 1024), // Start from 1M
            num_sectors: Some(32),         // Use 32 sectors
            enabled: true,
            ..Default::default()
        },
        ..Default::default()
    }
}
