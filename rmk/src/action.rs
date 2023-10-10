use crate::keycode::{KeyCode, ModifierCombination};
use log::error;
use num_enum::FromPrimitive;
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

    /// Convert via keycode to KeyAction.
    pub fn from_via_keycode(via_keycode: u16) -> Self {
        match via_keycode {
            0x0000 => KeyAction::No,
            0x0001 => KeyAction::Transparent,
            0x0002..=0x00FF => KeyAction::Single(Action::Key(KeyCode::from_primitive(via_keycode))),
            0x0100..=0x1FFF => {
                // WithModifier
                let keycode = KeyCode::from_primitive(via_keycode & 0x00FF);
                let modifier = ModifierCombination::from_bits((via_keycode >> 8) as u8);
                KeyAction::WithModifier(Action::Key(keycode), modifier)
            }
            0x5100..=0x510F => {
                // Layer Activate
                let layer = via_keycode as u8 & 0x0F;
                KeyAction::Single(Action::LayerOn(layer))
            }
            0x5400..=0x54FF => {
                // OneShot Layer
                let layer = via_keycode as u8 & 0xF;
                KeyAction::OneShot(Action::LayerOn(layer))
            }
            0x5500..=0x55FF => {
                // OneShot Modifier
                let m = ModifierCombination::from_bits(via_keycode as u8);
                KeyAction::OneShot(Action::Modifier(m))
            }
            0x5700..=0x57FF => {
                // Tap Dance
                todo!()
            }
            0x5C00..=0x5CFF => {
                // QMK functions, such as reset, swap ctrl/caps, gui on, haptic, music, clicky, combo, RGB, etc
                todo!()
            }
            0x5D00..=0x5D0F => {
                // DM Rec/Stop/Play
                todo!()
            }
            0x5F12..=0x5F21 => {
                // Macro 1-16
                todo!()
            }
            0x5F80..=0x5F8F => {
                // User 1-16
                todo!()
            }
            0x6000..=0x7FFF => {
                // Modifier Tap/Hold
                // The via equivalent of Modifier Tap/Hold is called Mod-tap, whose keycode representation is same with RMK
                let keycode = KeyCode::from_primitive(via_keycode & 0x00FF);
                let modifier = ModifierCombination::from_bits(((via_keycode >> 8) & 0b11111) as u8);
                KeyAction::ModifierTapHold(Action::Key(keycode), modifier)
            }
            0x4000..=0x4FFF => {
                // Layer Tap/Hold
                // The via equivalent of Modifier Tap/Hold is called Mod-tap,
                let layer = (via_keycode >> 8) & 0xF;
                let keycode = KeyCode::from_primitive(via_keycode & 0x00FF);
                KeyAction::LayerTapHold(Action::Key(keycode), layer as u8)
            }

            _ => KeyAction::No,
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_convert_via_keycode_to_key_action() {
        // A
        let via_keycode = 0x04;
        assert_eq!(
            KeyAction::Single(Action::Key(KeyCode::A)),
            KeyAction::from_via_keycode(via_keycode)
        );

        // Right shift
        let via_keycode = 0xE5;
        assert_eq!(
            KeyAction::Single(Action::Key(KeyCode::RShift)),
            KeyAction::from_via_keycode(via_keycode)
        );

        // Mo(3)
        let via_keycode = 0x5103;
        assert_eq!(
            KeyAction::Single(Action::LayerOn(3)),
            KeyAction::from_via_keycode(via_keycode)
        );

        // OSL(3)
        let via_keycode = 0x5403;
        assert_eq!(
            KeyAction::OneShot(Action::LayerOn(3)),
            KeyAction::from_via_keycode(via_keycode)
        );

        // OSM RCtrl
        let via_keycode = 0x5511;
        assert_eq!(
            KeyAction::OneShot(Action::Modifier(ModifierCombination::new(true, false, false, false, true))),
            KeyAction::from_via_keycode(via_keycode)
        );

        // LCtrl(A) -> WithModifier(A)
        let via_keycode = 0x104;
        assert_eq!(
            KeyAction::WithModifier(
                Action::Key(KeyCode::A),
                ModifierCombination::new(false, false, false, false, true)
            ),
            KeyAction::from_via_keycode(via_keycode)
        );

        // RCtrl(A) -> WithModifier(A)
        let via_keycode = 0x1104;
        assert_eq!(
            KeyAction::WithModifier(
                Action::Key(KeyCode::A),
                ModifierCombination::new(true, false, false, false, true)
            ),
            KeyAction::from_via_keycode(via_keycode)
        );

        // Meh(A) -> WithModifier(A)
        let via_keycode = 0x704;
        assert_eq!(
            KeyAction::WithModifier(
                Action::Key(KeyCode::A),
                ModifierCombination::new(false, false, true, true, true)
            ),
            KeyAction::from_via_keycode(via_keycode)
        );

        // Hypr(A) -> WithModifier(A)
        let via_keycode = 0xF04;
        assert_eq!(
            KeyAction::WithModifier(
                Action::Key(KeyCode::A),
                ModifierCombination::new(false, true, true, true, true)
            ),
            KeyAction::from_via_keycode(via_keycode)
        );

        // LT0(A) -> LayerTapHold(A, 0)
        let via_keycode = 0x4004;
        assert_eq!(
            KeyAction::LayerTapHold(Action::Key(KeyCode::A), 0),
            KeyAction::from_via_keycode(via_keycode)
        );

        // LT3(A) -> LayerTapHold(A, 3)
        let via_keycode = 0x4304;
        assert_eq!(
            KeyAction::LayerTapHold(Action::Key(KeyCode::A), 3),
            KeyAction::from_via_keycode(via_keycode)
        );

        // LSA_T(A) ->
        let via_keycode = 0x6604;
        assert_eq!(
            KeyAction::ModifierTapHold(
                Action::Key(KeyCode::A),
                ModifierCombination::new(false, false, true, true, false)
            ),
            KeyAction::from_via_keycode(via_keycode)
        );

        // RCAG_T(A) ->
        let via_keycode = 0x7D04;
        assert_eq!(
            KeyAction::ModifierTapHold(
                Action::Key(KeyCode::A),
                ModifierCombination::new(true, true, true, false, true)
            ),
            KeyAction::from_via_keycode(via_keycode)
        );

        // ALL_T(A) ->
        let via_keycode: u16 = 0x6F04;
        assert_eq!(
            KeyAction::ModifierTapHold(
                Action::Key(KeyCode::A),
                ModifierCombination::new(false, true, true, true, true)
            ),
            KeyAction::from_via_keycode(via_keycode)
        );

        // Meh_T(A) ->
        let via_keycode = 0x6704;
        assert_eq!(
            KeyAction::ModifierTapHold(
                Action::Key(KeyCode::A),
                ModifierCombination::new(false, false, true, true, true)
            ),
            KeyAction::from_via_keycode(via_keycode)
        );
    }
}
