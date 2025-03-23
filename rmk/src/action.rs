use crate::keycode::{KeyCode, ModifierCombination};

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
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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
    pub(crate) fn to_key_action_code(self) -> u16 {
        match self {
            KeyAction::No => 0x0000,
            KeyAction::Transparent => 0x0001,
            KeyAction::Single(a) => a.to_action_code(),
            KeyAction::Tap(a) => 0x0001 | a.to_action_code(),
            KeyAction::OneShot(a) => 0x0010 | a.to_action_code(),
            KeyAction::WithModifier(a, m) => {
                0x4000 | ((m.into_bits() as u16) << 8) | a.to_basic_action_code()
            }
            KeyAction::ModifierTapHold(a, m) => {
                0x6000 | ((m.into_bits() as u16) << 8) | a.to_basic_action_code()
            }
            KeyAction::LayerTapHold(action, layer) => {
                if layer < 16 {
                    0x3000 | ((layer as u16) << 15) | action.to_basic_action_code()
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
}

/// A single basic action that a keyboard can execute.
/// An Action can be represented in 12 bits, aka 0x000 ~ 0xFFF
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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
    /// Set default layer
    ///
    /// Uses 0xE80 ~ 0xE9F. Serialized as 1110|100|layer_num(5bits)
    DefaultLayer(u8),
    /// Activate a layer and deactivate all other layers(except default layer)
    ///
    /// Uses 0xEA0 ~ 0xEBF. Serialized as 1110|101|layer_num(5bits)
    LayerToggleOnly(u8),
}

impl Action {
    /// Convert an `Action` to 12-bit action code
    pub(crate) fn to_action_code(self) -> u16 {
        match self {
            Action::Key(k) => k as u16,
            Action::Modifier(m) => 0xE00 | (m.into_bits() as u16),
            Action::LayerOn(layer) => 0xE20 | (layer as u16),
            Action::LayerOff(layer) => 0xE40 | (layer as u16),
            Action::LayerToggle(layer) => 0xE60 | (layer as u16),
            Action::DefaultLayer(layer) => 0xE80 | (layer as u16),
            Action::LayerToggleOnly(layer) => 0xEA0 | (layer as u16),
        }
    }

    /// Convert an `Action` to 8-bit basic action code, only applicable for `Key(BasicKeyCode)`
    pub(crate) fn to_basic_action_code(self) -> u16 {
        match self {
            Action::Key(kc) => {
                if kc.is_basic() {
                    kc as u16
                } else {
                    0
                }
            }
            _ => 0,
        }
    }
}
