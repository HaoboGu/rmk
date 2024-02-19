use crate::keycode::{KeyCode, ModifierCombination};
use defmt::{error, warn, Format};
use num_enum::FromPrimitive;
use packed_struct::PackedStructSlice;

/// A KeyAction is the action at a keyboard position, stored in keymap.
/// It can be a single action like triggering a key, or a composite keyboard action like tap/hold
///
/// Each `KeyAction` can be serialized to a u16 action code. There are 2 patterns of action code's bit-field composition of `KeyAction`:
///
/// - KeyActionType(8bits) + BasicAction(8bits)
///
/// - KeyActionType(4bits) + Action(12bits)
///
/// The `BasicAction` represents only a single key action of keycodes defined in HID spec. The `Action` represents all actions defined in the following `Action` enum, including modifier combination and layer switch.
///
/// The KeyActionType bits varies between different types of a KeyAction, see docs of each enum variant.
#[derive(Format, Debug, Copy, Clone, PartialEq, Eq)]
pub enum KeyAction {
    /// No action. Serialized as 0x0000.
    No,
    /// Transparent action, next layer will be checked. Serialized as 0x0001.
    Transparent,
    /// A single action, such as triggering a key, or activating a layer. Action is triggered when pressed and cancelled when released.
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
    /// Layer tap/hold will trigger different actions: tap for basic action, hold for layer activation.
    ///
    /// Serialized as 0011|layer(4bits)|BasicAction(8bits).
    LayerTapHold(Action, u8),
    /// Action with the modifier combination triggered.
    ///
    /// Serialized as 010|modifier(5bits)|BasicAction(8bits).
    WithModifier(Action, ModifierCombination),
    /// Modifier tap/hold will trigger different actions: tap for basic action, hold for modifier activation.
    ///
    /// Serialized as 011|modifier(5bits)|BasicAction(8bits).
    ModifierTapHold(Action, ModifierCombination),
    /// General tap/hold action. Because current BaseAction actually uses at most 7 bits, so we borrow 1 bit as the identifier of general tap/hold action.
    ///
    /// Serialized as 1|BasicAction(7bits)|BasicAction(8bits).
    TapHold(Action, Action),
}

impl KeyAction {
    /// Convert a `KeyAction` to corresponding key action code.
    pub(crate) fn to_key_action_code(&self) -> u16 {
        match self {
            KeyAction::No => 0x0000,
            KeyAction::Transparent => 0x0001,
            KeyAction::Single(a) => a.to_action_code(),
            KeyAction::Tap(a) => 0x0001 | a.to_action_code(),
            KeyAction::OneShot(a) => 0x0010 | a.to_action_code(),
            KeyAction::WithModifier(a, m) => {
                let mut modifier_bits = [0];
                // Ignore packing error
                ModifierCombination::pack_to_slice(m, &mut modifier_bits).unwrap_or_default();
                0x4000 | ((modifier_bits[0] as u16) << 8) | a.to_basic_action_code()
            }
            KeyAction::ModifierTapHold(action, modifier) => {
                let mut modifier_bits = [0];
                // Ignore packing error
                ModifierCombination::pack_to_slice(modifier, &mut modifier_bits)
                    .unwrap_or_default();
                0x6000 | ((modifier_bits[0] as u16) << 8) | action.to_basic_action_code()
            }
            KeyAction::LayerTapHold(action, layer) => {
                if *layer < 16 {
                    0x3000 | ((*layer as u16) << 15) | action.to_basic_action_code()
                } else {
                    error!("LayerTapHold supports only layer 0~15, got {}", layer);
                    0x0000
                }
            }
            KeyAction::TapHold(tap, hold) => {
                0x8000 | (hold.to_basic_action_code() << 15) | tap.to_basic_action_code()
            }
        }
    }

    pub(crate) fn from_key_action_code(code: u16) -> KeyAction {
        match code {
            0x0..=0xFFF => KeyAction::Single(Action::from_action_code(code)),
            0x1000..=0x1FFF => KeyAction::Tap(Action::from_action_code(code & 0xFFF)),
            0x2000..=0x2FFF => KeyAction::OneShot(Action::from_action_code(code & 0xFFF)),
            0x3000..=0x3FFF => {
                let layer = (code >> 8) & 0xF;
                KeyAction::LayerTapHold(Action::from_action_code(code & 0xFF), layer as u8)
            }
            0x4000..=0x5FFF => {
                let modifier_bits = (code >> 8) & 0x1F;
                KeyAction::WithModifier(
                    Action::from_action_code(code & 0xFF),
                    ModifierCombination::from_bits(modifier_bits as u8),
                )
            }
            0x6000..=0x7FFF => {
                let modifier_bits = (code >> 8) & 0x1F;
                KeyAction::ModifierTapHold(
                    Action::from_action_code(code & 0xFF),
                    ModifierCombination::from_bits(modifier_bits as u8),
                )
            }
            0x8000..=0xFFFF => KeyAction::TapHold(
                Action::from_action_code(code & 0xFF),
                Action::from_action_code((code >> 8) & 0x7F),
            ),
        }
    }
}

/// A single basic action that a keyboard can execute.
/// An Action can be represented in 12 bits, aka 0x000 ~ 0xFFF
#[derive(Debug, Format, Copy, Clone, PartialEq, Eq)]
pub enum Action {
    /// A normal key stroke, uses for all keycodes defined in `KeyCode` enum, including mouse key, consumer/system control, etc.
    ///
    /// Uses 0x000 ~ 0xCFF
    Key(KeyCode),
    /// Modifier Combination, used for oneshot keyaction.
    ///
    /// Uses 0xE00 ~ 0xE1F. Serialized as 1110|000|modifier(5bits)
    Modifier(ModifierCombination),
    /// Activate a layer
    ///
    /// Uses 0xE20 ~ 0xE3F. Serialized as 1110|001|layer_num(5bits)
    LayerOn(u8),
    /// Deactivate a layer
    ///
    /// Uses 0xE40 ~ 0xE5F. Serialized as 1110|010|layer_num(5bits)
    LayerOff(u8),
    /// Toggle a layer
    ///
    /// Uses 0xE60 ~ 0xE7F. Serialized as 1110|011|layer_num(5bits)
    LayerToggle(u8),
}

impl Action {
    /// Convert an `Action` to 12-bit action code
    pub(crate) fn to_action_code(&self) -> u16 {
        match self {
            Action::Key(k) => *k as u16,
            Action::Modifier(m) => 0xE00 | (m.to_bits() as u16),
            Action::LayerOn(layer) => 0xE20 | (*layer as u16),
            Action::LayerOff(layer) => 0xE40 | (*layer as u16),
            Action::LayerToggle(layer) => 0xE60 | (*layer as u16),
        }
    }

    /// Create an `Action` from action_code, returns Key(KeyCode::No) if the action code is not valid.
    pub(crate) fn from_action_code(action_code: u16) -> Action {
        match action_code {
            0x000..=0xCFF => Action::Key(KeyCode::from_primitive(action_code)),
            0xE00..=0xE1F => {
                let modifier_bits = (action_code & 0xFF) as u8;
                Action::Modifier(ModifierCombination::from_bits(modifier_bits))
            }
            0xE20..=0xE3F => {
                let layer = (action_code & 0xFF) as u8;
                Action::LayerOn(layer)
            }
            0xE40..=0xE5F => {
                let layer = (action_code & 0xFF) as u8;
                Action::LayerOff(layer)
            }
            0xE60..=0xE7F => {
                let layer = (action_code & 0xFF) as u8;
                Action::LayerToggle(layer)
            }
            _ => {
                warn!("Not a valid 12-bit action code: {:#X}", action_code);
                Action::Key(KeyCode::No)
            }
        }
    }

    /// Convert an `Action` to 8-bit basic action code, only applicable for `Key(BasicKeyCode)`
    pub(crate) fn to_basic_action_code(&self) -> u16 {
        match self {
            Action::Key(kc) => {
                if kc.is_basic() {
                    *kc as u16
                } else {
                    0
                }
            }
            _ => 0,
        }
    }
}
