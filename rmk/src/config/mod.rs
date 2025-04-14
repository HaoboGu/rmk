#[cfg(feature = "_esp_ble")]
mod esp_config;
#[cfg(feature = "_nrf_ble")]
mod nrf_config;

use ::heapless::Vec;
use embassy_sync::channel::Channel;
use embassy_time::Duration;
use embedded_hal::digital::OutputPin;
#[cfg(feature = "_esp_ble")]
pub use esp_config::BleBatteryConfig;
#[cfg(feature = "_nrf_ble")]
pub use nrf_config::BleBatteryConfig;

use crate::combo::{Combo, COMBO_MAX_NUM};
use crate::event::{Event, KeyEvent};
use crate::fork::{Fork, FORK_MAX_NUM};
use crate::hid::Report;
use crate::light::LedIndicator;
#[cfg(feature = "storage")]
use crate::storage::FlashOperationMessage;
use crate::RawMutex;

/// The config struct for RMK keyboard.
///
/// There are 3 types of configs:
/// 1. `ChannelConfig`: Configurations for channels used in RMK.
/// 2. `ControllerConfig`: Config for controllers, the controllers are used for controlling other devices on the board.
/// 3. `RmkConfig`: Tunable configurations for RMK keyboard.
pub struct KeyboardConfig<'a, O: OutputPin> {
    pub channel_config: ChannelConfig,
    pub controller_config: ControllerConfig<O>,
    pub rmk_config: RmkConfig<'a>,
}

impl<O: OutputPin> Default for KeyboardConfig<'_, O> {
    fn default() -> Self {
        Self {
            channel_config: ChannelConfig::default(),
            controller_config: ControllerConfig::default(),
            rmk_config: RmkConfig::default(),
        }
    }
}

/// Configurations for channels used in RMK
pub struct ChannelConfig<
    const KEY_EVENT_CHANNEL_SIZE: usize = 16,
    const EVENT_CHANNEL_SIZE: usize = 16,
    const REPORT_CHANNEL_SIZE: usize = 16,
> {
    pub key_event_channel: Channel<RawMutex, KeyEvent, KEY_EVENT_CHANNEL_SIZE>,
    pub event_channel: Channel<RawMutex, Event, EVENT_CHANNEL_SIZE>,
    pub keyboard_report_channel: Channel<RawMutex, Report, REPORT_CHANNEL_SIZE>,
    #[cfg(feature = "storage")]
    pub(crate) flash_channel: Channel<RawMutex, FlashOperationMessage, 4>,
    pub(crate) led_channel: Channel<RawMutex, LedIndicator, 4>,
    pub(crate) vial_read_channel: Channel<RawMutex, [u8; 32], 4>,
}

impl<const KEY_EVENT_CHANNEL_SIZE: usize, const EVENT_CHANNEL_SIZE: usize, const REPORT_CHANNEL_SIZE: usize> Default
    for ChannelConfig<KEY_EVENT_CHANNEL_SIZE, EVENT_CHANNEL_SIZE, REPORT_CHANNEL_SIZE>
{
    fn default() -> Self {
        Self {
            key_event_channel: Channel::new(),
            event_channel: Channel::new(),
            keyboard_report_channel: Channel::new(),
            #[cfg(feature = "storage")]
            flash_channel: Channel::new(),
            led_channel: Channel::new(),
            vial_read_channel: Channel::new(),
        }
    }
}

impl<const KEY_EVENT_CHANNEL_SIZE: usize, const EVENT_CHANNEL_SIZE: usize, const REPORT_CHANNEL_SIZE: usize>
    ChannelConfig<KEY_EVENT_CHANNEL_SIZE, EVENT_CHANNEL_SIZE, REPORT_CHANNEL_SIZE>
{
    pub fn new() -> Self {
        Self::default()
    }
}

/// Config for controllers.
///
/// Controllers are used for controlling other devices on the board, such as lights, RGB, etc.
pub struct ControllerConfig<O: OutputPin> {
    pub light_config: LightConfig<O>,
}

impl<O: OutputPin> Default for ControllerConfig<O> {
    fn default() -> Self {
        Self {
            light_config: LightConfig::default(),
        }
    }
}

/// Internal configurations for RMK keyboard.
#[derive(Default)]
pub struct RmkConfig<'a> {
    pub mouse_config: MouseConfig,
    pub usb_config: KeyboardUsbConfig<'a>,
    pub vial_config: VialConfig<'a>,
    pub storage_config: StorageConfig,
    pub behavior_config: BehaviorConfig,
    #[cfg(feature = "_nrf_ble")]
    pub ble_battery_config: BleBatteryConfig<'a>,
    #[cfg(feature = "_esp_ble")]
    pub ble_battery_config: BleBatteryConfig<'a>,
}

/// Config for configurable action behavior
#[derive(Clone, Debug, Default)]
pub struct BehaviorConfig {
    pub tri_layer: Option<[u8; 3]>,
    pub tap_hold: TapHoldConfig,
    pub one_shot: OneShotConfig,
    pub combo: CombosConfig,
    pub fork: ForksConfig,
}

/// Configurations for tap hold behavior
#[derive(Clone, Copy, Debug)]
pub struct TapHoldConfig {
    pub enable_hrm: bool,
    pub prior_idle_time: Duration,
    pub post_wait_time: Duration,
    pub hold_timeout: Duration,
}

impl Default for TapHoldConfig {
    fn default() -> Self {
        Self {
            enable_hrm: false,
            prior_idle_time: Duration::from_millis(120),
            post_wait_time: Duration::from_millis(50),
            hold_timeout: Duration::from_millis(250),
        }
    }
}

/// Config for one shot behavior
#[derive(Clone, Copy, Debug)]
pub struct OneShotConfig {
    pub timeout: Duration,
}

impl Default for OneShotConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(1),
        }
    }
}

/// Config for combo behavior
#[derive(Clone, Debug)]
pub struct CombosConfig {
    pub combos: Vec<Combo, COMBO_MAX_NUM>,
    pub timeout: Duration,
}

impl Default for CombosConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(50),
            combos: Vec::new(),
        }
    }
}

/// Config for fork behavior
#[derive(Clone, Debug)]
pub struct ForksConfig {
    pub forks: Vec<Fork, FORK_MAX_NUM>,
}

impl Default for ForksConfig {
    fn default() -> Self {
        Self { forks: Vec::new() }
    }
}

/// Config for storage
#[derive(Clone, Copy, Debug)]
pub struct StorageConfig {
    /// Start address of local storage, MUST BE start of a sector.
    /// If start_addr is set to 0(this is the default value), the last `num_sectors` sectors will be used.
    pub start_addr: usize,
    // Number of sectors used for storage, >= 2.
    pub num_sectors: u8,
    pub clear_storage: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            start_addr: 0,
            num_sectors: 2,
            clear_storage: false,
        }
    }
}

/// Config for lights
pub struct LightConfig<O: OutputPin> {
    pub capslock: Option<LightPinConfig<O>>,
    pub scrolllock: Option<LightPinConfig<O>>,
    pub numslock: Option<LightPinConfig<O>>,
}

pub struct LightPinConfig<O: OutputPin> {
    pub pin: O,
    pub low_active: bool,
}

impl<O: OutputPin> Default for LightConfig<O> {
    fn default() -> Self {
        Self {
            capslock: None,
            scrolllock: None,
            numslock: None,
        }
    }
}

/// Config for [vial](https://get.vial.today/).
///
/// You can generate automatically using [`build.rs`](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/stm32h7/build.rs).
#[derive(Clone, Copy, Debug, Default)]
pub struct VialConfig<'a> {
    pub vial_keyboard_id: &'a [u8],
    pub vial_keyboard_def: &'a [u8],
}

impl<'a> VialConfig<'a> {
    pub fn new(vial_keyboard_id: &'a [u8], vial_keyboard_def: &'a [u8]) -> Self {
        Self {
            vial_keyboard_id,
            vial_keyboard_def,
        }
    }
}

/// Configuration for debouncing
pub struct DebounceConfig {
    /// Debounce time in ms
    pub debounce_time: u32,
}

/// Configurations for mouse functionalities
#[derive(Clone, Copy, Debug)]
pub struct MouseConfig {
    /// Time interval in ms of reporting mouse cursor states
    pub mouse_key_interval: u32,
    /// Time interval in ms of reporting mouse wheel states
    pub mouse_wheel_interval: u32,
}

impl Default for MouseConfig {
    fn default() -> Self {
        Self {
            mouse_key_interval: 20,
            mouse_wheel_interval: 80,
        }
    }
}

/// Configurations for RGB light
#[derive(Clone, Copy, Debug)]
pub struct RGBLightConfig {
    pub enabled: bool,
    pub rgb_led_num: u32,
    pub rgb_hue_step: u32,
    pub rgb_val_step: u32,
    pub rgb_sat_step: u32,
}

/// Configurations for usb
#[derive(Clone, Copy, Debug)]
pub struct KeyboardUsbConfig<'a> {
    /// Vender id
    pub vid: u16,
    /// Product id
    pub pid: u16,
    /// Manufacturer
    pub manufacturer: &'a str,
    /// Product name
    pub product_name: &'a str,
    /// Serial number
    pub serial_number: &'a str,
}

impl Default for KeyboardUsbConfig<'_> {
    fn default() -> Self {
        Self {
            vid: 0x4c4b,
            pid: 0x4643,
            manufacturer: "RMK",
            product_name: "RMK Keyboard",
            serial_number: "vial:f64c2b3c:000001",
        }
    }
}
