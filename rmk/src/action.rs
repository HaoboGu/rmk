use log::error;

use crate::keycode::{KeyCode, Modifier};

/// A KeyAction is the action of a keyboard position, stored in keymap.
/// It can be a single action like triggering a key, or a composite keyboard action like TapHold
/// 
/// Each `KeyAction` can be serialized to a u16, which can be stored in EEPROM.
///
/// 16bits = KeyAction type(4bits) + KeyAction Detail(4bits) + BasicAction(8bits)
///
/// OR
///
/// 16bits = KeyAction type(4bits) + KeyAction Detail(4bits) + BasicAction(8bits)
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum KeyAction {
    /// No action
    /// 
    /// Serialized as 0x0000
    No,
    /// Transparent action, next layer will be checked
    /// 
    /// Serialized as 0x0001
    Transparent,
    /// A single action, such as triggering a key, or activating a layer.
    /// Action is triggered when pressed and cancelled when released.
    /// 
    /// Serialized as 0000|Action(12bits).
    Single(Action),
    /// Don't wait the release of the key, auto-release after a time threshold.
    /// 
    /// Serialized as 0001|Action(12bits).
    Tap(Action),
    /// Keep current key pressed until the next key is triggered.
    /// 
    /// Serialized as 0010|Action(12bits).
    OneShot(Action),
    /// Action with a modifier triggered, only `Action::Key(BasicKeyCodes)`(aka 0x004 ~ 0x0FF) can be used with modifier.
    /// 
    /// Serialized as 0011|modifier(4bits)|BasicAction(8bits).
    WithModifier(Action, Modifier),
    /// Tap/hold will trigger different actions: TapHold(tap_action, hold_action).
    /// Only modifier and layer operation(0~15 layer) can be used as `hold_action`.
    /// `tap_action` is limited to `Action::Key(BasicKeyCodes)`(aka 0x004 ~ 0x0FF)
    /// 
    /// Serialized as 0100|modifier(4bits)|BasicKeyCodes(8bits)
    /// Serialized as 0101|layer(4bits)|BasicKeyCodes(8bits)
    // TODO1: Check compatibility with VIA
    // TODO2: Does it better for layer TapHold? -> 1|layer(3bits)|AllKeyCodes(12bits)
    // only layer 0-7 can be used in this case
    TapHold(Action, Action),
}

impl KeyAction {
    pub fn to_u16(&self) -> u16 {
        match self {
            KeyAction::No => 0x0000,
            KeyAction::Transparent => 0x0001,
            KeyAction::Single(a) => a.to_u16(),
            KeyAction::Tap(a) => 0x0001 | a.to_u16(),
            KeyAction::OneShot(a) => 0x0010 | a.to_u16(),
            KeyAction::WithModifier(a, m) => 0x0011 | ((*m as u16) << 8) | a.to_u16(),
            KeyAction::TapHold(tap_action, hold_action) => match (tap_action, hold_action) {
                (Action::Key(tap_key), Action::Key(hold_key)) => {
                    // TapHold(key, modifier)
                    if tap_key.is_basic() {
                        if let Some(m) = Modifier::from_keycode(*hold_key) {
                            return 0x0100 | ((m as u16) << 8) | (*tap_key as u16);
                        }
                    }
                    // Not supported case
                    error!("Unsupported TapHold action: {:?}", self);
                    0x0000
                }
                (Action::Key(tap_key), Action::LayerOn(layer)) => {
                    if tap_key.is_basic() && *layer < 16 {
                        0x0101 | ((*layer as u16) << 8) | (*tap_key as u16)
                    } else {
                        error!("Unsupported TapHold action: {:?}", self);
                        0x0000
                    }
                }
                _ => {
                    error!("Unsupported TapHold action: {:?}", self);
                    0x0000
                }
            },
        }
    }
}

/// A single basic action that a keyboard can execute.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Action {
    /// A normal key stroke, uses for all keycodes defined in `KeyCode` enum, including mouse key, consumer/system control, etc.
    Key(KeyCode),
    /// Activate a layer
    LayerOn(u8),
    /// Deactivate a layer
    LayerOff(u8),
    /// Toggle a layer
    LayerToggle(u8),
}

impl Action {
    pub fn to_u16(&self) -> u16 {
        match self {
            Action::Key(k) => *k as u16,
            Action::LayerOn(layer) => *layer as u16,
            Action::LayerOff(layer) => *layer as u16,
            Action::LayerToggle(layer) => *layer as u16,
        }
    }
}
