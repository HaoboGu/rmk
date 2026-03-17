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

mod encoder;
mod keyboard;
mod light;
mod morse;

pub use encoder::EncoderAction;
pub use keyboard::KeyboardAction;
pub use light::LightAction;
pub use morse::{MorseMode, MorseProfile};

use crate::keycode::{KeyCode, SpecialKey};
use crate::modifier::ModifierCombination;

/// A KeyAction is the action at a keyboard position, stored in keymap.
/// It can be a single action like triggering a key, or a composite keyboard action like tap/hold
#[derive(Debug, Copy, Clone, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(postcard::experimental::max_size::MaxSize)]
#[cfg_attr(feature = "protocol", derive(postcard_schema::Schema))]
pub enum KeyAction {
    /// No action
    No,
    /// Transparent action, next layer will be checked
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
#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(postcard::experimental::max_size::MaxSize)]
#[cfg_attr(feature = "protocol", derive(postcard_schema::Schema))]
pub enum Action {
    /// Default action, no action.
    No,
    /// A normal key stroke, uses for all keycodes defined in `KeyCode` enum, including mouse key, consumer/system control, etc.
    Key(KeyCode),
    /// Modifier Combination, used in tap hold
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
    TriLayerLower,
    TriLayerUpper,
    /// Triggers the Macro at the 'index'.
    TriggerMacro(u8),
    /// Oneshot layer, keep the layer active until the next key is triggered.
    OneShotLayer(u8),
    /// Oneshot modifier, keep the modifier active until the next key is triggered.
    OneShotModifier(ModifierCombination),
    /// Oneshot key, keep the key active until the next key is triggered.
    OneShotKey(KeyCode),
    /// Actions for controlling lights
    Light(LightAction),
    /// Actions for controlling the keyboard
    KeyboardControl(KeyboardAction),
    /// Special Keys
    Special(SpecialKey),
    /// User Keys
    User(u8),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_morse_profile_timeout_setters() {
        // Test with all fields set to verify bit field isolation
        let mut profile = MorseProfile::new(Some(true), Some(MorseMode::PermissiveHold), Some(1000), Some(2000));

        // Verify initial state
        assert_eq!(profile.hold_timeout_ms(), Some(1000));
        assert_eq!(profile.gap_timeout_ms(), Some(2000));
        assert_eq!(profile.unilateral_tap(), Some(true));
        assert_eq!(profile.mode(), Some(MorseMode::PermissiveHold));

        // Test set_hold_timeout_ms - should not affect other fields
        profile.set_hold_timeout_ms(1500);
        assert_eq!(profile.hold_timeout_ms(), Some(1500));
        assert_eq!(profile.gap_timeout_ms(), Some(2000));
        assert_eq!(profile.unilateral_tap(), Some(true));
        assert_eq!(profile.mode(), Some(MorseMode::PermissiveHold));

        // Test set_gap_timeout_ms - should not affect other fields (critical for unilateral_tap)
        profile.set_gap_timeout_ms(2500);
        assert_eq!(profile.hold_timeout_ms(), Some(1500));
        assert_eq!(profile.gap_timeout_ms(), Some(2500));
        assert_eq!(profile.unilateral_tap(), Some(true));
        assert_eq!(profile.mode(), Some(MorseMode::PermissiveHold));

        // Test maximum values (14 bits = 0x3FFF)
        profile.set_hold_timeout_ms(0x3FFF);
        profile.set_gap_timeout_ms(0x3FFF);
        assert_eq!(profile.hold_timeout_ms(), Some(0x3FFF));
        assert_eq!(profile.gap_timeout_ms(), Some(0x3FFF));
        assert_eq!(profile.unilateral_tap(), Some(true));
        assert_eq!(profile.mode(), Some(MorseMode::PermissiveHold));

        // Test zero values (should return None)
        profile.set_hold_timeout_ms(0);
        profile.set_gap_timeout_ms(0);
        assert_eq!(profile.hold_timeout_ms(), None);
        assert_eq!(profile.gap_timeout_ms(), None);
        assert_eq!(profile.unilateral_tap(), Some(true));
        assert_eq!(profile.mode(), Some(MorseMode::PermissiveHold));
    }
}
