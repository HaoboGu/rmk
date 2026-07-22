pub mod common;

mod macro_test {
    use heapless::Vec;
    use rmk::config::{BehaviorConfig, PositionalConfig};
    use rmk::keyboard::Keyboard;
    use rmk::keyboard_macros::{MacroOperation, define_macro_sequences, to_macro_sequence};
    use rmk::types::action::{Action, KeyAction};
    use rmk_types::keycode::HidKeyCode;

    use crate::common::{KC_LSHIFT, wrap_keymap};
    use crate::{kc_to_u8, key_sequence_test};

    fn create_simple_macro_keyboard(behavior_config: BehaviorConfig) -> Keyboard<'static> {
        let keymap = [[[
            KeyAction::Single(Action::TriggerMacro(0)),
            KeyAction::Single(Action::TriggerMacro(1)),
        ]]];
        let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(behavior_config));
        let per_key_config: &'static PositionalConfig<1, 2> = Box::leak(Box::new(PositionalConfig::default()));
        Keyboard::new(wrap_keymap(keymap, per_key_config, behavior_config))
    }

    #[test]
    fn test_macro_key_a_press_release() {
        let macro_sequences = &[Vec::from_slice(&[
            MacroOperation::Press(HidKeyCode::A),
            MacroOperation::Release(HidKeyCode::A),
        ])
        .expect("too many elements")];

        let macro_data = define_macro_sequences(macro_sequences);
        let mut config = BehaviorConfig::default();
        config.keyboard_macros.macro_sequences = macro_data;

        let keyboard = create_simple_macro_keyboard(config);

        key_sequence_test!(
            keyboard: keyboard,
            sequence: [
                [0, 0, true, 0],   // press Macro0
                [0, 0, false, 100], // release Macro0
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // press A
                [0, [0, 0, 0, 0, 0, 0]],            // release A
            ]
        );
    }

    // Macros fire on key *press* (matching QMK/ZMK), not release: the macro key is pressed and
    // never released here, yet the macro still runs to completion.
    #[test]
    fn test_macro_triggers_on_press() {
        let macro_sequences = &[Vec::from_slice(&[
            MacroOperation::Press(HidKeyCode::A),
            MacroOperation::Release(HidKeyCode::A),
        ])
        .expect("too many elements")];

        let macro_data = define_macro_sequences(macro_sequences);
        let mut config = BehaviorConfig::default();
        config.keyboard_macros.macro_sequences = macro_data;

        let keyboard = create_simple_macro_keyboard(config);

        key_sequence_test!(
            keyboard: keyboard,
            sequence: [
                [0, 0, true, 0], // press Macro0 only (no release)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // A down — emitted on press
                [0, [0, 0, 0, 0, 0, 0]],            // A up
            ]
        );
    }

    #[test]
    fn test_macro_text() {
        let macro_sequences = &[to_macro_sequence("AbCd123456")];

        let macro_data = define_macro_sequences(macro_sequences);
        let mut config = BehaviorConfig::default();
        config.keyboard_macros.macro_sequences = macro_data;

        let keyboard = create_simple_macro_keyboard(config);

        key_sequence_test!(
            keyboard: keyboard,
            sequence: [
                [0, 0, true, 0],   // press Macro0
                [0, 0, false, 100], // release Macro0
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],            // press shift
                [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // press A + shift
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],            // release A
                [0, [0, 0, 0, 0, 0, 0]],            // release shift
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // press B
                [0, [0, 0, 0, 0, 0, 0]],            // release B
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],            // press shift
                [KC_LSHIFT, [kc_to_u8!(C), 0, 0, 0, 0, 0]], // press C + shift
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],            // release C
                [0, [0, 0, 0, 0, 0, 0]],            // release shift
                [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]], // press D
                [0, [0, 0, 0, 0, 0, 0]],            // release D
                [0, [kc_to_u8!(Kc1), 0, 0, 0, 0, 0]], // press 1
                [0, [0, 0, 0, 0, 0, 0]],            // release 1
                [0, [kc_to_u8!(Kc2), 0, 0, 0, 0, 0]], // press 2
                [0, [0, 0, 0, 0, 0, 0]],            // release 2
                [0, [kc_to_u8!(Kc3), 0, 0, 0, 0, 0]], // press 3
                [0, [0, 0, 0, 0, 0, 0]],            // release 3
                [0, [kc_to_u8!(Kc4), 0, 0, 0, 0, 0]], // press 4
                [0, [0, 0, 0, 0, 0, 0]],            // release 4
                [0, [kc_to_u8!(Kc5), 0, 0, 0, 0, 0]], // press 5
                [0, [0, 0, 0, 0, 0, 0]],            // release 5
                [0, [kc_to_u8!(Kc6), 0, 0, 0, 0, 0]], // press 6
                [0, [0, 0, 0, 0, 0, 0]],            // release 6
            ]
        );
    }

    #[test]
    fn test_macro_tap_key_a() {
        let macro_sequences = &[Vec::from_slice(&[MacroOperation::Tap(HidKeyCode::A)]).expect("too many elements")];

        let macro_data = define_macro_sequences(macro_sequences);
        let mut config = BehaviorConfig::default();
        config.keyboard_macros.macro_sequences = macro_data;

        let keyboard = create_simple_macro_keyboard(config);

        key_sequence_test!(
            keyboard: keyboard,
            sequence: [
                [0, 0, true, 0],   // press Macro0
                [0, 0, false, 100], // release Macro0
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // press A
                [0, [0, 0, 0, 0, 0, 0]],            // release A
            ]
        );
    }

    #[test]
    fn test_macro_multiple_operations() {
        let macro_sequences = &[Vec::from_slice(&[
            MacroOperation::Press(HidKeyCode::LShift),
            MacroOperation::Tap(HidKeyCode::A),
            MacroOperation::Release(HidKeyCode::LShift),
            MacroOperation::Tap(HidKeyCode::B),
        ])
        .expect("too many elements")];

        let macro_data = define_macro_sequences(macro_sequences);
        let mut config = BehaviorConfig::default();
        config.keyboard_macros.macro_sequences = macro_data;

        let keyboard = create_simple_macro_keyboard(config);

        key_sequence_test!(
            keyboard: keyboard,
            sequence: [
                [0, 0, true, 0],   // press macro0
                [0, 0, false, 100], // release macro0
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],        // press shift
                [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // press shift + A
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],        // release A
                [0, [0, 0, 0, 0, 0, 0]],           // release shift
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // press B
                [0, [0, 0, 0, 0, 0, 0]],           // release B
            ]
        );
    }

    #[test]
    fn test_macro_with_delay() {
        let macro_sequences = &[Vec::from_slice(&[
            MacroOperation::Tap(HidKeyCode::A),
            MacroOperation::Delay(50 << 8), // 50 ms
            MacroOperation::Tap(HidKeyCode::B),
        ])
        .expect("too many elements")];

        let macro_data = define_macro_sequences(macro_sequences);
        let mut config = BehaviorConfig::default();
        config.keyboard_macros.macro_sequences = macro_data;

        let keyboard = create_simple_macro_keyboard(config);

        key_sequence_test!(
            keyboard: keyboard,
            sequence: [
                [0, 0, true, 0],
                [0, 0, false, 100],
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // press A
                [0, [0, 0, 0, 0, 0, 0]],            // release A
                // Delay 50 ms
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // press B
                [0, [0, 0, 0, 0, 0, 0]],            // release B
            ]
        );
    }

    // A 16-bit Vial keycode (LCtrl(A) = 0x0104) used as a macro TAP action is serialized as
    // VIAL_MACRO_EXT_TAP, decoded, and routed through the shared action path so the modifier
    // is applied exactly like a physical key. This is the mechanism that makes BT/PDF (and any
    // other 16-bit keycode) work inside a macro.
    #[cfg(feature = "vial")]
    #[test]
    fn test_macro_extended_tap_key_with_modifier() {
        use rmk::types::modifier::ModifierCombination;

        use crate::common::KC_LCTRL;

        let macro_sequences =
            &[
                Vec::from_slice(&[MacroOperation::TapAction(KeyAction::Single(Action::KeyWithModifier(
                    HidKeyCode::A,
                    ModifierCombination::LCTRL,
                )))])
                .expect("too many elements"),
            ];

        let macro_data = define_macro_sequences(macro_sequences);
        let mut config = BehaviorConfig::default();
        config.keyboard_macros.macro_sequences = macro_data;

        let keyboard = create_simple_macro_keyboard(config);

        key_sequence_test!(
            keyboard: keyboard,
            sequence: [
                [0, 0, true, 0],    // press Macro0
                [0, 0, false, 100], // release Macro0
            ],
            expected_reports: [
                [KC_LCTRL, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // press A with Left Ctrl
                [0, [0, 0, 0, 0, 0, 0]],                   // release
            ]
        );
    }

    // A macro cannot trigger another macro (which would re-enter the trigger queue and could loop
    // forever). Macro 0 tries to trigger macro 1, then taps A: the nested trigger is dropped, so
    // only A is emitted — B (macro 1) never runs — and the rest of macro 0 still executes.
    #[cfg(feature = "vial")]
    #[test]
    fn test_macro_cannot_trigger_macro() {
        let macro_sequences = &[
            Vec::from_slice(&[
                MacroOperation::TapAction(KeyAction::Single(Action::TriggerMacro(1))),
                MacroOperation::Tap(HidKeyCode::A),
            ])
            .expect("too many elements"),
            Vec::from_slice(&[MacroOperation::Tap(HidKeyCode::B)]).expect("too many elements"),
        ];

        let macro_data = define_macro_sequences(macro_sequences);
        let mut config = BehaviorConfig::default();
        config.keyboard_macros.macro_sequences = macro_data;

        let keyboard = create_simple_macro_keyboard(config);

        key_sequence_test!(
            keyboard: keyboard,
            sequence: [
                [0, 0, true, 0], // press Macro0 (no release needed — fires on press)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // A only; macro 1 was not triggered
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        );
    }

    // A mod-tap keycode (LSFT_T(A)) tapped in a macro resolves to its tap action: plain A, no
    // shift. Full tap-hold resolution can't run inside synchronous macro playback, so a macro
    // TAP is by definition the tap side.
    #[cfg(feature = "vial")]
    #[test]
    fn test_macro_extended_tap_mod_tap_sends_tap_action() {
        use rmk::types::keycode::KeyCode;
        use rmk::types::modifier::ModifierCombination;

        let macro_sequences = &[Vec::from_slice(&[MacroOperation::TapAction(KeyAction::TapHold(
            Action::Key(KeyCode::Hid(HidKeyCode::A)),
            Action::Modifier(ModifierCombination::LSHIFT),
            Default::default(),
        ))])
        .expect("too many elements")];

        let macro_data = define_macro_sequences(macro_sequences);
        let mut config = BehaviorConfig::default();
        config.keyboard_macros.macro_sequences = macro_data;

        let keyboard = create_simple_macro_keyboard(config);

        key_sequence_test!(
            keyboard: keyboard,
            sequence: [
                [0, 0, true, 0],    // press Macro0
                [0, 0, false, 100], // release Macro0
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // press A — tap side, no shift
                [0, [0, 0, 0, 0, 0, 0]],            // release A
            ]
        );
    }

    // DOWN/UP of a mod-tap in a macro resolve to the hold action: shift wraps the B tap,
    // matching what holding the physical key would do.
    #[cfg(feature = "vial")]
    #[test]
    fn test_macro_extended_down_up_mod_tap_sends_hold_action() {
        use rmk::types::keycode::KeyCode;
        use rmk::types::modifier::ModifierCombination;

        let mod_tap = KeyAction::TapHold(
            Action::Key(KeyCode::Hid(HidKeyCode::A)),
            Action::Modifier(ModifierCombination::LSHIFT),
            Default::default(),
        );
        let macro_sequences = &[Vec::from_slice(&[
            MacroOperation::PressAction(mod_tap),
            MacroOperation::Tap(HidKeyCode::B),
            MacroOperation::ReleaseAction(mod_tap),
        ])
        .expect("too many elements")];

        let macro_data = define_macro_sequences(macro_sequences);
        let mut config = BehaviorConfig::default();
        config.keyboard_macros.macro_sequences = macro_data;

        let keyboard = create_simple_macro_keyboard(config);

        key_sequence_test!(
            keyboard: keyboard,
            sequence: [
                [0, 0, true, 0],    // press Macro0
                [0, 0, false, 100], // release Macro0
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],            // press shift — hold side
                [KC_LSHIFT, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // press B with shift held
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],            // release B
                [0, [0, 0, 0, 0, 0, 0]],                    // release shift
            ]
        );
    }

    // A tap-dance keycode in a macro resolves through the keymap's morse table: TAP plays the
    // morse TAP action, DOWN/UP play the HOLD action.
    #[cfg(feature = "vial")]
    #[test]
    fn test_macro_extended_tap_dance() {
        use rmk::types::keycode::KeyCode;
        use rmk::types::modifier::ModifierCombination;
        use rmk::types::morse::{HOLD, Morse, TAP};

        let macro_sequences = &[Vec::from_slice(&[
            MacroOperation::TapAction(KeyAction::Morse(0)),
            MacroOperation::PressAction(KeyAction::Morse(0)),
            MacroOperation::Tap(HidKeyCode::B),
            MacroOperation::ReleaseAction(KeyAction::Morse(0)),
        ])
        .expect("too many elements")];

        let macro_data = define_macro_sequences(macro_sequences);
        let mut config = BehaviorConfig::default();
        config.keyboard_macros.macro_sequences = macro_data;
        let mut tap_dance = Morse::default();
        let _ = tap_dance.put(TAP, Action::Key(KeyCode::Hid(HidKeyCode::A)));
        let _ = tap_dance.put(HOLD, Action::Modifier(ModifierCombination::LSHIFT));
        config.morse.morses.push(tap_dance).unwrap();

        let keyboard = create_simple_macro_keyboard(config);

        key_sequence_test!(
            keyboard: keyboard,
            sequence: [
                [0, 0, true, 0],    // press Macro0
                [0, 0, false, 100], // release Macro0
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],         // tap: morse TAP action (A)
                [0, [0, 0, 0, 0, 0, 0]],                    // release A
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],            // down: morse HOLD action (shift)
                [KC_LSHIFT, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // press B with shift held
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],            // release B
                [0, [0, 0, 0, 0, 0, 0]],                    // up: release shift
            ]
        );
    }
}
