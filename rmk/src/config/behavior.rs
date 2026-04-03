use embassy_time::Duration;
use heapless::Vec;
use rmk_types::action::{MorseMode, MorseProfile};

use rmk_types::fork::Fork;

use crate::combo::Combo;
use crate::morse::Morse;
use crate::{
    COMBO_MAX_NUM, FORK_MAX_NUM, MACRO_SPACE_SIZE, MORSE_MAX_NUM, MOUSE_KEY_INTERVAL, MOUSE_WHEEL_INTERVAL,
};

/// Config for configurable action behavior
#[derive(Debug, Default)]
pub struct BehaviorConfig {
    pub tri_layer: Option<[u8; 3]>,
    pub tap: TapConfig,
    pub one_shot: OneShotConfig,
    pub one_shot_modifiers: OneShotModifiersConfig,
    pub combo: CombosConfig,
    pub fork: ForksConfig,
    pub morse: MorsesConfig,
    pub keyboard_macros: KeyboardMacrosConfig,
    pub mouse_key: MouseKeyConfig,
}

/// Configurations for tap behavior
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

/// Config for one shot behavior
#[derive(Clone, Copy, Debug)]
pub struct OneShotConfig {
    /// Timeout after which modifiers/layers are canceled/released
    pub timeout: Duration,
}

impl Default for OneShotConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(1),
        }
    }
}
/// Config for one-shot behavior
#[derive(Clone, Copy, Debug)]
pub struct OneShotModifiersConfig {
    /// Should modifiers be active from keypress (sticky modifiers)
    pub activate_on_keypress: bool,
}

impl Default for OneShotModifiersConfig {
    fn default() -> Self {
        Self {
            activate_on_keypress: false,
        }
    }
}

/// Config for combo behavior
#[derive(Clone, Debug)]
pub struct CombosConfig {
    pub combos: [Option<Combo>; COMBO_MAX_NUM],
    pub timeout: Duration,
}

impl Default for CombosConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(50),
            combos: core::array::from_fn(|_| None),
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

#[derive(Debug)]
pub struct KeyboardMacrosConfig {
    /// macros stored in biunary format to be compatible with Vial
    pub macro_sequences: [u8; MACRO_SPACE_SIZE],
}

impl Default for KeyboardMacrosConfig {
    fn default() -> Self {
        Self {
            macro_sequences: [0; MACRO_SPACE_SIZE],
        }
    }
}

impl KeyboardMacrosConfig {
    pub fn new(macro_sequences: [u8; MACRO_SPACE_SIZE]) -> Self {
        Self { macro_sequences }
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
    /// Number of repeat ticks until maximum cursor speed is reached
    pub ticks_to_max: u8,
    /// Initial delay between pressing a wheel key and first wheel movement (in milliseconds)
    pub wheel_initial_delay_ms: u16,
    /// Time between subsequent wheel movements in milliseconds
    pub wheel_repeat_interval_ms: u16,
    /// Wheel movement step size
    pub wheel_delta: u8,
    /// Maximum wheel speed
    pub wheel_max_speed: u8,
    /// Number of repeat ticks until maximum wheel speed is reached
    pub wheel_ticks_to_max: u8,
    /// Maximum movement distance per report
    pub move_max: u8,
    /// Maximum wheel distance per report
    pub wheel_max: u8,
}

impl Default for MouseKeyConfig {
    fn default() -> Self {
        Self {
            // Optimized values for comfortable and responsive mouse movement
            initial_delay_ms: 100,                          // 100ms initial delay
            repeat_interval_ms: MOUSE_KEY_INTERVAL,         // 20ms between movements
            move_delta: 5,                                  // 5 pixels per movement (~250 px/sec)
            max_speed: 3,                                   // Max speed multiplier (250 -> 750 px/sec)
            ticks_to_max: 50,                               // 50 ticks to max speed (~1s)
            wheel_initial_delay_ms: 100,                    // 100ms initial wheel delay
            wheel_repeat_interval_ms: MOUSE_WHEEL_INTERVAL, // 80ms between wheel movements
            wheel_delta: 1,                                 // 1 wheel unit per movement
            wheel_max_speed: 2,                             // Wheel max speed multiplier
            wheel_ticks_to_max: 40,                         // 40 ticks to max wheel speed (~3.2s)
            move_max: 25,                                   // Maximum movement per report
            wheel_max: 4,                                   // Maximum wheel movement per report
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
