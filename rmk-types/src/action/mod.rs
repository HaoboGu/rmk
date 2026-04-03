//! Keyboard actions and behaviors.
//!
//! This module defines the core action system used in RMK firmware.
//! Actions represent what happens when a key is pressed, from simple key
//! presses to complex behaviors like tap-hold, layer switching, and macros.
//!
//! Key types:
//! - [`Action`] - Single basic operations (key press, layer switch, macro trigger, etc.)
//! - [`KeyAction`] - Composite behaviors stored in the keymap (tap-hold, morse, etc.)
//! - [`EncoderAction`] - Rotary encoder actions
//! - [`LightAction`] - Light control actions
//! - [`KeyboardAction`] - Keyboard control actions (reboot, toggle features, etc.)
//! - [`MorseProfile`] / [`MorseMode`] - Morse/tap-hold timing configuration

mod encoder;
mod key_action;
mod keyboard;
mod light;

pub use encoder::EncoderAction;
pub use key_action::KeyAction;
pub use keyboard::KeyboardAction;
pub use light::LightAction;
pub use crate::morse::{MorseMode, MorseProfile};
use postcard::experimental::max_size::MaxSize;
#[cfg(feature = "rmk_protocol")]
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::keycode::{KeyCode, SpecialKey};
use crate::modifier::ModifierCombination;

/// A single basic action that a keyboard can execute.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
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
