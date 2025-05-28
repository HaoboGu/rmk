use crate::usb_interrupt_map::get_usb_info;
use crate::{BleConfig, ChipModel, ChipSeries, KeyboardTomlConfig};

/// Information about USB interface
#[derive(Clone, Debug, Default)]
pub struct UsbInfo {
    pub dm: String,
    pub dp: String,
    pub peripheral_name: String,
    pub interrupt_name: String,
}

impl UsbInfo {
    pub fn new(dm: &str, dp: &str, p: &str, i: &str) -> Self {
        UsbInfo {
            dm: dm.to_string(),
            dp: dp.to_string(),
            peripheral_name: p.to_string(),
            interrupt_name: i.to_string(),
        }
    }

    pub fn new_default(chip: &ChipModel) -> Self {
        match chip.series {
            ChipSeries::Stm32 => UsbInfo::new("PA11", "PA12", "USB_OTG_FS", "USB_FS"),
            ChipSeries::Nrf52 => UsbInfo::new("", "", "USBD", "USBD"),
            ChipSeries::Rp2040 => UsbInfo::new("", "", "USB", "USBCTRL_IRQ"),
            _ => UsbInfo::new(
                "default_dm",
                "default_dp",
                "default_usb_peripheral",
                "default_usb_interrupt",
            ),
        }
    }
}

/// Communication configuration enum
#[derive(Clone, Debug, Default)]
pub enum CommunicationConfig {
    // USB only
    Usb(UsbInfo),
    // BLE only
    Ble(BleConfig),
    // Both USB and BLE
    Both(UsbInfo, BleConfig),
    #[default]
    None,
}

impl CommunicationConfig {
    pub fn ble_enabled(&self) -> bool {
        matches!(self, CommunicationConfig::Ble(_) | CommunicationConfig::Both(_, _))
    }

    pub fn usb_enabled(&self) -> bool {
        matches!(self, CommunicationConfig::Usb(_) | CommunicationConfig::Both(_, _))
    }

    pub fn get_ble_config(&self) -> Option<BleConfig> {
        match self {
            CommunicationConfig::Ble(ble_config) | CommunicationConfig::Both(_, ble_config) => Some(ble_config.clone()),
            _ => None,
        }
    }

    pub fn get_usb_info(&self) -> Option<UsbInfo> {
        match self {
            CommunicationConfig::Usb(usb_info) | CommunicationConfig::Both(usb_info, _) => Some(usb_info.clone()),
            _ => None,
        }
    }
}

impl KeyboardTomlConfig {
    pub fn get_communication_config(&self) -> Result<CommunicationConfig, String> {
        let usb_enabled = self.keyboard.clone().unwrap_or_default().usb_enable.unwrap_or(false);
        let chip = self.get_chip_model().unwrap();
        let usb_info = if usb_enabled { get_usb_info(&chip.chip) } else { None };
        let ble_config = self.ble.clone();

        match (usb_info, ble_config) {
            (Some(usb_info), None) => Ok(CommunicationConfig::Usb(usb_info)),
            (Some(usb_info), Some(ble_config)) => {
                if !ble_config.enabled {
                    Ok(CommunicationConfig::Usb(usb_info))
                } else {
                    Ok(CommunicationConfig::Both(usb_info, ble_config))
                }
            }
            (None, Some(c)) => {
                if !c.enabled {
                    Err("You must enable at least one of usb or ble".to_string())
                } else {
                    Ok(CommunicationConfig::Ble(c))
                }
            }
            _ => Err("You must enable at least one of usb or ble".to_string()),
        }
    }
}
