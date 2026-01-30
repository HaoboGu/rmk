use std::collections::HashMap;
use std::path::Path;

use config::{Config, File, FileFormat};
use serde::de;
use serde_derive::Deserialize;
use serde_inline_default::serde_inline_default;

pub mod behavior;
pub mod board;
pub mod chip;
pub mod communication;
pub mod defaults;
pub mod error;
pub mod host;
pub mod keyboard;
pub mod keycode_alias;
pub mod layout;
pub mod light;
pub mod storage;
#[rustfmt::skip]
pub mod usb_interrupt_map;
pub mod validation;

pub use board::{BoardConfig, UniBodyConfig};
pub use chip::{ChipModel, ChipSeries};
pub use communication::{CommunicationConfig, UsbInfo};
pub use error::{ConfigError, ConfigResult};
pub use keyboard::DeviceInfo;
pub use keycode_alias::KEYCODE_ALIAS;

/// Configurations for RMK keyboard.
#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub struct KeyboardTomlConfig {
    /// Basic keyboard info
    keyboard: Option<KeyboardInfo>,
    /// Matrix of the keyboard, only for non-split keyboards
    matrix: Option<MatrixConfig>,
    // Aliases for key maps
    aliases: Option<HashMap<String, String>>,
    // Layers of key maps
    layer: Option<Vec<LayerTomlConfig>>,
    /// Layout config.
    /// For split keyboard, the total row/col should be defined in this section
    layout: Option<LayoutTomlConfig>,
    /// Behavior config
    behavior: Option<BehaviorConfig>,
    /// Light config
    light: Option<LightConfig>,
    /// Storage config
    storage: Option<StorageConfig>,
    /// Ble config
    ble: Option<BleConfig>,
    /// Chip-specific configs (e.g., [chip.nrf52840])
    chip: Option<HashMap<String, ChipConfig>>,
    /// Dependency config
    dependency: Option<DependencyConfig>,
    /// Split config
    split: Option<SplitConfig>,
    /// Input device config
    input_device: Option<InputDeviceConfig>,
    /// Output Pin config
    output: Option<Vec<OutputConfig>>,
    /// Set host configurations
    pub host: Option<HostConfig>,
    /// RMK config constants
    #[serde(default)]
    pub rmk: RmkConstantsConfig,
}

impl KeyboardTomlConfig {
    pub fn new_from_toml_path<P: AsRef<Path>>(config_toml_path: P) -> ConfigResult<Self> {
        let path_str = config_toml_path.as_ref().to_string_lossy().to_string();

        // The first run, load chip model only
        let content = std::fs::read_to_string(config_toml_path.as_ref()).map_err(|e| {
            ConfigError::FileRead {
                path: path_str.clone(),
                message: e.to_string(),
            }
        })?;

        let user_config: KeyboardTomlConfig =
            toml::from_str(&content).map_err(|e| ConfigError::TomlParse {
                path: path_str.clone(),
                message: e.message().to_string(),
            })?;

        let chip_model = user_config.get_chip_model()?;
        let default_config_str = chip_model.get_default_config_str()?;

        // The second run, load the user config and merge with the default config
        let mut config: KeyboardTomlConfig = Config::builder()
            .add_source(File::from_str(default_config_str, FileFormat::Toml))
            .add_source(File::with_name(config_toml_path.as_ref().to_str().unwrap()))
            .build()
            .map_err(|e| ConfigError::TomlParse {
                path: path_str.clone(),
                message: e.to_string(),
            })?
            .try_deserialize()
            .map_err(|e| ConfigError::TomlParse {
                path: path_str.clone(),
                message: e.to_string(),
            })?;

        // Run centralized validation
        validation::validate_config(&config)?;

        config.auto_calculate_parameters();

        Ok(config)
    }

    /// Auto calculate some parameters in toml:
    /// - Update morse_max_num to fit all configured morses
    /// - Update max_patterns_per_key to fit the max number of configured (pattern, action) pairs per morse key
    /// - Update peripheral number based on the number of split boards
    pub fn auto_calculate_parameters(&mut self) {
        // Update the number of peripherals
        if let Some(split) = &self.split {
            if split.peripheral.len() > self.rmk.split_peripherals_num {
                self.rmk.split_peripherals_num = split.peripheral.len();
            }
        }

        if let Some(behavior) = &self.behavior {
            // Update the max_patterns_per_key
            if let Some(morse) = &behavior.morse {
                if let Some(morses) = &morse.morses {
                    let mut max_required_patterns = self.rmk.max_patterns_per_key;

                    for morse in morses {
                        let tap_actions_len =
                            morse.tap_actions.as_ref().map(|v| v.len()).unwrap_or(0);
                        let hold_actions_len =
                            morse.hold_actions.as_ref().map(|v| v.len()).unwrap_or(0);
                        let morse_actions_len =
                            morse.morse_actions.as_ref().map(|v| v.len()).unwrap_or(0);

                        max_required_patterns = max_required_patterns
                            .max(tap_actions_len + hold_actions_len + morse_actions_len);
                    }
                    self.rmk.max_patterns_per_key = max_required_patterns;

                    // Update the morse_max_num
                    self.rmk.morse_max_num = self.rmk.morse_max_num.max(morses.len());
                }
            }
        }
    }

    // Reference getters for cleaner API
    pub fn keyboard(&self) -> Option<&KeyboardInfo> {
        self.keyboard.as_ref()
    }

    pub fn layout(&self) -> Option<&LayoutTomlConfig> {
        self.layout.as_ref()
    }

    pub fn behavior(&self) -> Option<&BehaviorConfig> {
        self.behavior.as_ref()
    }

    pub fn matrix(&self) -> Option<&MatrixConfig> {
        self.matrix.as_ref()
    }

    pub fn split(&self) -> Option<&SplitConfig> {
        self.split.as_ref()
    }

    pub fn ble(&self) -> Option<&BleConfig> {
        self.ble.as_ref()
    }

    pub fn storage(&self) -> Option<&StorageConfig> {
        self.storage.as_ref()
    }

    pub fn light(&self) -> Option<&LightConfig> {
        self.light.as_ref()
    }

    pub fn input_device(&self) -> Option<&InputDeviceConfig> {
        self.input_device.as_ref()
    }

    pub fn aliases(&self) -> Option<&HashMap<String, String>> {
        self.aliases.as_ref()
    }

    pub fn layer(&self) -> Option<&Vec<LayerTomlConfig>> {
        self.layer.as_ref()
    }
}

/// Keyboard constants configuration for performance and hardware limits
#[serde_inline_default]
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RmkConstantsConfig {
    /// Mouse key interval (ms) - controls mouse movement speed
    #[serde_inline_default(defaults::MOUSE_KEY_INTERVAL_MS)]
    pub mouse_key_interval: u32,
    /// Mouse wheel interval (ms) - controls scrolling speed
    #[serde_inline_default(defaults::MOUSE_WHEEL_INTERVAL_MS)]
    pub mouse_wheel_interval: u32,
    /// Maximum number of combos keyboard can store
    #[serde_inline_default(defaults::COMBO_MAX_NUM)]
    pub combo_max_num: usize,
    /// Maximum number of keys pressed simultaneously in a combo
    #[serde_inline_default(defaults::COMBO_MAX_LENGTH)]
    pub combo_max_length: usize,
    /// Maximum number of forks for conditional key actions
    #[serde_inline_default(defaults::FORK_MAX_NUM)]
    pub fork_max_num: usize,
    /// Maximum number of morses keyboard can store
    #[serde_inline_default(defaults::MORSE_MAX_NUM)]
    pub morse_max_num: usize,
    /// Maximum number of patterns a morse key can handle
    #[serde_inline_default(defaults::MAX_PATTERNS_PER_KEY)]
    pub max_patterns_per_key: usize,
    /// Macro space size in bytes for storing sequences
    #[serde_inline_default(defaults::MACRO_SPACE_SIZE)]
    pub macro_space_size: usize,
    /// Default debounce time in ms
    #[serde_inline_default(defaults::DEBOUNCE_TIME_MS)]
    pub debounce_time: u16,
    /// Event channel size
    #[serde_inline_default(defaults::EVENT_CHANNEL_SIZE)]
    pub event_channel_size: usize,
    /// Report channel size
    #[serde_inline_default(defaults::REPORT_CHANNEL_SIZE)]
    pub report_channel_size: usize,
    /// Vial channel size
    #[serde_inline_default(defaults::VIAL_CHANNEL_SIZE)]
    pub vial_channel_size: usize,
    /// Flash channel size
    #[serde_inline_default(defaults::FLASH_CHANNEL_SIZE)]
    pub flash_channel_size: usize,
    /// The number of the split peripherals
    #[serde_inline_default(defaults::SPLIT_PERIPHERALS_NUM)]
    pub split_peripherals_num: usize,
    /// The number of available BLE profiles
    #[serde_inline_default(defaults::BLE_PROFILES_NUM)]
    pub ble_profiles_num: usize,
    /// BLE Split Central sleep timeout in seconds (0 = disabled)
    #[serde_inline_default(defaults::SPLIT_CENTRAL_SLEEP_TIMEOUT_SECONDS)]
    pub split_central_sleep_timeout_seconds: u32,
}

/// This separate Default impl is needed when `[rmk]` section is not set in keyboard.toml
impl Default for RmkConstantsConfig {
    fn default() -> Self {
        Self {
            mouse_key_interval: defaults::MOUSE_KEY_INTERVAL_MS,
            mouse_wheel_interval: defaults::MOUSE_WHEEL_INTERVAL_MS,
            combo_max_num: defaults::COMBO_MAX_NUM,
            combo_max_length: defaults::COMBO_MAX_LENGTH,
            fork_max_num: defaults::FORK_MAX_NUM,
            morse_max_num: defaults::MORSE_MAX_NUM,
            max_patterns_per_key: defaults::MAX_PATTERNS_PER_KEY,
            macro_space_size: defaults::MACRO_SPACE_SIZE,
            debounce_time: defaults::DEBOUNCE_TIME_MS,
            event_channel_size: defaults::EVENT_CHANNEL_SIZE,
            report_channel_size: defaults::REPORT_CHANNEL_SIZE,
            vial_channel_size: defaults::VIAL_CHANNEL_SIZE,
            flash_channel_size: defaults::FLASH_CHANNEL_SIZE,
            split_peripherals_num: defaults::SPLIT_PERIPHERALS_NUM,
            ble_profiles_num: defaults::BLE_PROFILES_NUM,
            split_central_sleep_timeout_seconds: defaults::SPLIT_CENTRAL_SLEEP_TIMEOUT_SECONDS,
        }
    }
}

/// Configurations for keyboard layout
#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub struct LayoutTomlConfig {
    pub rows: u8,
    pub cols: u8,
    pub layers: u8,
    pub matrix_map: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub struct LayerTomlConfig {
    pub name: Option<String>,
    pub keys: String,
    pub encoders: Option<Vec<[String; 2]>>,
}

/// Configurations for keyboard info
#[derive(Clone, Debug, Default, Deserialize)]
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

#[serde_inline_default]
#[derive(Clone, Debug, Default, Deserialize)]
pub struct MatrixConfig {
    #[serde(default)]
    pub matrix_type: MatrixType,
    pub row_pins: Option<Vec<String>>,
    pub col_pins: Option<Vec<String>>,
    pub direct_pins: Option<Vec<Vec<String>>>,
    #[serde_inline_default(true)]
    pub direct_pin_low_active: bool,
    #[serde_inline_default(false)]
    pub row2col: bool,
    pub debouncer: Option<String>,
}

/// Config for storage
#[serde_inline_default]
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    /// Start address of local storage, MUST BE start of a sector.
    /// If start_addr is set to 0(this is the default value), the last `num_sectors` sectors will be used.
    pub start_addr: Option<usize>,
    // Number of sectors used for storage, >= 2.
    pub num_sectors: Option<u8>,
    #[serde_inline_default(true)]
    pub enabled: bool,
    // Clear on the storage at reboot, set this to true if you want to reset the keymap
    pub clear_storage: Option<bool>,
    // Clear on the layout at reboot, set this to true if you want to reset the layout
    pub clear_layout: Option<bool>,
}

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

/// Config for lights
#[derive(Clone, Default, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightConfig {
    pub capslock: Option<PinConfig>,
    pub scrolllock: Option<PinConfig>,
    pub numslock: Option<PinConfig>,
}

/// Config for a single pin
#[derive(Clone, Default, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinConfig {
    pub pin: String,
    pub low_active: bool,
}

/// Configurations for dependencies
#[serde_inline_default]
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyConfig {
    /// Enable defmt log or not
    #[serde_inline_default(true)]
    pub defmt_log: bool,
}

impl Default for DependencyConfig {
    fn default() -> Self {
        Self { defmt_log: true }
    }
}

/// Configurations for keyboard layout
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutConfig {
    pub rows: u8,
    pub cols: u8,
    pub layers: u8,
    pub keymap: Vec<Vec<Vec<String>>>,
    pub encoder_map: Vec<Vec<[String; 2]>>, // Empty if there are no encoders or not configured
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyInfo {
    pub hand: char, // 'L' or 'R' or other chars
}

/// Configurations for actions behavior
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BehaviorConfig {
    pub tri_layer: Option<TriLayerConfig>,
    pub one_shot: Option<OneShotConfig>,
    pub combo: Option<CombosConfig>,
    #[serde(alias = "macro")]
    pub macros: Option<MacrosConfig>,
    pub fork: Option<ForksConfig>,
    pub morse: Option<MorsesConfig>,
}

/// Per Key configurations profiles for morse, tap-hold, etc.
/// overrides the defaults given in TapHoldConfig
#[derive(Clone, Debug, Deserialize, Default)]
pub struct MorseProfile {
    /// if true, tap-hold key will always send tap action when tapped with the same hand only
    pub unilateral_tap: Option<bool>,

    /// The decision mode of the morse/tap-hold key (only one of permissive_hold, hold_on_other_press and normal_mode can be true)
    /// /// if none of them is given, normal mode will be the default
    pub permissive_hold: Option<bool>,
    pub hold_on_other_press: Option<bool>,
    pub normal_mode: Option<bool>,

    /// If the key is pressed longer than this, it is accepted as `hold` (in milliseconds)
    pub hold_timeout: Option<DurationMillis>,

    /// The time elapsed from the last release of a key is longer than this, it will break the morse pattern (in milliseconds)
    pub gap_timeout: Option<DurationMillis>,
}

/// Configurations for tri layer
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TriLayerConfig {
    pub upper: u8,
    pub lower: u8,
    pub adjust: u8,
}

/// Configurations for one shot
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OneShotConfig {
    pub timeout: Option<DurationMillis>,
}

/// Configurations for combos
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombosConfig {
    pub combos: Vec<ComboConfig>,
    pub timeout: Option<DurationMillis>,
}

/// Configurations for combo
#[derive(Clone, Debug, Deserialize)]
pub struct ComboConfig {
    pub actions: Vec<String>,
    pub output: String,
    pub layer: Option<u8>,
}

/// Configurations for macros
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacrosConfig {
    pub macros: Vec<MacroConfig>,
}

/// Configurations for macro
#[derive(Clone, Debug, Deserialize)]
pub struct MacroConfig {
    pub operations: Vec<MacroOperation>,
}

/// Macro operations
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "lowercase")]
pub enum MacroOperation {
    Tap { keycode: String },
    Down { keycode: String },
    Up { keycode: String },
    Delay { duration: DurationMillis },
    Text { text: String },
}

/// Configurations for forks
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForksConfig {
    pub forks: Vec<ForkConfig>,
}

/// Configurations for fork
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForkConfig {
    pub trigger: String,
    pub negative_output: String,
    pub positive_output: String,
    pub match_any: Option<String>,
    pub match_none: Option<String>,
    pub kept_modifiers: Option<String>,
    pub bindable: Option<bool>,
}

/// Configurations for morse keys
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MorsesConfig {
    pub enable_flow_tap: Option<bool>, //default: false
    /// used in permissive_hold mode
    pub prior_idle_time: Option<DurationMillis>,

    /// if true, tap-hold key will always send tap action when tapped with the same hand only
    pub unilateral_tap: Option<bool>,

    /// The decision mode of the morse/tap-hold key (only one of permissive_hold, hold_on_other_press and normal_mode can be true)
    /// if none of them is given, normal mode will be the default
    pub permissive_hold: Option<bool>,
    pub hold_on_other_press: Option<bool>,
    pub normal_mode: Option<bool>,

    /// If the key is pressed longer than this, it is accepted as `hold` (in milliseconds)
    pub hold_timeout: Option<DurationMillis>,

    /// The time elapsed from the last release of a key is longer than this, it will break the morse pattern (in milliseconds)
    pub gap_timeout: Option<DurationMillis>,

    /// these can be used to overrides the defaults given above
    pub profiles: Option<HashMap<String, MorseProfile>>,

    /// the definition of morse / tap dance keys
    pub morses: Option<Vec<MorseConfig>>,
}

/// Configurations for morse
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MorseConfig {
    // name of morse profile (to address BehaviorConfig::morse.profiles[self.profile])
    pub profile: Option<String>,

    pub tap: Option<String>,
    pub hold: Option<String>,
    pub hold_after_tap: Option<String>,
    pub double_tap: Option<String>,
    /// Array of tap actions for each tap count (0-indexed)
    pub tap_actions: Option<Vec<String>>,
    /// Array of hold actions for each tap count (0-indexed)
    pub hold_actions: Option<Vec<String>>,
    /// Array of morse patter->action pairs  count (0-indexed)
    pub morse_actions: Option<Vec<MorseActionPair>>,
}

/// Configurations for morse action pairs
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MorseActionPair {
    pub pattern: String, // for example morse code of "B": "-..." or "_..." or "1000"
    pub action: String,  // "B"
}

/// Configurations for split keyboards
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
    /// Battery ADC pin for this split board
    pub battery_adc_pin: Option<String>,
    /// ADC divider measured value for battery
    pub adc_divider_measured: Option<u32>,
    /// ADC divider total value for battery
    pub adc_divider_total: Option<u32>,
    /// Output Pin config for the split
    pub output: Option<Vec<OutputConfig>>,
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

/// Configuration for host tools
#[serde_inline_default]
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostConfig {
    /// Whether Vial is enabled
    #[serde_inline_default(true)]
    pub vial_enabled: bool,
    /// Unlock keys for Vial (optional)
    pub unlock_keys: Option<Vec<[u8; 2]>>,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            vial_enabled: true,
            unlock_keys: None,
        }
    }
}

/// Configurations for input devices
///
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
#[serde_inline_default]
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
    #[serde_inline_default(defaults::PMW3610_REPORT_HZ)]
    pub report_hz: u16,
}

#[serde_inline_default]
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
    #[serde_inline_default(false)]
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

impl KeyboardTomlConfig {
    pub fn get_output_config(&self) -> ConfigResult<Vec<OutputConfig>> {
        let output_config = self.output.clone();
        let split = self.split.clone();
        match (output_config, split) {
            (None, Some(s)) => Ok(s.central.output.unwrap_or_default()),
            (Some(c), None) => Ok(c),
            (None, None) => Ok(Default::default()),
            _ => Err(ConfigError::Validation {
                field: "output".to_string(),
                message: "Use [[split.output]] to define outputs for split in your keyboard.toml!"
                    .to_string(),
            }),
        }
    }
}

/// A validated keyboard configuration with all required fields guaranteed present.
///
/// This wrapper provides a cleaner API for consumers (like rmk-macro) by:
/// - Eliminating the need for `.unwrap()` calls on getters
/// - Ensuring all validation has been performed upfront
/// - Providing reference-based access to avoid unnecessary cloning
#[derive(Clone, Debug)]
pub struct ValidatedConfig {
    raw: KeyboardTomlConfig,
    chip_model: ChipModel,
    board_config: BoardConfig,
    layout_config: LayoutConfig,
    key_info: Vec<Vec<KeyInfo>>,
    communication_config: CommunicationConfig,
    device_info: keyboard::DeviceInfo,
    behavior_config: BehaviorConfig,
}

impl ValidatedConfig {
    /// Create a validated config from a raw config.
    /// This performs all validation and extracts commonly-used derived values.
    pub fn new(raw: KeyboardTomlConfig) -> ConfigResult<Self> {
        let chip_model = raw.get_chip_model()?;
        let board_config = raw.get_board_config()?;
        let (layout_config, key_info) = raw.get_layout_config()?;
        let communication_config = raw.get_communication_config()?;
        let device_info = raw.get_device_config()?;
        let behavior_config = raw.get_behavior_config()?;

        Ok(Self {
            raw,
            chip_model,
            board_config,
            layout_config,
            key_info,
            communication_config,
            device_info,
            behavior_config,
        })
    }

    /// Load and validate config from a TOML file path
    pub fn from_toml_path<P: AsRef<std::path::Path>>(path: P) -> ConfigResult<Self> {
        let raw = KeyboardTomlConfig::new_from_toml_path(path)?;
        Self::new(raw)
    }

    // Getters that return references (no Result needed, guaranteed valid)

    /// Get chip model (guaranteed valid)
    pub fn chip_model(&self) -> &ChipModel {
        &self.chip_model
    }

    /// Get board config (guaranteed valid)
    pub fn board_config(&self) -> &BoardConfig {
        &self.board_config
    }

    /// Get layout config (guaranteed valid)
    pub fn layout_config(&self) -> &LayoutConfig {
        &self.layout_config
    }

    /// Get key info (guaranteed valid)
    pub fn key_info(&self) -> &Vec<Vec<KeyInfo>> {
        &self.key_info
    }

    /// Get communication config (guaranteed valid)
    pub fn communication_config(&self) -> &CommunicationConfig {
        &self.communication_config
    }

    /// Get device info (guaranteed valid)
    pub fn device_info(&self) -> &keyboard::DeviceInfo {
        &self.device_info
    }

    /// Get behavior config (guaranteed valid)
    pub fn behavior_config(&self) -> &BehaviorConfig {
        &self.behavior_config
    }

    /// Get RMK constants config
    pub fn rmk_config(&self) -> &RmkConstantsConfig {
        &self.raw.rmk
    }

    /// Get host config
    pub fn host_config(&self) -> HostConfig {
        self.raw.host.clone().unwrap_or_default()
    }

    /// Get storage config
    pub fn storage_config(&self) -> StorageConfig {
        self.raw.storage.unwrap_or_default()
    }

    /// Get chip-specific config
    pub fn chip_config(&self) -> ChipConfig {
        self.raw.get_chip_config()
    }

    /// Get dependency config
    pub fn dependency_config(&self) -> DependencyConfig {
        self.raw.get_dependency_config()
    }

    /// Get output config
    pub fn output_config(&self) -> ConfigResult<Vec<OutputConfig>> {
        self.raw.get_output_config()
    }

    /// Access the raw config for advanced use cases
    pub fn raw(&self) -> &KeyboardTomlConfig {
        &self.raw
    }

    // Convenience methods

    /// Check if this is a split keyboard
    pub fn is_split(&self) -> bool {
        matches!(self.board_config, BoardConfig::Split(_))
    }

    /// Check if BLE is enabled
    pub fn is_ble_enabled(&self) -> bool {
        self.communication_config.ble_enabled()
    }

    /// Check if USB is enabled
    pub fn is_usb_enabled(&self) -> bool {
        self.communication_config.usb_enabled()
    }

    /// Get the number of encoders for each board
    pub fn num_encoders(&self) -> Vec<usize> {
        self.board_config.get_num_encoder()
    }

    /// Get matrix dimensions (rows, cols)
    pub fn matrix_dimensions(&self) -> (u8, u8) {
        (self.layout_config.rows, self.layout_config.cols)
    }

    /// Get number of layers
    pub fn num_layers(&self) -> u8 {
        self.layout_config.layers
    }
}
