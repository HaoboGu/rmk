// Hardware configuration types

use serde::Deserialize;

use super::common::default_false;

/// Config for chip-specific settings
#[derive(Clone, Default, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChipConfig {
    /// DCDC regulator 0 enabled (for nrf52840)
    pub dcdc_reg0: Option<bool>,
    /// DCDC regulator 1 enabled (for nrf52840, nrf52833)
    pub dcdc_reg1: Option<bool>,
    /// DCDC regulator 0 voltage (for nrf52840)
    /// Values: "3V3" or "1V8"
    pub dcdc_reg0_voltage: Option<String>,
}

/// Config for a single pin
#[derive(Clone, Default, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinConfig {
    pub pin: String,
    pub low_active: bool,
}

/// Configuration for an output pin
#[allow(unused)]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputConfig {
    pub pin: String,
    #[serde(default)]
    pub low_active: bool,
    #[serde(default)]
    pub initial_state_active: bool,
}

