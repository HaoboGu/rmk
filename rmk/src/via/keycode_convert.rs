use defmt::warn;
use num_enum::FromPrimitive;

use crate::{
    action::{Action, KeyAction},
    keycode::{KeyCode, ModifierCombination},
};

pub(crate) fn to_via_keycode(key_action: KeyAction) -> u16 {
    match key_action {
        KeyAction::No => 0x0000,
        KeyAction::Transparent => 0x0001,
        KeyAction::Single(a) => match a {
            Action::Key(k) => k as u16,
            Action::LayerOn(l) => 0x5100 | l as u16,
            _ => 0x0000,
        },
        KeyAction::Tap(_) => {
            warn!("Tap action is not supported by via");
            0
        }
        KeyAction::OneShot(a) => match a {
            Action::Modifier(m) => {
                let modifier_bits = m.to_bits();
                0x5500 | modifier_bits as u16
            }
            Action::LayerOn(l) => {
                if l < 16 {
                    0x5400 | l as u16
                } else {
                    0x0000
                }
            }
            _ => 0x0000,
        },
        KeyAction::WithModifier(a, m) => {
            let keycode = match a {
                Action::Key(k) => k as u16,
                _ => 0,
            };
            ((m.to_bits() as u16) << 8) | keycode
        }
        KeyAction::LayerTapHold(a, l) => {
            if l > 16 {
                0
            } else {
                let keycode = match a {
                    Action::Key(k) => k as u16,
                    _ => 0,
                };
                0x4000 | ((l as u16) << 8) | keycode
            }
        }
        KeyAction::ModifierTapHold(a, m) => {
            let keycode = match a {
                Action::Key(k) => k as u16,
                _ => 0,
            };

            0x6000 | ((m.to_bits() as u16) << 8) | keycode
        }
        KeyAction::TapHold(_tap, _hold) => todo!(),
    }
}

/// Convert via keycode to KeyAction.
pub(crate) fn from_via_keycode(via_keycode: u16) -> KeyAction {
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
            // TODO: Tap Dance
            warn!("Tap dance {:#X} not supported", via_keycode);
            KeyAction::No
        }
        0x5C00..=0x5CFF => {
            // TODO: QMK functions, such as reset, swap ctrl/caps, gui on, haptic, music, clicky, combo, RGB, etc
            warn!("QMK functions {:#X} not supported", via_keycode);
            KeyAction::No
        }
        0x5D00..=0x5D0F => {
            // TODO: DM Rec/Stop/Play
            warn!("DM Rec/Stop/Play {:#X} not supported", via_keycode);
            KeyAction::No
        }
        0x5F12..=0x5F21 => {
            // TODO: Macro 1-16
            warn!("Macro {:#X} not supported", via_keycode);
            KeyAction::No
        }
        0x5F80..=0x5F8F => {
            // TODO: User 1-16
            warn!("User {:#X} not supported", via_keycode);
            KeyAction::No
        }
        0x6000..=0x7FFF => {
            // Modifier tap/hold
            // The via equivalent of Modifier tap/hold is called Mod-tap, whose keycode representation is same with RMK
            let keycode = KeyCode::from_primitive(via_keycode & 0x00FF);
            let modifier = ModifierCombination::from_bits(((via_keycode >> 8) & 0b11111) as u8);
            KeyAction::ModifierTapHold(Action::Key(keycode), modifier)
        }
        0x4000..=0x4FFF => {
            // Layer tap/hold
            // The via equivalent of Modifier tap/hold is called Mod-tap,
            let layer = (via_keycode >> 8) & 0xF;
            let keycode = KeyCode::from_primitive(via_keycode & 0x00FF);
            KeyAction::LayerTapHold(Action::Key(keycode), layer as u8)
        }

        _ => {
            warn!("Via keycode {:#X} is not processed", via_keycode);
            KeyAction::No
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
            from_via_keycode(via_keycode)
        );

        // Right shift
        let via_keycode = 0xE5;
        assert_eq!(
            KeyAction::Single(Action::Key(KeyCode::RShift)),
            from_via_keycode(via_keycode)
        );

        // Mo(3)
        let via_keycode = 0x5103;
        assert_eq!(
            KeyAction::Single(Action::LayerOn(3)),
            from_via_keycode(via_keycode)
        );

        // OSL(3)
        let via_keycode = 0x5403;
        assert_eq!(
            KeyAction::OneShot(Action::LayerOn(3)),
            from_via_keycode(via_keycode)
        );

        // OSM RCtrl
        let via_keycode = 0x5511;
        assert_eq!(
            KeyAction::OneShot(Action::Modifier(ModifierCombination::new(
                true, false, false, false, true
            ))),
            from_via_keycode(via_keycode)
        );

        // LCtrl(A) -> WithModifier(A)
        let via_keycode = 0x104;
        assert_eq!(
            KeyAction::WithModifier(
                Action::Key(KeyCode::A),
                ModifierCombination::new(false, false, false, false, true)
            ),
            from_via_keycode(via_keycode)
        );

        // RCtrl(A) -> WithModifier(A)
        let via_keycode = 0x1104;
        assert_eq!(
            KeyAction::WithModifier(
                Action::Key(KeyCode::A),
                ModifierCombination::new(true, false, false, false, true)
            ),
            from_via_keycode(via_keycode)
        );

        // Meh(A) -> WithModifier(A)
        let via_keycode = 0x704;
        assert_eq!(
            KeyAction::WithModifier(
                Action::Key(KeyCode::A),
                ModifierCombination::new(false, false, true, true, true)
            ),
            from_via_keycode(via_keycode)
        );

        // Hypr(A) -> WithModifier(A)
        let via_keycode = 0xF04;
        assert_eq!(
            KeyAction::WithModifier(
                Action::Key(KeyCode::A),
                ModifierCombination::new(false, true, true, true, true)
            ),
            from_via_keycode(via_keycode)
        );

        // LT0(A) -> LayerTapHold(A, 0)
        let via_keycode = 0x4004;
        assert_eq!(
            KeyAction::LayerTapHold(Action::Key(KeyCode::A), 0),
            from_via_keycode(via_keycode)
        );

        // LT3(A) -> LayerTapHold(A, 3)
        let via_keycode = 0x4304;
        assert_eq!(
            KeyAction::LayerTapHold(Action::Key(KeyCode::A), 3),
            from_via_keycode(via_keycode)
        );

        // LSA_T(A) ->
        let via_keycode = 0x6604;
        assert_eq!(
            KeyAction::ModifierTapHold(
                Action::Key(KeyCode::A),
                ModifierCombination::new(false, false, true, true, false)
            ),
            from_via_keycode(via_keycode)
        );

        // RCAG_T(A) ->
        let via_keycode = 0x7D04;
        assert_eq!(
            KeyAction::ModifierTapHold(
                Action::Key(KeyCode::A),
                ModifierCombination::new(true, true, true, false, true)
            ),
            from_via_keycode(via_keycode)
        );

        // ALL_T(A) ->
        let via_keycode: u16 = 0x6F04;
        assert_eq!(
            KeyAction::ModifierTapHold(
                Action::Key(KeyCode::A),
                ModifierCombination::new(false, true, true, true, true)
            ),
            from_via_keycode(via_keycode)
        );

        // Meh_T(A) ->
        let via_keycode = 0x6704;
        assert_eq!(
            KeyAction::ModifierTapHold(
                Action::Key(KeyCode::A),
                ModifierCombination::new(false, false, true, true, true)
            ),
            from_via_keycode(via_keycode)
        );
    }

    #[test]
    fn test_convert_key_action_to_via_keycode() {
        // A
        let a = KeyAction::Single(Action::Key(KeyCode::A));
        assert_eq!(0x04, to_via_keycode(a));

        // Right shift
        let a = KeyAction::Single(Action::Key(KeyCode::RShift));
        assert_eq!(0xE5, to_via_keycode(a));

        // Mo(3)
        let a = KeyAction::Single(Action::LayerOn(3));
        assert_eq!(0x5103, to_via_keycode(a));

        // OSL(3)
        let a = KeyAction::OneShot(Action::LayerOn(3));
        assert_eq!(0x5403, to_via_keycode(a));

        // OSM RCtrl
        let a = KeyAction::OneShot(Action::Modifier(ModifierCombination::new(
            true, false, false, false, true,
        )));
        assert_eq!(0x5511, to_via_keycode(a));

        // LCtrl(A) -> WithModifier(A)
        let a = KeyAction::WithModifier(
            Action::Key(KeyCode::A),
            ModifierCombination::new(false, false, false, false, true),
        );
        assert_eq!(0x104, to_via_keycode(a));

        // RCtrl(A) -> WithModifier(A)
        let a = KeyAction::WithModifier(
            Action::Key(KeyCode::A),
            ModifierCombination::new(true, false, false, false, true),
        );
        assert_eq!(0x1104, to_via_keycode(a));

        // Meh(A) -> WithModifier(A)
        let a = KeyAction::WithModifier(
            Action::Key(KeyCode::A),
            ModifierCombination::new(false, false, true, true, true),
        );
        assert_eq!(0x704, to_via_keycode(a));

        // Hypr(A) -> WithModifier(A)
        let a = KeyAction::WithModifier(
            Action::Key(KeyCode::A),
            ModifierCombination::new(false, true, true, true, true),
        );
        assert_eq!(0xF04, to_via_keycode(a));

        // LT0(A) -> LayerTapHold(A, 0)
        let a = KeyAction::LayerTapHold(Action::Key(KeyCode::A), 0);
        assert_eq!(0x4004, to_via_keycode(a));

        // LT3(A) -> LayerTapHold(A, 3)
        let a = KeyAction::LayerTapHold(Action::Key(KeyCode::A), 3);
        assert_eq!(0x4304, to_via_keycode(a));

        // LSA_T(A) ->
        let a = KeyAction::ModifierTapHold(
            Action::Key(KeyCode::A),
            ModifierCombination::new(false, false, true, true, false),
        );
        assert_eq!(0x6604, to_via_keycode(a));

        // RCAG_T(A) ->
        let a = KeyAction::ModifierTapHold(
            Action::Key(KeyCode::A),
            ModifierCombination::new(true, true, true, false, true),
        );
        assert_eq!(0x7D04, to_via_keycode(a));

        // ALL_T(A) ->
        let a = KeyAction::ModifierTapHold(
            Action::Key(KeyCode::A),
            ModifierCombination::new(false, true, true, true, true),
        );
        assert_eq!(0x6F04, to_via_keycode(a));

        // Meh_T(A) ->

        let a = KeyAction::ModifierTapHold(
            Action::Key(KeyCode::A),
            ModifierCombination::new(false, false, true, true, true),
        );
        assert_eq!(0x6704, to_via_keycode(a));
    }
}
