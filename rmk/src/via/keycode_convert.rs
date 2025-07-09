use num_enum::FromPrimitive;

use crate::action::{Action, KeyAction};
use crate::keycode::{KeyCode, ModifierCombination};

pub(crate) fn to_via_keycode(key_action: KeyAction) -> u16 {
    match key_action {
        KeyAction::No => 0x0000,
        KeyAction::Transparent => 0x0001,
        KeyAction::Single(a) => match a {
            Action::Key(KeyCode::GraveEscape) => 0x7c16,
            Action::Key(KeyCode::RepeatKey) => 0x7c79,
            Action::Key(k) => {
                if k.is_macro() {
                    k as u16 & 0xFF | 0x7700
                } else if k.is_user() {
                    k as u16 & 0xF | 0x7E00
                } else if k.is_combo() || k.is_boot() {
                    // is_rmk() 's subset
                    k as u16 & 0xFF | 0x7C00
                } else {
                    k as u16
                }
            }
            Action::LayerToggleOnly(l) => 0x5200 | l as u16,
            Action::LayerOn(l) => 0x5220 | l as u16,
            Action::DefaultLayer(l) => 0x5240 | l as u16,
            Action::LayerToggle(l) => 0x5260 | l as u16,
            // convert to KeyCode::Macro0 - Macro31, 0x0 for above (as 0x600 is already reserved)
            Action::TriggerMacro(idx) => {
                // if idx < 32 {
                0x7700 + (idx as u16)
                // } else {
                // 0x0
                // }
            }
            _ => 0x0000,
        },
        KeyAction::Tap(_) => {
            warn!("Tap action is not supported by via");
            0
        }
        KeyAction::OneShot(a) => match a {
            Action::Modifier(m) => {
                // One-shot modifier
                let modifier_bits = m.into_bits();
                0x52A0 | modifier_bits as u16
            }
            Action::LayerOn(l) => {
                // One-shot layer
                if l < 16 {
                    0x5280 | l as u16
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
            ((m.into_bits() as u16) << 8) | keycode
        }
        KeyAction::TapHold(tap, hold) => match hold {
            Action::LayerOn(l) => {
                if l > 16 {
                    0
                } else {
                    let keycode = match tap {
                        Action::Key(k) => k as u16,
                        _ => 0,
                    };
                    0x4000 | ((l as u16) << 8) | keycode
                }
            }
            Action::Modifier(m) => {
                let keycode = match tap {
                    Action::Key(k) => k as u16,
                    _ => 0,
                };
                0x2000 | ((m.into_bits() as u16) << 8) | keycode
            }
            _ => 0x0000,
        },
        KeyAction::TapDance(index) => {
            // Tap dance keycodes: 0x5700..=0x57FF
            0x5700 | (index as u16)
        }
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
        0x2000..=0x3FFF => {
            // Modifier tap/hold
            // The via equivalent of Modifier tap/hold is called Mod-tap, whose keycode representation is same with RMK
            let keycode = KeyCode::from_primitive(via_keycode & 0x00FF);
            let modifier = ModifierCombination::from_bits(((via_keycode >> 8) & 0b11111) as u8);
            KeyAction::TapHold(Action::Key(keycode), Action::Modifier(modifier))
        }
        0x4000..=0x4FFF => {
            // Layer tap/hold
            // The via equivalent of Modifier tap/hold is called Mod-tap,
            let layer = (via_keycode >> 8) & 0xF;
            let keycode = KeyCode::from_primitive(via_keycode & 0x00FF);
            KeyAction::TapHold(Action::Key(keycode), Action::LayerOn(layer as u8))
        }
        0x5200..=0x521F => {
            // Activate layer X and deactivate other layers(except default layer)
            let layer = via_keycode as u8 & 0x0F;
            KeyAction::Single(Action::LayerToggleOnly(layer))
        }
        0x5220..=0x523F => {
            // Layer activate
            let layer = via_keycode as u8 & 0x0F;
            KeyAction::Single(Action::LayerOn(layer))
        }
        0x5240..=0x525F => {
            // Set default layer
            let layer = via_keycode as u8 & 0x0F;
            KeyAction::Single(Action::DefaultLayer(layer))
        }
        0x5260..=0x527F => {
            // Layer toggle
            let layer = via_keycode as u8 & 0x0F;
            KeyAction::Single(Action::LayerToggle(layer))
        }
        0x5280..=0x529F => {
            // One-shot layer
            let layer = via_keycode as u8 & 0xF;
            KeyAction::OneShot(Action::LayerOn(layer))
        }
        0x52A0..=0x52BF => {
            // One-shot modifier
            let m = ModifierCombination::from_bits((via_keycode & 0x1F) as u8);
            KeyAction::OneShot(Action::Modifier(m))
        }
        0x52C0..=0x52DF => {
            // TODO: Layer tap toggle
            warn!("Layer tap toggle {:#X} not supported", via_keycode);
            KeyAction::No
        }
        0x5700..=0x57FF => {
            // Tap dance
            let index = (via_keycode & 0xFF) as u8;
            KeyAction::TapDance(index)
        }
        0x7000..=0x701F => {
            // TODO: QMK functions, such as swap ctrl/caps, gui on, haptic, music, clicky, combo, RGB, etc
            warn!("QMK functions {:#X} not supported", via_keycode);
            KeyAction::No
        }
        0x7700..=0x770F => {
            // Macro
            let keycode = via_keycode & 0xFF | 0x500;
            KeyAction::Single(Action::Key(KeyCode::from_primitive(keycode)))
        }
        0x7800..=0x783F => {
            // TODO: backlight and rgb configuration
            warn!("Backlight and RGB configuration key not supported");
            KeyAction::No
        }
        // boot related | combo related
        0x7C00..=0x7C01 | 0x7C50..=0x7C52 => {
            // is_rmk() 's related
            let keycode = via_keycode & 0xFF | 0x700;
            KeyAction::Single(Action::Key(KeyCode::from_primitive(keycode)))
        }
        // GraveEscape
        0x7C16 => KeyAction::Single(Action::Key(KeyCode::GraveEscape)),
        // RepeatKey
        0x7C79 => KeyAction::Single(Action::Key(KeyCode::RepeatKey)),
        0x7C00..=0x7C5F => {
            // TODO: Reset/GESC/Space Cadet/Haptic/Auto shift(AS)/Dynamic macro
            // - [GESC](https://docs.qmk.fm/#/feature_grave_esc)
            // - [Space Cadet](https://docs.qmk.fm/#/feature_space_cadet)
            warn!(
                "Reset/GESC/Space Cadet/Haptic/Auto shift(AS)/Dynamic macro not supported: {:#X}",
                via_keycode
            );
            KeyAction::No
        }
        0x7E00..=0x7E0F => {
            // QK_KB_N, aka UserN
            let keycode = via_keycode & 0xFF | 0x840;
            KeyAction::Single(Action::Key(KeyCode::from_primitive(keycode)))
        }
        _ => {
            warn!("Via keycode {:#X} is not processed", via_keycode);
            KeyAction::No
        }
    }
}

/// Convert a ascii chat to keycode
/// bool, if the keycode should be shifted
/// assumes en-us keyboard mapping
pub(crate) fn from_ascii(ascii: u8) -> (KeyCode, bool) {
    match ascii {
        b'0' => (KeyCode::Kc0, false),
        b'1' => (KeyCode::Kc1, false),
        b'2' => (KeyCode::Kc2, false),
        b'3' => (KeyCode::Kc3, false),
        b'4' => (KeyCode::Kc4, false),
        b'5' => (KeyCode::Kc5, false),
        b'6' => (KeyCode::Kc6, false),
        b'7' => (KeyCode::Kc7, false),
        b'8' => (KeyCode::Kc8, false),
        b'9' => (KeyCode::Kc9, false),
        b'a' => (KeyCode::A, false),
        b'b' => (KeyCode::B, false),
        b'c' => (KeyCode::C, false),
        b'd' => (KeyCode::D, false),
        b'e' => (KeyCode::E, false),
        b'f' => (KeyCode::F, false),
        b'g' => (KeyCode::G, false),
        b'h' => (KeyCode::H, false),
        b'i' => (KeyCode::I, false),
        b'j' => (KeyCode::J, false),
        b'k' => (KeyCode::K, false),
        b'l' => (KeyCode::L, false),
        b'm' => (KeyCode::M, false),
        b'n' => (KeyCode::N, false),
        b'o' => (KeyCode::O, false),
        b'p' => (KeyCode::P, false),
        b'q' => (KeyCode::Q, false),
        b'r' => (KeyCode::R, false),
        b's' => (KeyCode::S, false),
        b't' => (KeyCode::T, false),
        b'u' => (KeyCode::U, false),
        b'v' => (KeyCode::V, false),
        b'w' => (KeyCode::W, false),
        b'x' => (KeyCode::X, false),
        b'y' => (KeyCode::Y, false),
        b'z' => (KeyCode::Z, false),
        b'A' => (KeyCode::A, true),
        b'B' => (KeyCode::B, true),
        b'C' => (KeyCode::C, true),
        b'D' => (KeyCode::D, true),
        b'E' => (KeyCode::E, true),
        b'F' => (KeyCode::F, true),
        b'G' => (KeyCode::G, true),
        b'H' => (KeyCode::H, true),
        b'I' => (KeyCode::I, true),
        b'J' => (KeyCode::J, true),
        b'K' => (KeyCode::K, true),
        b'L' => (KeyCode::L, true),
        b'M' => (KeyCode::M, true),
        b'N' => (KeyCode::N, true),
        b'O' => (KeyCode::O, true),
        b'P' => (KeyCode::P, true),
        b'Q' => (KeyCode::Q, true),
        b'R' => (KeyCode::R, true),
        b'S' => (KeyCode::S, true),
        b'T' => (KeyCode::T, true),
        b'U' => (KeyCode::U, true),
        b'V' => (KeyCode::V, true),
        b'W' => (KeyCode::W, true),
        b'X' => (KeyCode::X, true),
        b'Y' => (KeyCode::Y, true),
        b'Z' => (KeyCode::Z, true),
        b'!' => (KeyCode::Kc1, true),
        b'@' => (KeyCode::Kc2, true),
        b'#' => (KeyCode::Kc3, true),
        b'$' => (KeyCode::Kc4, true),
        b'%' => (KeyCode::Kc5, true),
        b'^' => (KeyCode::Kc6, true),
        b'&' => (KeyCode::Kc7, true),
        b'*' => (KeyCode::Kc8, true),
        b'(' => (KeyCode::Kc9, true),
        b')' => (KeyCode::Kc0, true),
        b'-' => (KeyCode::Minus, false),
        b'_' => (KeyCode::Minus, true),
        b'=' => (KeyCode::Equal, false),
        b'+' => (KeyCode::Equal, true),
        b'[' => (KeyCode::LeftBracket, false),
        b']' => (KeyCode::RightBracket, false),
        b'{' => (KeyCode::LeftBracket, true),
        b'}' => (KeyCode::RightBracket, true),
        b';' => (KeyCode::Semicolon, false),
        b':' => (KeyCode::Semicolon, true),
        b'\'' => (KeyCode::Quote, false),
        b'"' => (KeyCode::Quote, true),
        b'`' => (KeyCode::Grave, false),
        b'~' => (KeyCode::Grave, true),
        b'\\' => (KeyCode::Backslash, false),
        b'|' => (KeyCode::Backslash, true),
        b',' => (KeyCode::Comma, false),
        b'<' => (KeyCode::Comma, true),
        b'.' => (KeyCode::Dot, false),
        b'>' => (KeyCode::Dot, true),
        b'/' => (KeyCode::Slash, false),
        b'?' => (KeyCode::Slash, true),
        b' ' => (KeyCode::Space, false),
        b'\n' => (KeyCode::Enter, false),
        b'\t' => (KeyCode::Tab, false),
        b'\x08' => (KeyCode::Backspace, false),
        b'\x1B' => (KeyCode::Escape, false),
        b'\x7F' => (KeyCode::Delete, false),
        _ => (KeyCode::No, false),
    }
}

/// Convert a ascii chat to keycode
/// assumes en-us keyboard mapping
pub(crate) fn to_ascii(keycode: KeyCode, shifted: bool) -> u8 {
    match (keycode, shifted) {
        (KeyCode::Kc0, false) => b'0',
        (KeyCode::Kc1, false) => b'1',
        (KeyCode::Kc2, false) => b'2',
        (KeyCode::Kc3, false) => b'3',
        (KeyCode::Kc4, false) => b'4',
        (KeyCode::Kc5, false) => b'5',
        (KeyCode::Kc6, false) => b'6',
        (KeyCode::Kc7, false) => b'7',
        (KeyCode::Kc8, false) => b'8',
        (KeyCode::Kc9, false) => b'9',
        (KeyCode::A, false) => b'a',
        (KeyCode::B, false) => b'b',
        (KeyCode::C, false) => b'c',
        (KeyCode::D, false) => b'd',
        (KeyCode::E, false) => b'e',
        (KeyCode::F, false) => b'f',
        (KeyCode::G, false) => b'g',
        (KeyCode::H, false) => b'h',
        (KeyCode::I, false) => b'i',
        (KeyCode::J, false) => b'j',
        (KeyCode::K, false) => b'k',
        (KeyCode::L, false) => b'l',
        (KeyCode::M, false) => b'm',
        (KeyCode::N, false) => b'n',
        (KeyCode::O, false) => b'o',
        (KeyCode::P, false) => b'p',
        (KeyCode::Q, false) => b'q',
        (KeyCode::R, false) => b'r',
        (KeyCode::S, false) => b's',
        (KeyCode::T, false) => b't',
        (KeyCode::U, false) => b'u',
        (KeyCode::V, false) => b'v',
        (KeyCode::W, false) => b'w',
        (KeyCode::X, false) => b'x',
        (KeyCode::Y, false) => b'y',
        (KeyCode::Z, false) => b'z',
        (KeyCode::A, true) => b'A',
        (KeyCode::B, true) => b'B',
        (KeyCode::C, true) => b'C',
        (KeyCode::D, true) => b'D',
        (KeyCode::E, true) => b'E',
        (KeyCode::F, true) => b'F',
        (KeyCode::G, true) => b'G',
        (KeyCode::H, true) => b'H',
        (KeyCode::I, true) => b'I',
        (KeyCode::J, true) => b'J',
        (KeyCode::K, true) => b'K',
        (KeyCode::L, true) => b'L',
        (KeyCode::M, true) => b'M',
        (KeyCode::N, true) => b'N',
        (KeyCode::O, true) => b'O',
        (KeyCode::P, true) => b'P',
        (KeyCode::Q, true) => b'Q',
        (KeyCode::R, true) => b'R',
        (KeyCode::S, true) => b'S',
        (KeyCode::T, true) => b'T',
        (KeyCode::U, true) => b'U',
        (KeyCode::V, true) => b'V',
        (KeyCode::W, true) => b'W',
        (KeyCode::X, true) => b'X',
        (KeyCode::Y, true) => b'Y',
        (KeyCode::Z, true) => b'Z',
        (KeyCode::Kc1, true) => b'!',
        (KeyCode::Kc2, true) => b'@',
        (KeyCode::Kc3, true) => b'#',
        (KeyCode::Kc4, true) => b'$',
        (KeyCode::Kc5, true) => b'%',
        (KeyCode::Kc6, true) => b'^',
        (KeyCode::Kc7, true) => b'&',
        (KeyCode::Kc8, true) => b'*',
        (KeyCode::Kc9, true) => b'(',
        (KeyCode::Kc0, true) => b')',
        (KeyCode::Minus, false) => b'-',
        (KeyCode::Minus, true) => b'_',
        (KeyCode::Equal, false) => b'=',
        (KeyCode::Equal, true) => b'+',
        (KeyCode::LeftBracket, false) => b'[',
        (KeyCode::RightBracket, false) => b']',
        (KeyCode::LeftBracket, true) => b'{',
        (KeyCode::RightBracket, true) => b'}',
        (KeyCode::Semicolon, false) => b';',
        (KeyCode::Semicolon, true) => b':',
        (KeyCode::Quote, false) => b'\'',
        (KeyCode::Quote, true) => b'"',
        (KeyCode::Grave, false) => b'`',
        (KeyCode::Grave, true) => b'~',
        (KeyCode::Backslash, true) => b'\\',
        (KeyCode::Backslash, false) => b'|',
        (KeyCode::Comma, false) => b',',
        (KeyCode::Comma, true) => b'<',
        (KeyCode::Dot, false) => b'.',
        (KeyCode::Dot, true) => b'>',
        (KeyCode::Slash, false) => b'/',
        (KeyCode::Slash, true) => b'?',
        (KeyCode::Space, false) => b' ',
        (KeyCode::Enter, false) => b'\n',
        (KeyCode::Tab, false) => b'\t',
        (KeyCode::Backspace, false) => b'\x08',
        (KeyCode::Escape, false) => b'\x1B',
        (KeyCode::Delete, false) => b'\x7F',
        // not supported keycodes
        (_, _) => b'X',
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
        let via_keycode = 0x5223;
        assert_eq!(KeyAction::Single(Action::LayerOn(3)), from_via_keycode(via_keycode));

        // OSL(3)
        let via_keycode = 0x5283;
        assert_eq!(KeyAction::OneShot(Action::LayerOn(3)), from_via_keycode(via_keycode));

        // OSM RCtrl
        let via_keycode = 0x52B1;
        assert_eq!(
            KeyAction::OneShot(Action::Modifier(ModifierCombination::new_from(
                true, false, false, false, true
            ))),
            from_via_keycode(via_keycode)
        );

        // LCtrl(A) -> WithModifier(A)
        let via_keycode = 0x104;
        assert_eq!(
            KeyAction::WithModifier(
                Action::Key(KeyCode::A),
                ModifierCombination::new_from(false, false, false, false, true)
            ),
            from_via_keycode(via_keycode)
        );

        // RCtrl(A) -> WithModifier(A)
        let via_keycode = 0x1104;
        assert_eq!(
            KeyAction::WithModifier(
                Action::Key(KeyCode::A),
                ModifierCombination::new_from(true, false, false, false, true)
            ),
            from_via_keycode(via_keycode)
        );

        // Meh(A) -> WithModifier(A)
        let via_keycode = 0x704;
        assert_eq!(
            KeyAction::WithModifier(
                Action::Key(KeyCode::A),
                ModifierCombination::new_from(false, false, true, true, true)
            ),
            from_via_keycode(via_keycode)
        );

        // Hypr(A) -> WithModifier(A)
        let via_keycode = 0xF04;
        assert_eq!(
            KeyAction::WithModifier(
                Action::Key(KeyCode::A),
                ModifierCombination::new_from(false, true, true, true, true)
            ),
            from_via_keycode(via_keycode)
        );

        // LT0(A) -> LayerTapHold(A, 0)
        let via_keycode = 0x4004;
        assert_eq!(
            KeyAction::TapHold(Action::Key(KeyCode::A), Action::LayerOn(0)),
            from_via_keycode(via_keycode)
        );

        // LT3(A) -> LayerTapHold(A, 3)
        let via_keycode = 0x4304;
        assert_eq!(
            KeyAction::TapHold(Action::Key(KeyCode::A), Action::LayerOn(3)),
            from_via_keycode(via_keycode)
        );

        // LSA_T(A) ->
        let via_keycode = 0x2604;
        assert_eq!(
            KeyAction::TapHold(
                Action::Key(KeyCode::A),
                Action::Modifier(ModifierCombination::new_from(false, false, true, true, false))
            ),
            from_via_keycode(via_keycode)
        );

        // RCAG_T(A) ->
        let via_keycode = 0x3D04;
        assert_eq!(
            KeyAction::TapHold(
                Action::Key(KeyCode::A),
                Action::Modifier(ModifierCombination::new_from(true, true, true, false, true))
            ),
            from_via_keycode(via_keycode)
        );

        // ALL_T(A) ->
        let via_keycode: u16 = 0x2F04;
        assert_eq!(
            KeyAction::TapHold(
                Action::Key(KeyCode::A),
                Action::Modifier(ModifierCombination::new_from(false, true, true, true, true))
            ),
            from_via_keycode(via_keycode)
        );

        // Meh_T(A) ->
        let via_keycode = 0x2704;
        assert_eq!(
            KeyAction::TapHold(
                Action::Key(KeyCode::A),
                Action::Modifier(ModifierCombination::new_from(false, false, true, true, true))
            ),
            from_via_keycode(via_keycode)
        );

        // ComboOff
        let via_keycode = 0x7C51;
        assert_eq!(
            KeyAction::Single(Action::Key(KeyCode::ComboOff)),
            from_via_keycode(via_keycode)
        );

        // GraveEscape
        let via_keycode = 0x7C16;
        assert_eq!(
            KeyAction::Single(Action::Key(KeyCode::GraveEscape)),
            from_via_keycode(via_keycode)
        );

        // RepeatKey
        let via_keycode = 0x7C79;
        assert_eq!(
            KeyAction::Single(Action::Key(KeyCode::RepeatKey)),
            from_via_keycode(via_keycode)
        );

        // TapDance(0)
        let via_keycode = 0x5700;
        assert_eq!(KeyAction::TapDance(0), from_via_keycode(via_keycode));

        // TapDance(5)
        let via_keycode = 0x5705;
        assert_eq!(KeyAction::TapDance(5), from_via_keycode(via_keycode));

        // TapDance(255)
        let via_keycode = 0x57FF;
        assert_eq!(KeyAction::TapDance(255), from_via_keycode(via_keycode));
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
        assert_eq!(0x5223, to_via_keycode(a));

        // OSL(3)
        let a = KeyAction::OneShot(Action::LayerOn(3));
        assert_eq!(0x5283, to_via_keycode(a));

        // OSM RCtrl
        let a = KeyAction::OneShot(Action::Modifier(ModifierCombination::new_from(
            true, false, false, false, true,
        )));
        assert_eq!(0x52B1, to_via_keycode(a));

        // LCtrl(A) -> WithModifier(A)
        let a = KeyAction::WithModifier(
            Action::Key(KeyCode::A),
            ModifierCombination::new_from(false, false, false, false, true),
        );
        assert_eq!(0x104, to_via_keycode(a));

        // RCtrl(A) -> WithModifier(A)
        let a = KeyAction::WithModifier(
            Action::Key(KeyCode::A),
            ModifierCombination::new_from(true, false, false, false, true),
        );
        assert_eq!(0x1104, to_via_keycode(a));

        // Meh(A) -> WithModifier(A)
        let a = KeyAction::WithModifier(
            Action::Key(KeyCode::A),
            ModifierCombination::new_from(false, false, true, true, true),
        );
        assert_eq!(0x704, to_via_keycode(a));

        // Hypr(A) -> WithModifier(A)
        let a = KeyAction::WithModifier(
            Action::Key(KeyCode::A),
            ModifierCombination::new_from(false, true, true, true, true),
        );
        assert_eq!(0xF04, to_via_keycode(a));

        // LT0(A) -> LayerTapHold(A, 0)
        let a = KeyAction::TapHold(Action::Key(KeyCode::A), Action::LayerOn(0));
        assert_eq!(0x4004, to_via_keycode(a));

        // LT3(A) -> LayerTapHold(A, 3)
        let a = KeyAction::TapHold(Action::Key(KeyCode::A), Action::LayerOn(3));
        assert_eq!(0x4304, to_via_keycode(a));

        // LSA_T(A) ->
        let a = KeyAction::TapHold(
            Action::Key(KeyCode::A),
            Action::Modifier(ModifierCombination::new_from(false, false, true, true, false)),
        );
        assert_eq!(0x2604, to_via_keycode(a));

        // RCAG_T(A) ->
        let a = KeyAction::TapHold(
            Action::Key(KeyCode::A),
            Action::Modifier(ModifierCombination::new_from(true, true, true, false, true)),
        );
        assert_eq!(0x3D04, to_via_keycode(a));

        // ALL_T(A) ->
        let a = KeyAction::TapHold(
            Action::Key(KeyCode::A),
            Action::Modifier(ModifierCombination::new_from(false, true, true, true, true)),
        );
        assert_eq!(0x2F04, to_via_keycode(a));

        // Meh_T(A) ->
        let a = KeyAction::TapHold(
            Action::Key(KeyCode::A),
            Action::Modifier(ModifierCombination::new_from(false, false, true, true, true)),
        );
        assert_eq!(0x2704, to_via_keycode(a));

        // ComboOff
        let a = KeyAction::Single(Action::Key(KeyCode::ComboOff));
        assert_eq!(0x7C51, to_via_keycode(a));

        // GraveEscape
        let a = KeyAction::Single(Action::Key(KeyCode::GraveEscape));
        assert_eq!(0x7C16, to_via_keycode(a));

        // RepeatKey
        let a = KeyAction::Single(Action::Key(KeyCode::RepeatKey));
        assert_eq!(0x7C79, to_via_keycode(a));

        // TapDance
        let a = KeyAction::TapDance(0);
        assert_eq!(0x5700, to_via_keycode(a));

        let a = KeyAction::TapDance(5);
        assert_eq!(0x5705, to_via_keycode(a));

        let a = KeyAction::TapDance(255);
        assert_eq!(0x57FF, to_via_keycode(a));
    }

    #[test]
    fn test_convert_from_to_ascii_a() {
        let keycode = KeyCode::A;
        let shifted = false;
        let ascii = b'a';

        assert_eq!(to_ascii(keycode, shifted), ascii);
        assert_eq!(from_ascii(ascii), (keycode, shifted));
    }
    #[test]
    fn test_convert_from_to_ascii_K() {
        let keycode = KeyCode::K;
        let shifted = true;
        let ascii = b'K';

        assert_eq!(to_ascii(keycode, shifted), ascii);
        assert_eq!(from_ascii(ascii), (keycode, shifted));
    }
}
