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
    /// Keep current key pressed until the next key is triggered.
    OneShot(Action),
    /// General tap/hold action: (tap_action, hold_action)
    TapHold(Action, Action),
    /// Tap dance action, references a tap dance configuration by index.
    TapDance(u8),
}

/// A single basic action that a keyboard can execute.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Action {
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
}
