//! Keyboard actions and behaviors.
//!
//! This module defines the core action system used in RMK firmware.
//! Actions represent what happens when a key is pressed, from simple key
//! presses to complex behaviors like tap-hold, layer switching, and macros.
//!
//! Key types:
//! - [`Action`] - Single operations that keyboards send or execute
//! - [`KeyAction`] - Complex behaviors that keyboards should behave
//! - [`EncoderAction`] - Rotary encoder actions

use crate::keycode::KeyCode;
use crate::modifier::ModifierCombination;

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
    /// Create a new encoder action.
    pub const fn new(clockwise: KeyAction, counter_clockwise: KeyAction) -> Self {
        Self {
            clockwise,
            counter_clockwise,
        }
    }

    /// Set the clockwise action.
    pub fn set_clockwise(&mut self, clockwise: KeyAction) {
        self.clockwise = clockwise;
    }

    /// Set the counter clockwise action.
    pub fn set_counter_clockwise(&mut self, counter_clockwise: KeyAction) {
        self.counter_clockwise = counter_clockwise;
    }

    /// Get the clockwise action.
    pub fn clockwise(&self) -> KeyAction {
        self.clockwise
    }

    /// Get the counter clockwise action.
    pub fn counter_clockwise(&self) -> KeyAction {
        self.counter_clockwise
    }
}

/// Mode for morse key behavior
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum MorseMode {
    /// Same as QMK's permissive hold: https://docs.qmk.fm/tap_hold#tap-or-hold-decision-modes
    /// When another key is pressed and released during the current morse key is held,
    /// the hold action of current morse key will be triggered
    PermissiveHold,
    /// Trigger hold immediately if any other non-morse key is pressed when the current morse key is held
    HoldOnOtherPress,
    /// Normal mode, the decision is made when timeout
    Normal,
}

/// Configuration for morse, tap dance and tap-hold
/// to save some RAM space, manually packed into 32 bits
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MorseProfile(u32);

impl MorseProfile {
    pub const fn const_default() -> Self {
        Self(0)
    }

    /// If the previous key is on the same "hand", the current key will be determined as a tap
    pub fn unilateral_tap(self) -> Option<bool> {
        match self.0 & 0x0000_C000 {
            0x0000_C000 => Some(true),
            0x0000_8000 => Some(false),
            _ => None,
        }
    }
    pub const fn with_unilateral_tap(self, b: Option<bool>) -> Self {
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
    pub const fn with_mode(self, m: Option<MorseMode>) -> Self {
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
    /// if given, should not be zero
    pub fn hold_timeout_ms(self) -> Option<u16> {
        // NonZero
        let t = (self.0 & 0x3FFF) as u16;
        if t == 0 { None } else { Some(t) }
    }
    pub const fn with_hold_timeout_ms(self, t: Option<u16>) -> Self {
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
    pub const fn with_gap_timeout_ms(self, t: Option<u16>) -> Self {
        if let Some(t) = t {
            Self((self.0 & 0xC000_FFFF) | ((t as u32 & 0x3FFF) << 16))
        } else {
            Self(self.0 & 0xC000_FFFF)
        }
    }

    pub const fn new(
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

impl Default for MorseProfile {
    fn default() -> Self {
        MorseProfile::const_default()
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

/// A KeyAction is the action at a keyboard position, stored in keymap.
/// It can be a single action like triggering a key, or a composite keyboard action like tap/hold
#[derive(Debug, Copy, Clone, Eq)]
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
    TapHold(Action, Action, MorseProfile),

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

    /// 'morse' is an alias for the superset of tap dance and tap hold keys,
    /// since their handling have many similarities
    pub fn is_morse(&self) -> bool {
        matches!(self, KeyAction::TapHold(_, _, _) | KeyAction::Morse(_))
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, KeyAction::No)
    }
}

/// combo, fork, etc. compares key actions
/// WARNING: this is not a perfect comparison, we ignores the profile config of TapHold!
impl PartialEq for KeyAction {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (KeyAction::No, KeyAction::No) => true,
            (KeyAction::Transparent, KeyAction::Transparent) => true,
            (KeyAction::Single(a), KeyAction::Single(b)) => a == b,
            (KeyAction::Tap(a), KeyAction::Tap(b)) => a == b,
            (KeyAction::TapHold(a, b, _), KeyAction::TapHold(c, d, _)) => a == c && b == d,
            (KeyAction::Morse(a), KeyAction::Morse(b)) => a == b,
            _ => false,
        }
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
