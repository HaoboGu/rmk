// Input device configuration types

use serde::Deserialize;

use super::common::default_false;

const fn default_pmw3610_report_hz() -> u16 {
    125
}

/// Configurations for input devices
#[derive(Clone, Debug, Default, Deserialize)]
#[allow(unused)]
#[serde(deny_unknown_fields)]
pub struct InputDeviceConfig {
    pub encoder: Option<Vec<EncoderConfig>>,
    pub pointing: Option<Vec<PointingDeviceConfig>>,
    pub joystick: Option<Vec<JoystickConfig>>,
    pub pmw3610: Option<Vec<Pmw3610Config>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[allow(unused)]
#[serde(deny_unknown_fields)]
pub struct JoystickConfig {
    // Name of the joystick
    pub name: String,
    // Pin a of the joystick
    pub pin_x: String,
    // Pin b of the joystick
    pub pin_y: String,
    // Pin z of the joystick
    pub pin_z: String,
    pub transform: Vec<Vec<i16>>,
    pub bias: Vec<i16>,
    pub resolution: u16,
}

/// PMW3610 optical mouse sensor configuration
#[derive(Clone, Debug, Default, Deserialize)]
#[allow(unused)]
#[serde(deny_unknown_fields)]
pub struct Pmw3610Config {
    /// Name of the sensor (used for variable naming)
    pub name: String,
    /// SPI pins
    pub spi: SpiConfig,
    /// Optional motion interrupt pin
    pub motion: Option<String>,
    /// CPI resolution (200-3200, step 200). Optional, uses sensor default if not set.
    pub cpi: Option<u16>,
    /// Invert X axis
    #[serde(default)]
    pub invert_x: bool,
    /// Invert Y axis
    #[serde(default)]
    pub invert_y: bool,
    /// Swap X and Y axes
    #[serde(default)]
    pub swap_xy: bool,
    /// Force awake mode (disable power saving)
    #[serde(default)]
    pub force_awake: bool,
    /// Enable smart mode for better tracking on shiny surfaces
    #[serde(default)]
    pub smart_mode: bool,

    /// Report rate (Hz). Motion will be accumulated and emitted at this rate.
    #[serde(default = "default_pmw3610_report_hz")]
    pub report_hz: u16,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[allow(unused)]
#[serde(deny_unknown_fields)]
pub struct EncoderConfig {
    // Pin a of the encoder
    pub pin_a: String,
    // Pin b of the encoder
    pub pin_b: String,
    // Phase is the working mode of the rotary encoders.
    // Available mode:
    // - default: resolution = 1
    // - resolution: customized resolution, the resolution value and reverse should be specified
    //   A typical [EC11 encoder](https://tech.alpsalpine.com/cms.media/product_catalog_ec_01_ec11e_en_611f078659.pdf)'s resolution is 2
    //   In resolution mode, you can also specify the number of detent and pulses, the resolution will be calculated by `pulse * 4 / detent`
    pub phase: Option<String>,
    // Resolution
    pub resolution: Option<EncoderResolution>,
    // The number of detent
    pub detent: Option<u8>,
    // The number of pulse
    pub pulse: Option<u8>,
    // Whether the direction of the rotary encoder is reversed.
    pub reverse: Option<bool>,
    // Use MCU's internal pull-up resistor or not, defaults to false, the external pull-up resistor is needed
    #[serde(default = "default_false")]
    pub internal_pullup: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
#[serde(deny_unknown_fields, untagged)]
pub enum EncoderResolution {
    Value(u8),
    Derived { detent: u8, pulse: u8 },
}

impl Default for EncoderResolution {
    fn default() -> Self {
        Self::Value(4)
    }
}

/// Pointing device config
#[derive(Clone, Debug, Default, Deserialize)]
#[allow(unused)]
#[serde(deny_unknown_fields)]
pub struct PointingDeviceConfig {
    pub interface: Option<CommunicationProtocol>,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub enum CommunicationProtocol {
    I2c(I2cConfig),
    Spi(SpiConfig),
}

/// SPI config
#[derive(Clone, Debug, Default, Deserialize)]
#[allow(unused)]
pub struct SpiConfig {
    pub instance: String,
    pub sck: String,
    pub mosi: String,
    pub miso: String,
    pub cs: Option<String>,
    pub cpi: Option<u32>,
}

/// I2C config
#[derive(Clone, Debug, Default, Deserialize)]
#[allow(unused)]
pub struct I2cConfig {
    pub instance: String,
    pub sda: String,
    pub scl: String,
    pub address: u8,
}
