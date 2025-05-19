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
        let default_setting = self.get_default_config().unwrap().communication;
        let chip = self.get_chip_model().unwrap();
        // Get usb config
        let usb_enabled = self.keyboard.usb_enable.unwrap_or(default_setting.usb_enabled());
        let usb_info = if usb_enabled { get_usb_info(&chip.chip) } else { None };

        // Get ble config
        let ble_config = match (&default_setting, &self.ble) {
            (CommunicationConfig::Ble(default), None) | (CommunicationConfig::Both(_, default), None) => {
                Some(default.clone())
            }
            (CommunicationConfig::Ble(default), Some(config))
            | (CommunicationConfig::Both(_, default), Some(config)) => {
                // Use default setting if the corresponding field is not set
                let mut new_config = config.clone();
                new_config.battery_adc_pin = new_config.battery_adc_pin.or_else(|| default.battery_adc_pin.clone());
                new_config.charge_state = new_config.charge_state.or_else(|| default.charge_state.clone());
                new_config.charge_led = new_config.charge_led.or_else(|| default.charge_led.clone());
                new_config.adc_divider_measured = new_config.adc_divider_measured.or(default.adc_divider_measured);
                new_config.adc_divider_total = new_config.adc_divider_total.or(default.adc_divider_total);
                Some(new_config)
            }
            (_, c) => c.clone(),
        };

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
