#[cfg(feature = "_ble")]
mod ble_config;
pub mod macro_config;

#[cfg(feature = "_ble")]
pub use ble_config::BleBatteryConfig;
use embassy_time::Duration;
use embedded_hal::digital::OutputPin;
use heapless::Vec;
use macro_config::KeyboardMacrosConfig;

use crate::combo::Combo;
use crate::fork::Fork;
use crate::tap_dance::TapDance;
use crate::{COMBO_MAX_NUM, FORK_MAX_NUM, TAP_DANCE_MAX_NUM};

/// The config struct for RMK keyboard.
///
/// There are 3 types of configs:
/// 1. `ChannelConfig`: Configurations for channels used in RMK.
/// 2. `ControllerConfig`: Config for controllers, the controllers are used for controlling other devices on the board.
/// 3. `RmkConfig`: Tunable configurations for RMK keyboard.
pub struct KeyboardConfig<'a, O: OutputPin> {
    pub controller_config: ControllerConfig<O>,
    pub rmk_config: RmkConfig<'a>,
}

impl<O: OutputPin> Default for KeyboardConfig<'_, O> {
    fn default() -> Self {
        Self {
            controller_config: ControllerConfig::default(),
            rmk_config: RmkConfig::default(),
        }
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
    pub usb_config: KeyboardUsbConfig<'a>,
    pub vial_config: VialConfig<'a>,
    #[cfg(feature = "storage")]
    pub storage_config: StorageConfig,
    #[cfg(feature = "_ble")]
    pub ble_battery_config: BleBatteryConfig<'a>,
}

/// Config for configurable action behavior
#[derive(Debug, Default)]
pub struct BehaviorConfig {
    pub tri_layer: Option<[u8; 3]>,
    pub tap_hold: TapHoldConfig,
    pub one_shot: OneShotConfig,
    pub combo: CombosConfig,
    pub fork: ForksConfig,
    pub tap_dance: TapDancesConfig,
    pub keyboard_macros: KeyboardMacrosConfig,
    pub mouse_key: MouseKeyConfig,
}

/// Configuration for tap dance behavior
#[derive(Clone, Debug)]
pub struct TapDancesConfig {
    pub tap_dances: Vec<TapDance, TAP_DANCE_MAX_NUM>,
}

impl Default for TapDancesConfig {
    fn default() -> Self {
        Self { tap_dances: Vec::new() }
    }
}

/// Configurations for tap hold behavior
#[derive(Clone, Copy, Debug)]
pub struct TapHoldConfig {
    pub enable_hrm: bool,
    pub prior_idle_time: Duration,
    /// Depreciated
    pub post_wait_time: Duration,
    pub hold_timeout: Duration,
    /// Same as QMK's permissive hold: https://docs.qmk.fm/tap_hold#tap-or-hold-decision-modes
    pub permissive_hold: bool,
    /// If the previous key is on the same "hand", the current key will be determined as a tap
    pub chordal_hold: bool,
}

impl Default for TapHoldConfig {
    fn default() -> Self {
        Self {
            enable_hrm: false,
            permissive_hold: false,
            chordal_hold: false,
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

/// Config for mouse key behavior
#[derive(Clone, Copy, Debug)]
pub struct MouseKeyConfig {
    // Accelerated mode parameters
    /// Initial delay between pressing a movement key and first cursor movement (in milliseconds)
    pub initial_delay_ms: u16,
    /// Time between subsequent cursor movements in milliseconds
    pub repeat_interval_ms: u16,
    /// Step size for each movement
    pub move_delta: u8,
    /// Maximum cursor speed at which acceleration stops
    pub max_speed: u8,
    /// Number of repeated movements until maximum cursor speed is reached
    pub time_to_max: u8,
    /// Initial delay between pressing a wheel key and first wheel movement (in milliseconds)
    pub wheel_initial_delay_ms: u16,
    /// Time between subsequent wheel movements in milliseconds
    pub wheel_repeat_interval_ms: u16,
    /// Wheel movement step size
    pub wheel_delta: u8,
    /// Maximum wheel speed
    pub wheel_max_speed_multiplier: u8,
    /// Number of repeated movements until maximum wheel speed is reached
    pub wheel_time_to_max: u8,
    /// Maximum movement distance per report
    pub move_max: u8,
    /// Maximum wheel distance per report
    pub wheel_max: u8,
}

impl Default for MouseKeyConfig {
    fn default() -> Self {
        Self {
            // Optimized values for comfortable and responsive mouse movement
            initial_delay_ms: 100,         // 100ms initial delay
            repeat_interval_ms: 20,        // 20ms between movements
            move_delta: 6,                 // 6 pixels per movement (~300 px/sec)
            max_speed: 3,                  // Conservative max speed multiplier (300 -> 900 px/sec)
            time_to_max: 50,               // 1.0 second to max
            wheel_initial_delay_ms: 100,   // 100ms initial wheel delay
            wheel_repeat_interval_ms: 80,  // 80ms between wheel movements
            wheel_delta: 1,                // 1 wheel unit per movement
            wheel_max_speed_multiplier: 3, // Conservative wheel max speed
            wheel_time_to_max: 40,         // 0.5 second to max
            move_max: 20,                  // Maximum movement per report
            wheel_max: 4,                  // Maximum wheel movement per report
        }
    }
}

impl MouseKeyConfig {
    /// Get the appropriate delay for cursor movement based on repeat count
    pub fn get_movement_delay(&self, repeat_count: u8) -> u16 {
        if repeat_count == 0 {
            self.initial_delay_ms
        } else {
            self.repeat_interval_ms
        }
    }

    /// Get the appropriate delay for wheel movement based on repeat count
    pub fn get_wheel_delay(&self, repeat_count: u8) -> u16 {
        if repeat_count == 0 {
            self.wheel_initial_delay_ms
        } else {
            self.wheel_repeat_interval_ms
        }
    }
}
