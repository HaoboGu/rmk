use crate::config::StorageConfig;

use crate::{
    keyboard_config::{CommunicationConfig, KeyboardConfig},
    usb_interrupt_map::get_usb_info,
    ChipModel,
};

// Default config for rp2040
pub(crate) fn default_rp2040(chip: ChipModel) -> KeyboardConfig {
    KeyboardConfig {
        chip,
        communication: CommunicationConfig::Usb(get_usb_info("rp2040").unwrap()),
        storage: StorageConfig {
            start_addr: Some(0),
            num_sectors: Some(16),
            enabled: true,
        },
        ..Default::default()
    }
}
