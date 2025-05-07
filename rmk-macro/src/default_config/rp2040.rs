use rmk_config::StorageConfig;

use rmk_config::CommunicationConfig;
use crate::keyboard_config::KeyboardConfig;
use rmk_config::usb_interrupt_map::get_usb_info;
use rmk_config::ChipModel;

// Default config for rp2040
pub(crate) fn default_rp2040(chip: ChipModel) -> KeyboardConfig {
    KeyboardConfig {
        chip,
        communication: CommunicationConfig::Usb(get_usb_info("rp2040").unwrap()),
        storage: StorageConfig {
            start_addr: Some(0),
            num_sectors: Some(16),
            enabled: true,
            ..Default::default()
        },
        ..Default::default()
    }
}
