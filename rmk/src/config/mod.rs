#[cfg(feature = "_ble")]
mod ble_config;
pub mod macro_config;

#[cfg(feature = "_ble")]
pub use ble_config::BleBatteryConfig;
use embassy_time::Duration;
use heapless::Vec;
use macro_config::KeyboardMacrosConfig;

use crate::combo::Combo;
use crate::fork::Fork;
use crate::morse::{Morse, MorseMode};
use crate::{COMBO_MAX_NUM, FORK_MAX_NUM, MORSE_MAX_NUM};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hand {
    Unknown,
    Left,
    Right,
}

impl Default for Hand {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Config for configurable action behavior
#[derive(Debug, Default)]
pub struct BehaviorConfig {
    pub tri_layer: Option<[u8; 3]>,
    pub tap: TapConfig,
    pub one_shot: OneShotConfig,
    pub combo: CombosConfig,
    pub fork: ForksConfig,
    pub morse: MorsesConfig,
    pub keyboard_macros: KeyboardMacrosConfig,
    pub mouse_key: MouseKeyConfig,
}

/// Configurations for morse behavior
#[derive(Clone, Copy, Debug)]
pub struct TapConfig {
    // TODO: Use `Duration` instead?
    pub tap_interval: u16,
    pub tap_capslock_interval: u16,
}

impl Default for TapConfig {
    fn default() -> Self {
        Self {
            tap_interval: 20,
            tap_capslock_interval: 20,
        }
    }
}

/// Configuration for morse, tap dance, tap-hold and home row mods
#[derive(Clone, Debug)]
pub struct MorsesConfig {
    pub enable_flow_tap: bool,
    pub prior_idle_time: Duration, //used only when flow tap is enabled
    pub default_profile: MorseProfile,

    pub morses: Vec<Morse, MORSE_MAX_NUM>,
}

impl Default for MorsesConfig {
    fn default() -> Self {
        Self {
            enable_flow_tap: false,
            prior_idle_time: Duration::from_millis(120),
            default_profile: MorseProfile::new(Some(false), Some(MorseMode::Normal), Some(250u16), Some(250u16)),
            morses: Vec::new(),
        }
    }
}

/// Configuration for morse, tap dance and tap-hold
/// to save some RAM space, manually packed into 32 bits
#[derive(PartialEq, Eq, Clone, Copy, Default, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MorseProfile(u32);

impl MorseProfile {
    /// If the previous key is on the same "hand", the current key will be determined as a tap
    pub fn unilateral_tap(self) -> Option<bool> {
        match self.0 & 0x0000_C000 {
            0x0000_C000 => Some(true),
            0x0000_8000 => Some(false),
            _ => None,
        }
    }
    pub fn with_unilateral_tap(self, b: Option<bool>) -> Self {
        Self(
            (self.0 & 0xFFFF_3FFF)
                | match b {
                    Some(true) => 0x0000_C000,
                    Some(false) => 0x0000_8000,
                    None => 0,
                },
        )
    }

    /// The decision mode of the morse/tap-hold key
    /// - If neither of them is set, the decision is made when timeout
    /// - If permissive_hold is set, same as QMK's permissive hold:
    ///   When another key is pressed and released while the current morse key is held,
    ///   the hold action of current morse key will be triggered
    ///   https://docs.qmk.fm/tap_hold#tap-or-hold-decision-modes
    /// - if hold_on_other_press is set - triggers hold immediately if any other non-morse
    ///   key is pressed while the current morse key is held    
    pub fn mode(self) -> Option<MorseMode> {
        match self.0 & 0xC000_0000 {
            0xC000_0000 => Some(MorseMode::Normal),
            0x8000_0000 => Some(MorseMode::HoldOnOtherPress),
            0x4000_0000 => Some(MorseMode::PermissiveHold),
            _ => None,
        }
    }
    pub fn with_mode(self, m: Option<MorseMode>) -> Self {
        Self(
            (self.0 & 0x3FFF_FFFF)
                | match m {
                    Some(MorseMode::Normal) => 0xC000_0000,
                    Some(MorseMode::HoldOnOtherPress) => 0x8000_0000,
                    Some(MorseMode::PermissiveHold) => 0x4000_0000,
                    None => 0,
                },
        )
    }

    /// If the key is pressed longer than this, it is accepted as `hold` (in milliseconds)
    /// /// if given, should not be zero
    pub fn hold_timeout_ms(self) -> Option<u16> {
        // NonZero
        let t = (self.0 & 0x3FFF) as u16;
        if t == 0 { None } else { Some(t) }
    }
    pub fn with_hold_timeout_ms(self, t: Option<u16>) -> Self {
        if let Some(t) = t {
            Self((self.0 & 0xFFFF_C000) | (t as u32 & 0x3FFF))
        } else {
            Self(self.0 & 0xFFFF_C000)
        }
    }

    /// The time elapsed from the last release of a key is longer than this, it will break the morse pattern (in milliseconds)
    /// if given, should not be zero
    pub fn gap_timeout_ms(self) -> Option<u16> {
        // NonZero
        let t = ((self.0 >> 16) & 0x3FFF) as u16;
        if t == 0 { None } else { Some(t) }
    }
    pub fn with_gap_timeout_ms(self, t: Option<u16>) -> Self {
        if let Some(t) = t {
            Self((self.0 & 0xC000_FFFF) | ((t as u32 & 0x3FFF) << 16))
        } else {
            Self(self.0 & 0xC000_FFFF)
        }
    }

    pub fn new(
        unilateral_tap: Option<bool>,
        mode: Option<MorseMode>,
        hold_timeout_ms: Option<u16>,
        gap_timeout_ms: Option<u16>,
    ) -> Self {
        let mut v = 0u32;
        if let Some(t) = hold_timeout_ms {
            //zero value also considered as None!
            v = (t & 0x3FFF) as u32;
        }

        if let Some(t) = gap_timeout_ms {
            //zero value also considered as None!
            v |= ((t & 0x3FFF) as u32) << 16;
        }

        if let Some(b) = unilateral_tap {
            v |= if b { 0x0000_C000 } else { 0x0000_8000 };
        }

        if let Some(m) = mode {
            v |= match m {
                MorseMode::Normal => 0xC000_0000,
                MorseMode::HoldOnOtherPress => 0x8000_0000,
                MorseMode::PermissiveHold => 0x4000_0000,
            };
        }

        MorseProfile(v)
    }
}

impl From<u32> for MorseProfile {
    fn from(v: u32) -> Self {
        MorseProfile(v)
    }
}

impl Into<u32> for MorseProfile {
    fn into(self) -> u32 {
        self.0
    }
}

/// Per key position information about a key
/// In the future more fields can be added here for the future configurator GUI
/// - physical key position and orientation
/// - key size,
/// - key shape,
/// - backlight sequence number, etc.
/// IDEA: For Keyboards with low memory, these should be compile time constants to save RAM?
#[derive(Clone, Copy, Default, Debug)]
pub struct KeyInfo {
    /// store hand information for unilateral_tap processing
    pub hand: Hand,
    /// this gives possibility to override some the default MorseProfile setting in certain key positions (typically home row mods)
    pub morse_profile_override: MorseProfile,
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
    pub clear_layout: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            start_addr: 0,
            num_sectors: 2,
            clear_storage: false,
            clear_layout: false,
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
    pub unlock_keys: &'a [(u8, u8)],
}

impl<'a> VialConfig<'a> {
    pub fn new(vial_keyboard_id: &'a [u8], vial_keyboard_def: &'a [u8], unlock_keys: &'a [(u8, u8)]) -> Self {
        Self {
            vial_keyboard_id,
            vial_keyboard_def,
            unlock_keys,
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
