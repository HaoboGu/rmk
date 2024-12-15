use serde::de;
use serde_derive::Deserialize;

/// Configurations for RMK keyboard.
#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub struct KeyboardTomlConfig {
    /// Basic keyboard info
    pub keyboard: KeyboardInfo,
    /// Matrix of the keyboard, only for non-split keyboards
    pub matrix: Option<MatrixConfig>,
    /// Layout config.
    /// For split keyboard, the total row/col should be defined in this section
    pub layout: LayoutConfig,
    /// Behavior config
    pub behavior: Option<BehaviorConfig>,
    /// Light config
    pub light: Option<LightConfig>,
    /// Storage config
    pub storage: Option<StorageConfig>,
    /// Ble config
    pub ble: Option<BleConfig>,
    /// Dependency config
    pub dependency: Option<DependencyConfig>,
    /// Split config
    pub split: Option<SplitConfig>,
    /// Input device config
    pub input_device: Option<InputDeviceConfig>,
}

/// Configurations for keyboard info
#[derive(Clone, Debug, Deserialize)]
pub struct KeyboardInfo {
    /// Keyboard name
    pub name: String,
    /// Vender id
    pub vendor_id: u16,
    /// Product id
    pub product_id: u16,
    /// Manufacturer
    pub manufacturer: Option<String>,
    /// Product name, if not set, it will use `name` as default
    pub product_name: Option<String>,
    /// Serial number
    pub serial_number: Option<String>,
    /// Board name(if a supported board is used)
    pub board: Option<String>,
    /// Chip model
    pub chip: Option<String>,
    /// enable usb
    pub usb_enable: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[allow(non_camel_case_types)]
pub enum MatrixType {
    #[default]
    normal,
    direct_pin,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MatrixConfig {
    #[serde(default)]
    pub matrix_type: MatrixType,
    pub input_pins: Option<Vec<String>>,
    pub output_pins: Option<Vec<String>>,
    pub direct_pins: Option<Vec<Vec<String>>>,
    #[serde(default = "default_true")]
    pub direct_pin_low_active: bool,
    #[serde(default = "default_false")]
    pub row2col: bool,
}

/// Config for storage
#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct StorageConfig {
    /// Start address of local storage, MUST BE start of a sector.
    /// If start_addr is set to 0(this is the default value), the last `num_sectors` sectors will be used.
    pub start_addr: Option<usize>,
    // Number of sectors used for storage, >= 2.
    pub num_sectors: Option<u8>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    // Clear on the storage at reboot, set this to true if you want to reset the keymap
    pub clear_storage: Option<bool>,
}

#[derive(Clone, Default, Debug, Deserialize)]
pub struct BleConfig {
    pub enabled: bool,
    pub battery_adc_pin: Option<String>,
    pub charge_state: Option<PinConfig>,
    pub charge_led: Option<PinConfig>,
    pub adc_divider_measured: Option<u32>,
    pub adc_divider_total: Option<u32>,
}

/// Config for lights
#[derive(Clone, Default, Debug, Deserialize)]
pub struct LightConfig {
    pub capslock: Option<PinConfig>,
    pub scrolllock: Option<PinConfig>,
    pub numslock: Option<PinConfig>,
}

/// Config for a single pin
#[derive(Clone, Default, Debug, Deserialize)]
pub struct PinConfig {
    pub pin: String,
    pub low_active: bool,
}

/// Configurations for dependencies
#[derive(Clone, Debug, Deserialize)]
pub struct DependencyConfig {
    /// Enable defmt log or not
    #[serde(default = "default_true")]
    pub defmt_log: bool,
}

impl Default for DependencyConfig {
    fn default() -> Self {
        Self { defmt_log: true }
    }
}

/// Configurations for keyboard layout
#[derive(Clone, Debug, Default, Deserialize)]
pub struct LayoutConfig {
    pub rows: u8,
    pub cols: u8,
    pub layers: u8,
    pub keymap: Vec<Vec<Vec<String>>>,
}

/// Configurations for actions behavior
#[derive(Clone, Debug, Default, Deserialize)]
pub struct BehaviorConfig {
    pub tri_layer: Option<TriLayerConfig>,
    pub tap_hold: Option<TapHoldConfig>,
    pub one_shot: Option<OneShotConfig>,
    pub combo: Option<CombosConfig>,
}

/// Configurations for tap hold
#[derive(Clone, Debug, Deserialize)]
pub struct TapHoldConfig {
    pub enable_hrm: Option<bool>,
    pub prior_idle_time: Option<DurationMillis>,
    pub post_wait_time: Option<DurationMillis>,
    pub hold_timeout: Option<DurationMillis>,
}

/// Configurations for tri layer
#[derive(Clone, Debug, Deserialize)]
pub struct TriLayerConfig {
    pub upper: u8,
    pub lower: u8,
    pub adjust: u8,
}

/// Configurations for one shot
#[derive(Clone, Debug, Deserialize)]
pub struct OneShotConfig {
    pub timeout: Option<DurationMillis>,
}

/// Configurations for combos
#[derive(Clone, Debug, Deserialize)]
pub struct CombosConfig {
    pub combos: Vec<ComboConfig>,
}

/// Configurations for combo
#[derive(Clone, Debug, Deserialize)]
pub struct ComboConfig {
    pub actions: Vec<String>,
    pub output: String,
}

/// Configurations for split keyboards
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SplitConfig {
    pub connection: String,
    pub central: SplitBoardConfig,
    pub peripheral: Vec<SplitBoardConfig>,
}

/// Configurations for each split board
///
/// Either ble_addr or serial must be set, but not both.
#[allow(unused)]
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SplitBoardConfig {
    /// Row number of the split board
    pub rows: usize,
    /// Col number of the split board
    pub cols: usize,
    /// Row offset of the split board
    pub row_offset: usize,
    /// Col offset of the split board
    pub col_offset: usize,
    /// Ble address
    pub ble_addr: Option<[u8; 6]>,
    /// Serial config, the vector length should be 1 for peripheral
    pub serial: Option<Vec<SerialConfig>>,
    /// Matrix config for the split
    pub matrix: MatrixConfig,
    /// Input device config for the split
    pub input_device: Option<InputDeviceConfig>,
}

/// Serial port config
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SerialConfig {
    pub instance: String,
    pub tx_pin: String,
    pub rx_pin: String,
}

/// Duration in milliseconds
#[derive(Clone, Debug, Deserialize)]
pub struct DurationMillis(#[serde(deserialize_with = "parse_duration_millis")] pub u64);

const fn default_true() -> bool {
    true
}

const fn default_false() -> bool {
    false
}

fn parse_duration_millis<'de, D: de::Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    let input: String = de::Deserialize::deserialize(deserializer)?;
    let num = input.trim_end_matches(|c: char| !c.is_numeric());
    let unit = &input[num.len()..];
    let num: u64 = num.parse().map_err(|_| {
        de::Error::custom(format!(
            "Invalid number \"{num}\" in duration: number part must be a u64"
        ))
    })?;

    match unit {
        "s" => Ok(num * 1000),
        "ms" => Ok(num),
        other => Err(de::Error::custom(format!(
            "Invalid duration unit \"{other}\": unit part must be either \"s\" or \"ms\""
        ))),
    }
}

/// Configurations for input devices
///
#[derive(Clone, Debug, Default, Deserialize)]
#[allow(unused)]
pub struct InputDeviceConfig {
    pub encoder: Option<Vec<EncoderConfig>>,
    pub pointing: Option<Vec<PointingDeviceConfig>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[allow(unused)]
pub struct EncoderConfig {
    // Pin a of the encoder
    pub pin_a: String,
    // Pin b of the encoder
    pub pin_b: String,
    // Press button position in the keyboard matrix
    // TODO: direct pin support?
    pub btn_pos: Option<(u8, u8)>,
    // Resolution
    pub resolution: Option<u8>,
    pub clockwise_pos: (u8, u8),
    pub counter_clockwise_pos: (u8, u8),
}

/// Pointing device config
#[derive(Clone, Debug, Default, Deserialize)]
#[allow(unused)]
pub struct PointingDeviceConfig {
    pub interface: Option<CommunicationProtocol>,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub enum CommunicationProtocol {
    I2C(I2cConfig),
    SPI(SpiConfig),
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
