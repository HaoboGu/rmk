use embassy_time::Duration;

use crate::TAP_DANCE_MAX_TAP;
use crate::config::BehaviorConfig;
use crate::keycode::{KeyCode, ModifierCombination};
use crate::morse::{HOLD, MorseKeyMode, MorsePattern, TAP};

/// EncoderAction is the action at a encoder position, stored in encoder_map.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct EncoderAction {
    clockwise: KeyAction,
    counter_clockwise: KeyAction,
}

impl Default for EncoderAction {
    fn default() -> Self {
        Self {
            clockwise: KeyAction::No,
            counter_clockwise: KeyAction::No,
        }
    }
}
impl EncoderAction {
    pub const fn new(clockwise: KeyAction, counter_clockwise: KeyAction) -> Self {
        Self {
            clockwise,
            counter_clockwise,
        }
    }

    pub fn set_clockwise(&mut self, clockwise: KeyAction) {
        self.clockwise = clockwise;
    }

    pub fn set_counter_clockwise(&mut self, counter_clockwise: KeyAction) {
        self.counter_clockwise = counter_clockwise;
    }

    pub fn clockwise(&self) -> KeyAction {
        self.clockwise
    }

    pub fn counter_clockwise(&self) -> KeyAction {
        self.counter_clockwise
    }
}

/// A KeyAction is the action at a keyboard position, stored in keymap.
/// It can be a single action like triggering a key, or a composite keyboard action like tap/hold
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum KeyAction {
    /// No action. Serialized as 0x0000.
    No,
    /// Transparent action, next layer will be checked. Serialized as 0x0001.
    Transparent,
    /// A single action, such as triggering a key, or activating a layer. Action is triggered when pressed and cancelled when released.
    Single(Action),
    /// Don't wait the release of the key, auto-release after a time threshold.
    Tap(Action),

    /// Tap hold action
    TapHold(Action, Action),

    /// Tap dance action, references a tap dance configuration by index.
    TapDance(u8),

    /// Morse action, references a morse configuration by index.
    Morse(u8),
}

impl KeyAction {
    /// Convert `KeyAction` to the internal `Action`.
    /// Only valid for `Single` and `Tap` variant, returns `Action::No` for other variants.
    pub fn to_action(self) -> Action {
        match self {
            KeyAction::Single(a) | KeyAction::Tap(a) => a,
            _ => Action::No,
        }
    }

    pub fn is_morse(&self) -> bool {
        matches!(
            self,
            KeyAction::TapHold(_, _) | KeyAction::TapDance(_) | KeyAction::Morse(_)
        )
    }

    pub fn action_from_pattern(&self, behavior_config: &BehaviorConfig, pattern: MorsePattern) -> Action {
        match self {
            KeyAction::TapHold(tap_action, hold_action) => match pattern {
                TAP => *tap_action,
                HOLD => *hold_action,
                _ => Action::No,
            },
            KeyAction::TapDance(idx) => {
                behavior_config
                    .tap_dance
                    .tap_dances
                    .get(*idx as usize)
                    .map_or(Action::No, |td| {
                        let l = pattern.pattern_length();
                        if l > 0 && l <= TAP_DANCE_MAX_TAP {
                            if pattern.last_is_hold() {
                                td.actions[l - 1].1
                            } else {
                                td.actions[l - 1].0
                            }
                        } else {
                            Action::No
                        }
                    })
            }

            KeyAction::Morse(idx) => behavior_config
                .morse
                .action_sets
                .get(*idx as usize)
                .map_or(Action::No, |morse| *morse.get(pattern).unwrap_or(&Action::No)),
            _ => Action::No,
        }
    }

    pub fn morse_timeout(&self, behavior_config: &BehaviorConfig) -> Duration {
        match self {
            KeyAction::TapDance(idx) => behavior_config
                .tap_dance
                .tap_dances
                .get(*idx as usize)
                .map(|td| Duration::from_millis(td.timeout_ms as u64)),

            KeyAction::Morse(idx) => behavior_config
                .morse
                .action_sets
                .get(*idx as usize)
                .map(|morse| Duration::from_millis(morse.timeout_ms as u64)),

            _ => None,
        }
        .unwrap_or_else(|| behavior_config.morse.operation_timeout)
    }

    pub fn morse_mode(&self, behavior_config: &BehaviorConfig) -> (MorseKeyMode, bool) {
        match self {
            KeyAction::TapDance(idx) => behavior_config
                .tap_dance
                .tap_dances
                .get(*idx as usize)
                .map(|td| (td.mode, td.unilateral_tap)),

            KeyAction::Morse(idx) => behavior_config
                .morse
                .action_sets
                .get(*idx as usize)
                .map(|morse| (morse.mode, morse.unilateral_tap)),

            _ => None,
        }
        .unwrap_or_else(|| {
            if behavior_config.morse.enable_hrm //TODO instead of this let the HRM keycodes configurable!
               && let Action::Key(tap_key_code) = self.action_from_pattern(behavior_config, TAP)
               && tap_key_code.is_home_row()
            //&& (!let Action::Key(_) = hold_action) //the hold action in home row is not key, but modifier or layer activation
            {
                (MorseKeyMode::PermissiveHold, true)
            } else {
                (behavior_config.morse.mode, behavior_config.morse.unilateral_tap)
            }
        })
    }

    pub fn max_pattern_length(&self, behavior_config: &BehaviorConfig) -> usize {
        match self {
            KeyAction::TapHold(_, _) => 1,
            KeyAction::TapDance(idx) => behavior_config
                .tap_dance
                .tap_dances
                .get(*idx as usize)
                .map_or(0, |td| td.max_pattern_length()),
            KeyAction::Morse(idx) => behavior_config
                .morse
                .action_sets
                .get(*idx as usize)
                .map_or(0, |morse| morse.max_pattern_length()),
            _ => 0,
        }
    }

    pub fn is_pattern_completed(&self, behavior_config: &BehaviorConfig, pattern: MorsePattern) -> bool {
        pattern.is_full()
        || (pattern.last_is_hold() && matches!(self, KeyAction::TapDance(_) | KeyAction::TapHold(_, _))) //allow tap hold, tap dance to finish with the first long press (this is not true for real morse!)
        || pattern.pattern_length() >= self.max_pattern_length(behavior_config)
    }
}

/// A single basic action that a keyboard can execute.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Action {
    /// Default action, no action.
    No,
    /// Transparent action, next layer will be checked.
    Transparent,
    /// A normal key stroke, uses for all keycodes defined in `KeyCode` enum, including mouse key, consumer/system control, etc.
    Key(KeyCode),
    /// Modifier Combination, used for oneshot keyaction.
    Modifier(ModifierCombination),
    /// Key stroke with modifier combination triggered.
    KeyWithModifier(KeyCode, ModifierCombination),
    /// Activate a layer
    LayerOn(u8),
    /// Activate a layer with modifier combination triggered.
    LayerOnWithModifier(u8, ModifierCombination),
    /// Deactivate a layer
    LayerOff(u8),
    /// Toggle a layer
    LayerToggle(u8),
    /// Set default layer
    DefaultLayer(u8),
    /// Activate a layer and deactivate all other layers(except default layer)
    LayerToggleOnly(u8),
    /// Triggers the Macro at the 'index'.
    /// this is an alternative trigger to
    /// Macro keycodes (0x500 ~ 0x5FF; KeyCode::Macro0 ~ KeyCode::Macro31
    /// e.g. `Action::TriggerMacro(6)`` will trigger the same Macro as `Action::Key(KeyCode::Macro6)`
    /// the main purpose for this enum variant is to easily extend to more than 32 macros (to 256)
    /// without introducing new Keycodes.
    TriggerMacro(u8),
    /// Oneshot layer, keep the layer active until the next key is triggered.
    OneShotLayer(u8),
    /// Oneshot modifier, keep the modifier active until the next key is triggered.
    OneShotModifier(ModifierCombination),
    /// Oneshot key, keep the key active until the next key is triggered.
    OneShotKey(KeyCode),
}
