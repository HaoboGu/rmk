use std::collections::HashMap;
use std::path::Path;

use config::{Config, File, FileFormat};
use serde::{Deserialize as SerdeDeserialize, de};
use serde_derive::Deserialize;
use serde_inline_default::serde_inline_default;

/// Event channel default configuration
const EVENT_DEFAULT_CONFIG: &str = include_str!("default_config/event_default.toml");

pub mod chip;
pub mod communication;
pub mod keyboard;
#[rustfmt::skip]
pub mod usb_interrupt_map;
pub mod behavior;
pub mod board;
pub mod host;
pub mod keycode_alias;
pub mod layout;
pub mod light;
pub mod storage;

pub use board::{BoardConfig, UniBodyConfig};
pub use chip::{ChipModel, ChipSeries};
pub use communication::{CommunicationConfig, UsbInfo};
pub use keyboard::Basic;
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
    /// Event channel configuration
    /// Default values are loaded from event_default.toml in new_from_toml_path()
    /// build.rs also loads event defaults via new_from_toml_path_with_event_defaults()
    #[serde(default)]
    pub event: EventConfig,
}

impl KeyboardTomlConfig {
    fn parse_from_toml_path<P: AsRef<Path>>(config_toml_path: P, chip_default_config: Option<&str>) -> Self {
        let path = config_toml_path.as_ref();
        let path_str = path
            .to_str()
            .unwrap_or_else(|| panic!("Config path is not valid UTF-8: {:?}", path));

        let mut builder = Config::builder().add_source(File::from_str(EVENT_DEFAULT_CONFIG, FileFormat::Toml));
        if let Some(default_config) = chip_default_config {
            builder = builder.add_source(File::from_str(default_config, FileFormat::Toml));
        }
        builder
            .add_source(File::with_name(path_str))
            .build()
            .unwrap_or_else(|e| panic!("Parse {:?} error: {}", path, e))
            .try_deserialize()
            .unwrap_or_else(|e| panic!("Deserialize {:?} error: {}", path, e))
    }

    /// Load keyboard.toml with event defaults only.
    ///
    /// This is used in build.rs where we only need [rmk] and [event] constants,
    /// and should not require `[keyboard.board]`/`[keyboard.chip]`.
    pub fn new_from_toml_path_with_event_defaults<P: AsRef<Path>>(config_toml_path: P) -> Self {
        let mut config = Self::parse_from_toml_path(config_toml_path, None);
        config.auto_calculate_parameters();
        config
    }

    pub fn new_from_toml_path<P: AsRef<Path>>(config_toml_path: P) -> Self {
        let path = config_toml_path.as_ref();

        // First pass: load user config with event defaults to get chip model.
        // This allows user's keyboard.toml to omit [event] section.
        let user_config = Self::parse_from_toml_path(path, None);

        let default_config_str = user_config.get_chip_model().unwrap().get_default_config_str().unwrap();

        // Second pass: load with all three config sources
        // Config priority (later sources override earlier ones):
        // 1. Event default config (lowest priority)
        // 2. Chip-specific default config
        // 3. User config (highest priority)
        let mut config = Self::parse_from_toml_path(path, Some(default_config_str));

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
        if let Some(split) = &self.split
            && split.peripheral.len() > self.rmk.split_peripherals_num
        {
            // eprintln!(
            //     "The number of split peripherals is updated to {} from {}",
            //     split.peripheral.len(),
            //     self.rmk.split_peripherals_num
            // );
            self.rmk.split_peripherals_num = split.peripheral.len();
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

/// Keyboard constants configuration for performance and hardware limits
#[serde_inline_default]
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RmkConstantsConfig {
    /// Mouse key interval (ms) - controls mouse movement speed
    #[serde_inline_default(20)]
    pub mouse_key_interval: u16,
    /// Mouse wheel interval (ms) - controls scrolling speed
    #[serde_inline_default(80)]
    pub mouse_wheel_interval: u16,
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
    pub split_central_sleep_timeout_seconds: u32,
    /// Maximum size of user event payload for split forwarding
    #[serde_inline_default(16)]
    pub split_user_payload_max_size: usize,
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
    if !(4..=65536).contains(&value) {
        panic!("❌ Parse `keyboard.toml` error: max_patterns_per_key must be between 4 and 65536, got {value}");
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
            report_channel_size: 16,
            vial_channel_size: 4,
            flash_channel_size: 4,
            split_peripherals_num: 0,
            ble_profiles_num: 3,
            split_central_sleep_timeout_seconds: 0,
            split_user_payload_max_size: 16,
        }
    }
}

/// Event channel configuration for a single event type
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventChannelConfig {
    /// Channel buffer size
    pub channel_size: usize,
    /// Number of publishers
    pub pubs: usize,
    /// Number of subscribers
    pub subs: usize,
}

impl Default for EventChannelConfig {
    fn default() -> Self {
        Self {
            channel_size: 1,
            pubs: 1,
            subs: 1,
        }
    }
}

impl EventChannelConfig {
    /// Extract values as tuple
    pub fn into_values(self) -> (usize, usize, usize) {
        (self.channel_size, self.pubs, self.subs)
    }
}

/// Macro to define EventConfig and related code without repetition
macro_rules! define_event_config {
    ($($field:ident),* $(,)?) => {
        /// Event configuration for all controller events
        /// Default values are loaded from event_default.toml
        #[derive(Clone, Debug, Deserialize)]
        #[serde(deny_unknown_fields, default)]
        pub struct EventConfig {
            $(pub $field: EventChannelConfig,)*
        }

        /// Cached default EventConfig parsed from event_default.toml
        static EVENT_CONFIG_DEFAULTS: std::sync::LazyLock<EventConfig> = std::sync::LazyLock::new(|| {
            #[derive(Deserialize)]
            struct Inner { $($field: EventChannelConfig,)* }
            #[derive(Deserialize)]
            struct Wrapper { event: Inner }
            let w: Wrapper = toml::from_str(EVENT_DEFAULT_CONFIG).expect("Failed to parse event_default.toml");
            EventConfig { $($field: w.event.$field,)* }
        });

        impl Default for EventConfig {
            fn default() -> Self {
                EVENT_CONFIG_DEFAULTS.clone()
            }
        }
    };
}

define_event_config!(
    // BLE events
    ble_state_change,
    ble_profile_change,
    // Connection events
    connection_change,
    // Input events
    modifier,
    keyboard,
    // Keyboard state events
    layer_change,
    wpm_update,
    led_indicator,
    sleep_state,
    // Power events
    battery_state,
    battery_adc,
    charging_state,
    // Pointing device events
    pointing,
    // Split events
    peripheral_connected,
    central_connected,
    peripheral_battery,
    clear_peer,
    // Split forwarding channels
    split_forward,
    split_dispatch,
);

/// Configurations for keyboard layout
#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub struct LayoutTomlConfig {
    pub rows: u8,
    pub cols: u8,
    pub layers: u8,
    pub keymap: Option<Vec<Vec<Vec<String>>>>, // Will be deprecated in the future
    pub matrix_map: Option<String>,            // Temporarily allow both matrix_map and keymap to be set
    pub encoder_map: Option<Vec<Vec<[String; 2]>>>, // Will be deprecated together with keymap
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

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MatrixConfig {
    #[serde(default)]
    pub matrix_type: MatrixType,
    pub row_pins: Option<Vec<String>>,
    pub col_pins: Option<Vec<String>>,
    pub direct_pins: Option<Vec<Vec<String>>>,
    #[serde(default = "default_true")]
    pub direct_pin_low_active: bool,
    #[serde(default = "default_false")]
    pub row2col: bool,
    pub debouncer: Option<String>,
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

const fn default_true() -> bool {
    true
}

const fn default_false() -> bool {
    false
}

const fn default_pointing_report_hz() -> u16 {
    125
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
    pub pmw33xx: Option<Vec<Pmw33xxConfig>>,
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
    /// id of the device
    pub id: Option<u8>,
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
    #[serde(default = "default_pointing_report_hz")]
    pub report_hz: u16,
    #[serde(default)]
    pub proc_invert_x: bool,
    /// Invert Y axis
    #[serde(default)]
    pub proc_invert_y: bool,
    /// Swap X and Y axes
    #[serde(default)]
    pub proc_swap_xy: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[allow(unused)]
#[serde(deny_unknown_fields)]
pub enum Pmw33xxType {
    #[default]
    PMW3360,
    PMW3389,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[allow(unused)]
#[serde(deny_unknown_fields)]
pub struct Pmw33xxConfig {
    // Name of the sensor (used for variable naming)
    pub name: String,
    // id of the device
    pub id: Option<u8>,
    // Sensor Type (3360 or 3389)
    pub sensor_type: Pmw33xxType,
    // SPI pins
    pub spi: SpiConfig,
    // Optional motion interrupt pin
    pub motion: Option<String>,
    // CPI resolution (100-12000, step 100).Optional, uses sensor default 1600 if not set.
    pub cpi: Option<u16>,
    // Rotational transform angle (-127 to 127) Optional, uses sensor default 0 if not set.
    pub rot_trans_angle: Option<i8>,
    // liftoff distance. Optional, uses sensor default 0 if not set.
    pub liftoff_dist: Option<u8>,
    // Invert X axis
    #[serde(default)]
    pub proc_invert_x: bool,
    // Invert Y axis
    #[serde(default)]
    pub proc_invert_y: bool,
    // Swap X and Y axes
    #[serde(default)]
    pub proc_swap_xy: bool,
    /// Report rate (Hz). Motion will be accumulated and emitted at this rate.
    #[serde(default = "default_pointing_report_hz")]
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
    pub tx_dma: Option<String>,
    pub rx_dma: Option<String>,
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
    pub fn get_output_config(&self) -> Result<Vec<OutputConfig>, String> {
        let output_config = self.output.clone();
        let split = self.split.clone();
        match (output_config, split) {
            (None, Some(s)) => Ok(s.central.output.unwrap_or_default()),
            (Some(c), None) => Ok(c),
            (None, None) => Ok(Default::default()),
            _ => Err("Use [[split.output]] to define outputs for split in your keyboard.toml!".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_config_default_values() {
        let config = EventConfig::default();

        // Check some key default values from event_default.toml
        assert_eq!(config.keyboard.channel_size, 16);
        assert_eq!(config.keyboard.pubs, 2);
        assert_eq!(config.keyboard.subs, 2);

        assert_eq!(config.modifier.channel_size, 8);
        assert_eq!(config.modifier.pubs, 1);
        assert_eq!(config.modifier.subs, 2);

        assert_eq!(config.layer_change.channel_size, 1);
        assert_eq!(config.layer_change.subs, 1);

        assert_eq!(config.led_indicator.channel_size, 2);
        assert_eq!(config.led_indicator.pubs, 2);
        assert_eq!(config.led_indicator.subs, 4);

        assert_eq!(config.pointing.channel_size, 8);
        assert_eq!(config.pointing.subs, 2);
    }

    #[test]
    fn test_event_config_user_override() {
        // Simulate user config that overrides some event settings
        let user_toml = r#"
[event.keyboard]
channel_size = 32
"#;
        // Parse with event defaults first, then user config
        let config: KeyboardTomlConfig = Config::builder()
            .add_source(File::from_str(EVENT_DEFAULT_CONFIG, FileFormat::Toml))
            .add_source(File::from_str(user_toml, FileFormat::Toml))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();

        // User-overridden values
        assert_eq!(config.event.keyboard.channel_size, 32);
        assert_eq!(config.event.keyboard.pubs, 2);
        assert_eq!(config.event.keyboard.subs, 2);

        // Non-overridden values should use defaults
        assert_eq!(config.event.modifier.channel_size, 8);
        assert_eq!(config.event.modifier.subs, 2);
        assert_eq!(config.event.layer_change.subs, 1);
    }

    #[test]
    fn test_event_config_partial_override_with_event_defaults_loader() {
        let user_toml = r#"
[event.layer_change]
subs = 2
"#;
        let path = std::env::temp_dir().join(format!(
            "rmk-event-defaults-loader-{}-{}.toml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, user_toml).unwrap();

        let config = KeyboardTomlConfig::new_from_toml_path_with_event_defaults(&path);
        std::fs::remove_file(path).unwrap();

        assert_eq!(config.event.layer_change.channel_size, 1);
        assert_eq!(config.event.layer_change.pubs, 2);
        assert_eq!(config.event.layer_change.subs, 2);
    }
}
