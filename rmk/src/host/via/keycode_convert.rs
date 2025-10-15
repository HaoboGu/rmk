use rmk_types::action::{Action, KeyAction};
use rmk_types::keycode::KeyCode;
use rmk_types::modifier::ModifierCombination;

pub(crate) fn to_via_keycode(key_action: KeyAction) -> u16 {
    match key_action {
        KeyAction::No => 0x0000,
        KeyAction::Transparent => 0x0001,
        KeyAction::Single(a) => match a {
            Action::Key(KeyCode::GraveEscape) => 0x7c16,
            Action::Key(KeyCode::RepeatKey) => 0x7c79,
            Action::Key(KeyCode::CapsWordToggle) => 0x7c73,
            Action::Key(KeyCode::TriLayerLower) => 0x7c77,
            Action::Key(KeyCode::TriLayerUpper) => 0x7c78,
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
            Action::KeyWithModifier(k, m) => ((m.into_packed_bits() as u16) << 8) | k as u16,
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
            Action::OneShotLayer(l) => {
                // One-shot layer
                if l < 16 { 0x5280 | l as u16 } else { 0x0000 }
            }
            Action::OneShotModifier(m) => {
                // One-shot modifier
                let modifier_bits = m.into_packed_bits();
                0x52A0 | modifier_bits as u16
            }
            Action::LayerOnWithModifier(l, m) => {
                if l < 16 {
                    0x5000 | ((l as u16) << 5) | ((m.into_packed_bits() & 0b11111) as u16)
                } else {
                    0
                }
            }
            _ => 0x0000,
        },
        KeyAction::Tap(_) => {
            warn!("Tap action is not supported by via");
            0
        }
        KeyAction::TapHold(tap, hold, _) => match hold {
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
                0x2000 | ((m.into_packed_bits() as u16) << 8) | keycode
            }
            _ => 0x0000,
        },
        KeyAction::Morse(index) => {
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
        0x0002..=0x00FF => KeyAction::Single(Action::Key(via_keycode.into())),
        0x0100..=0x1FFF => {
            // WithModifier
            let keycode = (via_keycode & 0x00FF).into();
            let modifier = ModifierCombination::from_packed_bits((via_keycode >> 8) as u8);
            KeyAction::Single(Action::KeyWithModifier(keycode, modifier))
        }
        0x2000..=0x3FFF => {
            // Modifier tap-hold.
            let keycode = (via_keycode & 0x00FF).into();
            let modifier = ModifierCombination::from_packed_bits(((via_keycode >> 8) & 0b11111) as u8);
            KeyAction::TapHold(Action::Key(keycode), Action::Modifier(modifier), Default::default())
        }
        0x4000..=0x4FFF => {
            // Layer tap-hold.
            let layer = (via_keycode >> 8) & 0xF;
            let keycode = (via_keycode & 0x00FF).into();
            KeyAction::TapHold(Action::Key(keycode), Action::LayerOn(layer as u8), Default::default())
        }
        0x5000..=0x51FF => {
            let layer = (via_keycode >> 5) & 0xF;
            let modifier = ModifierCombination::from_packed_bits((via_keycode & 0b11111) as u8);
            KeyAction::Single(Action::LayerOnWithModifier(layer as u8, modifier))
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
            KeyAction::Single(Action::OneShotLayer(layer))
        }
        0x52A0..=0x52BF => {
            // One-shot modifier
            let m = ModifierCombination::from_packed_bits((via_keycode & 0x1F) as u8);
            KeyAction::Single(Action::OneShotModifier(m))
        }
        0x52C0..=0x52DF => {
            // TODO: Layer tap toggle
            warn!("Layer tap toggle {:#X} not supported", via_keycode);
            KeyAction::No
        }
        0x5700..=0x57FF => {
            // Tap dance
            let index = (via_keycode & 0xFF) as u8;
            KeyAction::Morse(index)
        }
        0x7000..=0x701F => {
            // TODO: QMK functions, such as swap ctrl/caps, gui on, haptic, music, clicky, combo, RGB, etc
            warn!("QMK functions {:#X} not supported", via_keycode);
            KeyAction::No
        }
        0x7700..=0x770F => {
            // Macro
            let keycode = via_keycode & 0xFF | 0x500;
            KeyAction::Single(Action::Key(keycode.into()))
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
            KeyAction::Single(Action::Key(keycode.into()))
        }
        // GraveEscape
        0x7C16 => KeyAction::Single(Action::Key(KeyCode::GraveEscape)),
        // RepeatKey
        0x7C79 => KeyAction::Single(Action::Key(KeyCode::RepeatKey)),
        // Caps Word
        0x7C73 => KeyAction::Single(Action::Key(KeyCode::CapsWordToggle)),
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
        // TriLayer Lower
        0x7C77 => KeyAction::Single(Action::Key(KeyCode::TriLayerLower)),
        // TriLayer Upper
        0x7C78 => KeyAction::Single(Action::Key(KeyCode::TriLayerUpper)),
        0x7E00..=0x7E0F => {
            // QK_KB_N, aka UserN
            let keycode = via_keycode & 0xFF | 0x840;
            KeyAction::Single(Action::Key(keycode.into()))
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
    use crate::types::keycode::{from_ascii, to_ascii};

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
        assert_eq!(
            KeyAction::Single(Action::OneShotLayer(3)),
            from_via_keycode(via_keycode)
        );

        // OSM RCtrl
        let via_keycode = 0x52B1;
        assert_eq!(
            KeyAction::Single(Action::OneShotModifier(ModifierCombination::new_from(
                true, false, false, false, true
            ))),
            from_via_keycode(via_keycode)
        );

        // LCtrl(A) -> WithModifier(A)
        let via_keycode = 0x104;
        assert_eq!(
            KeyAction::Single(Action::KeyWithModifier(
                KeyCode::A,
                ModifierCombination::new_from(false, false, false, false, true)
            )),
            from_via_keycode(via_keycode)
        );

        // RCtrl(A) -> WithModifier(A)
        let via_keycode = 0x1104;
        assert_eq!(
            KeyAction::Single(Action::KeyWithModifier(
                KeyCode::A,
                ModifierCombination::new_from(true, false, false, false, true)
            )),
            from_via_keycode(via_keycode)
        );

        // Meh(A) -> WithModifier(A)
        let via_keycode = 0x704;
        assert_eq!(
            KeyAction::Single(Action::KeyWithModifier(
                KeyCode::A,
                ModifierCombination::new_from(false, false, true, true, true)
            )),
            from_via_keycode(via_keycode)
        );

        // Hypr(A) -> WithModifier(A)
        let via_keycode = 0xF04;
        assert_eq!(
            KeyAction::Single(Action::KeyWithModifier(
                KeyCode::A,
                ModifierCombination::new_from(false, true, true, true, true)
            )),
            from_via_keycode(via_keycode)
        );

        // LT0(A) -> LayerTapHold(A, 0)
        let via_keycode = 0x4004;
        assert_eq!(
            KeyAction::TapHold(Action::Key(KeyCode::A), Action::LayerOn(0), Default::default()),
            from_via_keycode(via_keycode)
        );

        // LT3(A) -> LayerTapHold(A, 3)
        let via_keycode = 0x4304;
        assert_eq!(
            KeyAction::TapHold(Action::Key(KeyCode::A), Action::LayerOn(3), Default::default()),
            from_via_keycode(via_keycode)
        );

        // LSA_T(A) ->
        let via_keycode = 0x2604;
        assert_eq!(
            KeyAction::TapHold(
                Action::Key(KeyCode::A),
                Action::Modifier(ModifierCombination::new_from(false, false, true, true, false)),
                Default::default(),
            ), //hrm
            from_via_keycode(via_keycode)
        );

        // RCAG_T(B) ->
        let via_keycode = 0x3D05;
        assert_eq!(
            KeyAction::TapHold(
                Action::Key(KeyCode::B),
                Action::Modifier(ModifierCombination::new_from(true, true, true, false, true)),
                Default::default(),
            ),
            from_via_keycode(via_keycode)
        );

        // ALL_T(A) ->
        let via_keycode: u16 = 0x2F04;
        assert_eq!(
            KeyAction::TapHold(
                Action::Key(KeyCode::A),
                Action::Modifier(ModifierCombination::new_from(false, true, true, true, true)),
                Default::default(),
            ), //hrm
            from_via_keycode(via_keycode)
        );

        // Meh_T(B) ->
        let via_keycode = 0x2705;
        assert_eq!(
            KeyAction::TapHold(
                Action::Key(KeyCode::B),
                Action::Modifier(ModifierCombination::new_from(false, false, true, true, true)),
                Default::default(),
            ),
            from_via_keycode(via_keycode)
        );

        // LM(1, LSHIFT)
        let via_keycode = 0x5022;
        assert_eq!(
            KeyAction::Single(Action::LayerOnWithModifier(1, ModifierCombination::LSHIFT)),
            from_via_keycode(via_keycode)
        );

        // LM(15, RGUI | RCTRL)
        let via_keycode = 0x5039;
        assert_eq!(
            KeyAction::Single(Action::LayerOnWithModifier(
                1,
                ModifierCombination::new().with_right_gui(true).with_right_ctrl(true)
            )),
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

        // Morse(0)
        let via_keycode = 0x5700;
        assert_eq!(KeyAction::Morse(0), from_via_keycode(via_keycode));

        // Morse(5)
        let via_keycode = 0x5705;
        assert_eq!(KeyAction::Morse(5), from_via_keycode(via_keycode));

        // Morse(255)
        let via_keycode = 0x57FF;
        assert_eq!(KeyAction::Morse(255), from_via_keycode(via_keycode));
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
        let a = KeyAction::Single(Action::OneShotLayer(3));
        assert_eq!(0x5283, to_via_keycode(a));

        // OSM RCtrl
        let a = KeyAction::Single(Action::OneShotModifier(ModifierCombination::new_from(
            true, false, false, false, true,
        )));
        assert_eq!(0x52B1, to_via_keycode(a));

        // LCtrl(A) -> WithModifier(A)
        let a = KeyAction::Single(Action::KeyWithModifier(
            KeyCode::A,
            ModifierCombination::new_from(false, false, false, false, true),
        ));
        assert_eq!(0x104, to_via_keycode(a));

        // RCtrl(A) -> WithModifier(A)
        let a = KeyAction::Single(Action::KeyWithModifier(
            KeyCode::A,
            ModifierCombination::new_from(true, false, false, false, true),
        ));
        assert_eq!(0x1104, to_via_keycode(a));

        // Meh(A) -> WithModifier(A)
        let a = KeyAction::Single(Action::KeyWithModifier(
            KeyCode::A,
            ModifierCombination::new_from(false, false, true, true, true),
        ));
        assert_eq!(0x704, to_via_keycode(a));

        // Hypr(A) -> WithModifier(A)
        let a = KeyAction::Single(Action::KeyWithModifier(
            KeyCode::A,
            ModifierCombination::new_from(false, true, true, true, true),
        ));
        assert_eq!(0xF04, to_via_keycode(a));

        // LT0(A) -> LayerTapHold(A, 0)
        let a = KeyAction::TapHold(Action::Key(KeyCode::A), Action::LayerOn(0), Default::default());
        assert_eq!(0x4004, to_via_keycode(a));

        // LT3(A) -> LayerTapHold(A, 3)
        let a = KeyAction::TapHold(Action::Key(KeyCode::A), Action::LayerOn(3), Default::default());
        assert_eq!(0x4304, to_via_keycode(a));

        // LSA_T(A) ->
        let a = KeyAction::TapHold(
            Action::Key(KeyCode::A),
            Action::Modifier(ModifierCombination::new_from(false, false, true, true, false)),
            Default::default(),
        );
        assert_eq!(0x2604, to_via_keycode(a));

        // RCAG_T(A) ->
        let a = KeyAction::TapHold(
            Action::Key(KeyCode::A),
            Action::Modifier(ModifierCombination::new_from(true, true, true, false, true)),
            Default::default(),
        );
        assert_eq!(0x3D04, to_via_keycode(a));

        // ALL_T(A) ->
        let a = KeyAction::TapHold(
            Action::Key(KeyCode::A),
            Action::Modifier(ModifierCombination::new_from(false, true, true, true, true)),
            Default::default(),
        );
        assert_eq!(0x2F04, to_via_keycode(a));

        // Meh_T(A) ->
        let a = KeyAction::TapHold(
            Action::Key(KeyCode::A),
            Action::Modifier(ModifierCombination::new_from(false, false, true, true, true)),
            Default::default(),
        );
        assert_eq!(0x2704, to_via_keycode(a));

        // LM(1, LSHIFT)
        let a = KeyAction::Single(Action::LayerOnWithModifier(1, ModifierCombination::LSHIFT));
        assert_eq!(0x5022, to_via_keycode(a));

        // LM(15, RGUI | RCTRL)
        let a = KeyAction::Single(Action::LayerOnWithModifier(
            1,
            ModifierCombination::new().with_right_gui(true).with_right_ctrl(true),
        ));
        assert_eq!(0x5039, to_via_keycode(a));

        // ComboOff
        let a = KeyAction::Single(Action::Key(KeyCode::ComboOff));
        assert_eq!(0x7C51, to_via_keycode(a));

        // GraveEscape
        let a = KeyAction::Single(Action::Key(KeyCode::GraveEscape));
        assert_eq!(0x7C16, to_via_keycode(a));

        // RepeatKey
        let a = KeyAction::Single(Action::Key(KeyCode::RepeatKey));
        assert_eq!(0x7C79, to_via_keycode(a));

        // Morse
        let a = KeyAction::Morse(0);
        assert_eq!(0x5700, to_via_keycode(a));

        let a = KeyAction::Morse(5);
        assert_eq!(0x5705, to_via_keycode(a));

        let a = KeyAction::Morse(255);
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
