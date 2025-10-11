use std::collections::HashMap;
use std::path::Path;

use config::{Config, File, FileFormat};
use serde::{Deserialize as SerdeDeserialize, de};
use serde_derive::Deserialize;
use serde_inline_default::serde_inline_default;

pub mod chip;
pub mod communication;
pub mod keyboard;
#[rustfmt::skip]
pub mod usb_interrupt_map;
pub mod behavior;
pub mod board;
pub mod keycode_alias;
pub mod layout;
pub mod light;
pub mod storage;

pub use board::{BoardConfig, UniBodyConfig};
pub use chip::{ChipModel, ChipSeries};
pub use communication::{CommunicationConfig, UsbInfo};
pub use keyboard::Basic;
pub use keycode_alias::KEYCODE_ALIAS;

/// Keyboard constants configuration for performance and hardware limits
#[serde_inline_default]
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RmkConstantsConfig {
    /// Mouse key interval (ms) - controls mouse movement speed
    #[serde_inline_default(20)]
    pub mouse_key_interval: u32,
    /// Mouse wheel interval (ms) - controls scrolling speed
    #[serde_inline_default(80)]
    pub mouse_wheel_interval: u32,
    /// Maximum number of combos keyboard can store
    #[serde_inline_default(8)]
    #[serde(deserialize_with = "check_combo_max_num")]
    pub combo_max_num: usize,
    /// Maximum number of keys pressed simultaneously in a combo
    #[serde_inline_default(4)]
    pub combo_max_length: usize,
    /// Maximum number of forks for conditional key actions
    #[serde_inline_default(8)]
    #[serde(deserialize_with = "check_fork_max_num")]
    pub fork_max_num: usize,
    /// Maximum number of morses keyboard can store
    #[serde_inline_default(8)]
    #[serde(deserialize_with = "check_morse_max_num")]
    pub morse_max_num: usize,
    /// Maximum number of patterns a morse key can handle
    #[serde_inline_default(8)]
    #[serde(deserialize_with = "check_max_patterns_per_key")]
    pub max_patterns_per_key: usize,
    /// Macro space size in bytes for storing sequences
    #[serde_inline_default(256)]
    pub macro_space_size: usize,
    /// Default debounce time in ms
    #[serde_inline_default(20)]
    pub debounce_time: u16,
    /// Event channel size
    #[serde_inline_default(16)]
    pub event_channel_size: usize,
    /// Controller event channel size
    #[serde_inline_default(16)]
    pub controller_channel_size: usize,
    /// Number of publishers to controllers
    #[serde_inline_default(8)]
    pub controller_channel_pubs: usize,
    /// Number of controllers
    #[serde_inline_default(8)]
    pub controller_channel_subs: usize,
    /// Report channel size
    #[serde_inline_default(16)]
    pub report_channel_size: usize,
    /// Vial channel size
    #[serde_inline_default(4)]
    pub vial_channel_size: usize,
    /// Flash channel size
    #[serde_inline_default(4)]
    pub flash_channel_size: usize,
    /// The number of the split peripherals
    #[serde_inline_default(1)]
    pub split_peripherals_num: usize,
    /// The number of available BLE profiles
    #[serde_inline_default(3)]
    pub ble_profiles_num: usize,
    /// BLE Split Central sleep timeout in minutes (0 = disabled)
    #[serde_inline_default(0)]
    pub split_central_sleep_timeout_minutes: u32,
    /// Whethe Vial is enabled
    #[serde_inline_default(true)]
    pub vial_enabled: bool,
}

fn check_combo_max_num<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: de::Deserializer<'de>,
{
    let value = SerdeDeserialize::deserialize(deserializer)?;
    if value > 256 {
        panic!("❌ Parse `keyboard.toml` error: combo_max_num must be between 0 and 256, got {value}");
    }
    Ok(value)
}

fn check_morse_max_num<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: de::Deserializer<'de>,
{
    let value = SerdeDeserialize::deserialize(deserializer)?;
    if value > 256 {
        panic!("❌ Parse `keyboard.toml` error: morse_max_num must be between 0 and 256, got {value}");
    }
    Ok(value)
}

fn check_max_patterns_per_key<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: de::Deserializer<'de>,
{
    let value = SerdeDeserialize::deserialize(deserializer)?;
    if value < 4 || value > 65536 {
        panic!("❌ Parse `keyboard.toml` error: max_patterns_per_key must be between 4 and 65566, got {value}");
    }
    Ok(value)
}

fn check_fork_max_num<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: de::Deserializer<'de>,
{
    let value = SerdeDeserialize::deserialize(deserializer)?;
    if value > 256 {
        panic!("❌ Parse `keyboard.toml` error: fork_max_num must be between 0 and 256, got {value}");
    }
    Ok(value)
}

/// This separate Default impl is needed when `[rmk]` section is not set in keyboard.toml
impl Default for RmkConstantsConfig {
    fn default() -> Self {
        Self {
            mouse_key_interval: 20,
            mouse_wheel_interval: 80,
            combo_max_num: 8,
            combo_max_length: 4,
            fork_max_num: 8,
            morse_max_num: 8,
            max_patterns_per_key: 8,
            macro_space_size: 256,
            debounce_time: 20,
            event_channel_size: 16,
            controller_channel_size: 16,
            controller_channel_pubs: 8,
            controller_channel_subs: 8,
            report_channel_size: 16,
            vial_channel_size: 4,
            flash_channel_size: 4,
            split_peripherals_num: 1,
            ble_profiles_num: 3,
            split_central_sleep_timeout_minutes: 0,
            vial_enabled: true,
        }
    }
}

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
    /// Dependency config
    dependency: Option<DependencyConfig>,
    /// Split config
    split: Option<SplitConfig>,
    /// Input device config
    input_device: Option<InputDeviceConfig>,
    /// Unlock keys for the keyboard
    pub security: Option<SecurityConfig>,
    /// RMK config constants
    #[serde(default)]
    pub rmk: RmkConstantsConfig,
}

impl KeyboardTomlConfig {
    pub fn new_from_toml_path<P: AsRef<Path>>(config_toml_path: P) -> Self {
        // The first run, load chip model only
        let user_config = match std::fs::read_to_string(config_toml_path.as_ref()) {
            Ok(s) => match toml::from_str::<KeyboardTomlConfig>(&s) {
                Ok(c) => c,
                Err(e) => panic!("Parse {:?} error: {}", config_toml_path.as_ref(), e.message()),
            },
            Err(e) => panic!("Read keyboard config file {:?} error: {}", config_toml_path.as_ref(), e),
        };
        let default_config_str = user_config.get_chip_model().unwrap().get_default_config_str().unwrap();

        // The second run, load the user config and merge with the default config
        let mut config: KeyboardTomlConfig = Config::builder()
            .add_source(File::from_str(default_config_str, FileFormat::Toml))
            .add_source(File::with_name(config_toml_path.as_ref().to_str().unwrap()))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();

        config.auto_calculate_parameters();

        config
    }

    /// Auto calculate some parameters in toml:
    /// - Update morse_max_num to fit all configured morses
    /// - Update max_patterns_per_key to fit the max number of configured (pattern, action) pairs per morse key
    /// - Update peripheral number based on the number of split boards
    /// - TODO: Update controller number based on the number of split boards
    pub fn auto_calculate_parameters(&mut self) {
        // Update the number of peripherals
        if let Some(split) = &self.split {
            if split.peripheral.len() > self.rmk.split_peripherals_num {
                // eprintln!(
                //     "The number of split peripherals is updated to {} from {}",
                //     split.peripheral.len(),
                //     self.rmk.split_peripherals_num
                // );
                self.rmk.split_peripherals_num = split.peripheral.len();
            }
        }

        if let Some(behavior) = &self.behavior {
            // Update the max_patterns_per_key
            if let Some(morse) = &behavior.morse
                && let Some(morses) = &morse.morses
            {
                let mut max_required_patterns = self.rmk.max_patterns_per_key;

                for morse in morses {
                    let tap_actions_len = morse.tap_actions.as_ref().map(|v| v.len()).unwrap_or(0);
                    let hold_actions_len = morse.hold_actions.as_ref().map(|v| v.len()).unwrap_or(0);

                    let n = tap_actions_len.max(hold_actions_len);
                    if n > 15 {
                        panic!("The number of taps per morse is too large, the max number of taps is 15, got {n}");
                    }

                    let morse_actions_len = morse.morse_actions.as_ref().map(|v| v.len()).unwrap_or(0);

                    max_required_patterns =
                        max_required_patterns.max(tap_actions_len + hold_actions_len + morse_actions_len);
                }
                self.rmk.max_patterns_per_key = max_required_patterns;

                // Update the morse_max_num
                self.rmk.morse_max_num = self.rmk.morse_max_num.max(morses.len());
            }
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
    pub keymap: Option<Vec<Vec<Vec<String>>>>, // will be deprecated in the future
    pub matrix_map: Option<String>,            //temporarily allow both matrix_map and keymap to be set
}

#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub struct LayerTomlConfig {
    pub name: Option<String>,
    pub keys: String,
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
#[serde(deny_unknown_fields)]
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
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct LayoutConfig {
    pub rows: u8,
    pub cols: u8,
    pub layers: u8,
    pub keymap: Vec<Vec<Vec<String>>>,
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

/// Configuration for security
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SecurityConfig {
    pub unlock_keys: Vec<[u8; 2]>,
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
    // - default: EC11 compatible, resolution = 1
    // - e8h7: resolution = 2, reverse = true
    // - resolution: customized resolution, the resolution value and reverse should be specified
    pub phase: Option<String>,
    // Resolution
    pub resolution: Option<u8>,
    // Whether the direction of the rotary encoder is reversed.
    pub reverse: Option<bool>,
    // Use MCU's internal pull-up resistor or not
    #[serde(default = "default_false")]
    pub internal_pullup: bool,
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
