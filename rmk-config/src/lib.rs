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
    #[serde(default)]
    pub event: EventConfig,
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
        }
    }
}

/// Event channel configuration for a single event type
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventChannelConfig {
    /// Channel buffer size
    #[serde(default)]
    pub channel_size: Option<usize>,
    /// Number of publishers
    #[serde(default)]
    pub pubs: Option<usize>,
    /// Number of subscribers
    #[serde(default)]
    pub subs: Option<usize>,
}

impl EventChannelConfig {
    /// Merge with defaults: user config takes precedence, fallback to defaults for None fields
    pub fn with_defaults(mut self, defaults: EventChannelConfig) -> Self {
        self.channel_size = self.channel_size.or(defaults.channel_size);
        self.pubs = self.pubs.or(defaults.pubs);
        self.subs = self.subs.or(defaults.subs);
        self
    }

    /// Extract final values (all fields must be Some at this point)
    pub fn into_values(self) -> (usize, usize, usize) {
        (
            self.channel_size.expect("channel_size must be set after with_defaults"),
            self.pubs.expect("pubs must be set after with_defaults"),
            self.subs.expect("subs must be set after with_defaults"),
        )
    }
}

/// Event configuration for all controller events
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventConfig {
    // BLE events
    #[serde(default = "default_ble_state_event")]
    pub ble_state_change: EventChannelConfig,
    #[serde(default = "default_event")]
    pub ble_profile_change: EventChannelConfig,

    // Connection events
    #[serde(default = "default_event")]
    pub connection_change: EventChannelConfig,

    // Input events
    #[serde(default = "default_input_event")]
    pub key: EventChannelConfig,
    #[serde(default = "default_input_event")]
    pub modifier: EventChannelConfig,
    #[serde(default = "default_keyboard_event")]
    pub keyboard: EventChannelConfig,

    // Keyboard state events
    #[serde(default = "default_monitored_event")]
    pub layer_change: EventChannelConfig,
    #[serde(default = "default_event")]
    pub wpm_update: EventChannelConfig,
    #[serde(default = "default_led_indicator_event")]
    pub led_indicator: EventChannelConfig,
    #[serde(default = "default_monitored_event")]
    pub sleep_state: EventChannelConfig,

    // Power events
    #[serde(default = "default_monitored_event")]
    pub battery_state: EventChannelConfig,
    #[serde(default = "default_ble_state_event")]
    pub battery_adc: EventChannelConfig,
    #[serde(default = "default_ble_state_event")]
    pub charging_state: EventChannelConfig,

    // Pointing device events
    #[serde(default = "default_input_event")]
    pub pointing: EventChannelConfig,
    #[serde(default = "default_input_event")]
    pub touchpad: EventChannelConfig,

    // Split events
    #[serde(default = "default_event")]
    pub peripheral_connected: EventChannelConfig,
    #[serde(default = "default_event")]
    pub central_connected: EventChannelConfig,
    #[serde(default = "default_peripheral_battery_event")]
    pub peripheral_battery: EventChannelConfig,
    #[serde(default = "default_monitored_event")]
    pub clear_peer: EventChannelConfig,
}

impl EventConfig {
    /// Apply defaults to all events after deserialization
    pub fn with_defaults(mut self) -> Self {
        self.ble_state_change = self.ble_state_change.with_defaults(default_ble_state_event());
        self.ble_profile_change = self.ble_profile_change.with_defaults(default_event());
        self.connection_change = self.connection_change.with_defaults(default_event());
        self.key = self.key.with_defaults(default_input_event());
        self.modifier = self.modifier.with_defaults(default_input_event());
        self.keyboard = self.keyboard.with_defaults(default_keyboard_event());
        self.layer_change = self.layer_change.with_defaults(default_monitored_event());
        self.wpm_update = self.wpm_update.with_defaults(default_event());
        self.led_indicator = self.led_indicator.with_defaults(default_led_indicator_event());
        self.sleep_state = self.sleep_state.with_defaults(default_monitored_event());
        self.battery_state = self.battery_state.with_defaults(default_monitored_event());
        self.battery_adc = self.battery_adc.with_defaults(default_ble_state_event());
        self.charging_state = self.charging_state.with_defaults(default_ble_state_event());
        self.pointing = self.pointing.with_defaults(default_input_event());
        self.touchpad = self.touchpad.with_defaults(default_input_event());
        self.peripheral_connected = self.peripheral_connected.with_defaults(default_event());
        self.central_connected = self.central_connected.with_defaults(default_event());
        self.peripheral_battery = self
            .peripheral_battery
            .with_defaults(default_peripheral_battery_event());
        self.clear_peer = self.clear_peer.with_defaults(default_monitored_event());
        self
    }
}

// Default event configurations with semantic names

/// Default for simple events: (1, 1, 1)
/// Used by: ble_profile_change, connection_change, wpm_update, peripheral_connected, central_connected
fn default_event() -> EventChannelConfig {
    EventChannelConfig {
        channel_size: Some(1),
        pubs: Some(1),
        subs: Some(1),
    }
}

/// Default for monitored events
fn default_monitored_event() -> EventChannelConfig {
    EventChannelConfig {
        channel_size: Some(1),
        pubs: Some(1),
        subs: Some(4),
    }
}

/// Default for buffered multi-monitored events: (2, 1, 4)
/// LED indicator events with buffering + 4 subscribers
fn default_led_indicator_event() -> EventChannelConfig {
    EventChannelConfig {
        channel_size: Some(2),
        pubs: Some(1),
        subs: Some(4),
    }
}

/// Default for high-frequency input events: (8, 1, 2)
/// Used by: key, modifier
fn default_input_event() -> EventChannelConfig {
    EventChannelConfig {
        channel_size: Some(8),
        pubs: Some(1),
        subs: Some(2),
    }
}

/// Default for BLE state change event: (2, 1, 1)
/// Needs buffering for state transitions
fn default_ble_state_event() -> EventChannelConfig {
    EventChannelConfig {
        channel_size: Some(2),
        pubs: Some(1),
        subs: Some(1),
    }
}

/// Default for peripheral battery monitoring: (2, 1, 2)
/// Buffering for split keyboard battery updates
fn default_peripheral_battery_event() -> EventChannelConfig {
    EventChannelConfig {
        channel_size: Some(2),
        pubs: Some(1),
        subs: Some(2),
    }
}

/// Default for keyboard events: (16, 1, 1)
/// High-frequency keyboard input with large buffer
fn default_keyboard_event() -> EventChannelConfig {
    EventChannelConfig {
        channel_size: Some(16),
        pubs: Some(1),
        subs: Some(1),
    }
}

impl Default for EventConfig {
    fn default() -> Self {
        Self {
            ble_state_change: default_ble_state_event(),
            ble_profile_change: default_event(),
            connection_change: default_event(),
            key: default_input_event(),
            modifier: default_input_event(),
            keyboard: default_keyboard_event(),
            layer_change: default_monitored_event(),
            wpm_update: default_event(),
            led_indicator: default_led_indicator_event(),
            sleep_state: default_monitored_event(),
            battery_state: default_monitored_event(),
            battery_adc: default_ble_state_event(),
            charging_state: default_ble_state_event(),
            pointing: default_input_event(),
            touchpad: default_input_event(),
            peripheral_connected: default_event(),
            central_connected: default_event(),
            peripheral_battery: default_peripheral_battery_event(),
            clear_peer: default_monitored_event(),
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
