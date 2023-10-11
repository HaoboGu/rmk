use crate::keycode::{KeyCode, ModifierCombination};
use log::error;
use packed_struct::prelude::*;

/// A KeyAction is the action of a keyboard position, stored in keymap.
/// It can be a single action like triggering a key, or a composite keyboard action like TapHold
///
/// Each `KeyAction` can be serialized to a u16, which can be stored in EEPROM.
///
/// 16bits = KeyAction type(3bits) + Layer/Modifier Detail(5bits) + BasicAction(8bits)
///
/// OR
///
/// 16bits = KeyAction type(3bits) + Action(12bits)
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
    /// Serialized as 010|modifier(5bits)|BasicAction(8bits).
    WithModifier(Action, ModifierCombination),
    /// Layer Tap/hold will trigger different actions: TapHold(tap_action, hold_action).
    /// Only modifier and layer operation(0~15 layer) can be used as `hold_action`.
    /// `tap_action` is limited to `Action::Key(BasicKeyCodes)`(aka 0x004 ~ 0x0FF)
    ///
    /// Serialized as 1|layer(3bits)|Action(12bits)
    LayerTapHold(Action, u8),
    /// Modifier Tap/hold will trigger different actions: TapHold(tap_action, modifier).
    ///
    /// Serialized as 011|modifier(5bits)|BasicKeyCodes(8bits)
    ModifierTapHold(Action, ModifierCombination),
    /// General TapHold action. It cannot be serialized to u16, will be ignored temporarily.
    /// TODO: Figura out a better way to represent & save a general tap/hold action
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
            KeyAction::WithModifier(a, m) => {
                let mut modifier_bits = [0];
                // Ignore packing error
                ModifierCombination::pack_to_slice(m, &mut modifier_bits).unwrap_or_default();
                0x4000 | ((modifier_bits[0] as u16) << 8) | a.to_u16()
            }
            KeyAction::ModifierTapHold(action, modifier) => match action {
                Action::Key(k) => {
                    if k.is_basic() {
                        let mut modifier_bits = [0];
                        // Ignore packing error
                        ModifierCombination::pack_to_slice(modifier, &mut modifier_bits)
                            .unwrap_or_default();
                        0x6000 | ((modifier_bits[0] as u16) << 8) | *k as u16
                    } else {
                        0x000
                    }
                }
                _ => {
                    error!("ModifierTapHold supports basic keycodes");
                    0x0000
                }
            },
            KeyAction::LayerTapHold(action, layer) => {
                if *layer < 8 {
                    0x8000 | ((*layer as u16) << 15) | action.to_u16()
                } else {
                    error!("LayerTapHold supports layers 0~7, got {}", layer);
                    0x0000
                }
            }
            KeyAction::TapHold(_, _) => {
                error!("Unsupported TapHold action: {:?}", self);
                0x0000
            }
        }
    }

    
}

/// A single basic action that a keyboard can execute.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Action {
    /// A normal key stroke, uses for all keycodes defined in `KeyCode` enum, including mouse key, consumer/system control, etc.
    Key(KeyCode),
    /// Modifier Combination, used for oneshot keyaction
    Modifier(ModifierCombination),
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
            Action::Modifier(m) => m.to_bits() as u16,
        }
    }
}
