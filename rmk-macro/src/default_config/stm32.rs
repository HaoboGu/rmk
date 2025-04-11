use crate::config::StorageConfig;
use crate::keyboard_config::{CommunicationConfig, KeyboardConfig};
use crate::usb_interrupt_map::get_usb_info;
use crate::ChipModel;
// Default config for stm32
pub(crate) fn default_stm32(chip: ChipModel) -> KeyboardConfig {
    let chip_name = chip.chip.clone();
    KeyboardConfig {
        chip,
        communication: CommunicationConfig::Usb(get_usb_info(&chip_name).unwrap()),
        storage: StorageConfig {
            start_addr: Some(0),
            num_sectors: Some(2),
            enabled: true,
            ..Default::default()
        },
        ..Default::default()
    }
}
