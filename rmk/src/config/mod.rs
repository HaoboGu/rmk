#[cfg(feature = "_esp_ble")]
mod esp_config;
#[cfg(feature = "_nrf_ble")]
mod nrf_config;

#[cfg(feature = "_esp_ble")]
pub use esp_config::BleBatteryConfig;
#[cfg(feature = "_nrf_ble")]
pub use nrf_config::BleBatteryConfig;

use embassy_time::Duration;
use embedded_hal::digital::OutputPin;

/// Internal configurations for RMK keyboard.
pub struct RmkConfig<'a, O: OutputPin> {
    pub mouse_config: MouseConfig,
    pub usb_config: KeyboardUsbConfig<'a>,
    pub vial_config: VialConfig<'a>,
    pub light_config: LightConfig<O>,
    pub storage_config: StorageConfig,
    pub behavior_config: BehaviorConfig,
    #[cfg(feature = "_nrf_ble")]
    pub ble_battery_config: BleBatteryConfig<'a>,
    #[cfg(feature = "_esp_ble")]
    pub ble_battery_config: BleBatteryConfig,
}

impl<'a, O: OutputPin> Default for RmkConfig<'a, O> {
    fn default() -> Self {
        Self {
            mouse_config: MouseConfig::default(),
            usb_config: KeyboardUsbConfig::default(),
            vial_config: VialConfig::default(),
            light_config: LightConfig::default(),
            storage_config: StorageConfig::default(),
            behavior_config: BehaviorConfig::default(),
            #[cfg(any(feature = "_nrf_ble", feature = "_esp_ble"))]
            ble_battery_config: BleBatteryConfig::default(),
        }
    }
}

/// Config for configurable action behavior
pub struct BehaviorConfig {
    pub tri_layer: Option<[u8; 3]>,
    pub tap_hold: TapHoldConfig,
    pub one_shot: OneShotConfig,
    pub macros: MacrosConfig,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            tri_layer: None,
            tap_hold: TapHoldConfig::default(),
            one_shot: OneShotConfig::default(),
            macros: MacrosConfig::default(),
        }
    }
}
/// Configurations for tap hold behavior
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

/// Configurations for keyboard macros
pub struct MacrosConfig {
    pub count: usize,
}

impl Default for MacrosConfig {
    fn default() -> Self {
        Self { count: 8 }
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
#[derive(Clone, Copy, Debug)]
pub struct LightConfig<O: OutputPin> {
    pub capslock: Option<LightPinConfig<O>>,
    pub scrolllock: Option<LightPinConfig<O>>,
    pub numslock: Option<LightPinConfig<O>>,
}

#[derive(Clone, Copy, Default, Debug)]
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

impl<'a> Default for KeyboardUsbConfig<'a> {
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
