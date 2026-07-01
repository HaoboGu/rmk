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
//! - [`crate::morse::MorseProfile`] / [`crate::morse::MorseMode`] - Morse/tap-hold timing configuration

mod encoder;
mod key_action;
mod keyboard;
mod light;

pub use encoder::EncoderAction;
pub use key_action::KeyAction;
pub use keyboard::KeyboardAction;
pub use light::LightAction;
use postcard::experimental::max_size::MaxSize;
#[cfg(feature = "rmk_protocol")]
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::keycode::{KeyCode, SpecialKey};
use crate::modifier::ModifierCombination;
#[cfg(feature = "steno")]
use crate::steno::StenoKey;

/// Parameters for the StickyKey action.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
pub struct StickyKeyAction {
    /// Key sent on each SK press. `KeyCode::Hid(HidKeyCode::No)` selects the pure-mod (OSM) shape
    /// when `layer` is `None`; otherwise it's the tap-key (alt-tab) shape.
    pub key: KeyCode,
    /// Modifiers held between presses (0 = none). Unused for the layer (OSL) shape.
    pub keep: ModifierCombination,
    /// `Some(n)` = one-shot-layer (OSL) shape activating layer `n`.
    /// `None` + `key == KeyCode::Hid(HidKeyCode::No)` = pure-mod (OSM) shape.
    /// `None` + `key != KeyCode::Hid(HidKeyCode::No)` = tap-key (alt-tab) shape.
    pub layer: Option<u8>,
}

/// A single basic action that a keyboard can execute.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
#[non_exhaustive]
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
    /// Sticky key: a unified one-shot action whose shape (pure-mod, tap-key, or
    /// one-shot-layer) is determined by the [`StickyKeyAction`] payload.
    StickyKey(StickyKeyAction),
    /// Set the default layer and persist it to storage; restored on next boot.
    ///
    /// Runtime behavior matches [`Action::DefaultLayer`]; additionally persisted to flash.
    PersistentDefaultLayer(u8),
    /// A Plover HID stenography key. Press/release of this key updates the
    /// in-progress steno chord; on first release the accumulated chord is
    /// sent to the host as a vendor HID report.
    #[cfg(feature = "steno")]
    Steno(StenoKey),
}
