// Communication configuration types

use serde::Deserialize;

use super::hardware::PinConfig;

#[derive(Clone, Default, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BleConfig {
    pub enabled: bool,
    pub battery_adc_pin: Option<String>,
    pub charge_state: Option<PinConfig>,
    pub charge_led: Option<PinConfig>,
    pub adc_divider_measured: Option<u32>,
    pub adc_divider_total: Option<u32>,
    pub default_tx_power: Option<i8>,
    pub use_2m_phy: Option<bool>,
}

// Re-export CommunicationConfig and UsbInfo from the communication module
// These are defined in src/communication.rs
pub use crate::communication::{CommunicationConfig, UsbInfo};
