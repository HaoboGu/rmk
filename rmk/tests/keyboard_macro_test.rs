pub mod common;

mod macro_test {
    use embassy_futures::block_on;
    use heapless::Vec;
    use rmk::action::{Action, KeyAction};
    use rmk::config::BehaviorConfig;
    use rmk::keyboard::Keyboard;
    use rmk::keyboard_macros::{define_macro_sequences, to_macro_sequence, MacroOperation};
    use rmk::keycode::KeyCode;
    use rusty_fork::rusty_fork_test;

    use crate::common::{wrap_keymap, KC_LSHIFT};
    use crate::{kc_to_u8, key_sequence_test};

    fn create_simple_macro_keyboard(behavior_config: BehaviorConfig) -> Keyboard<'static, 1, 2, 1> {
        let keymap = [[[
            KeyAction::Single(Action::Key(KeyCode::Macro0)),
            KeyAction::Single(Action::Key(KeyCode::Macro1)),
        ]]];

        Keyboard::new(wrap_keymap(keymap, behavior_config))
    }

    rusty_fork_test! {

        #[test]
        fn test_macro_key_a_press_release() {
            let macro_sequences = &[Vec::from_slice(&[
                MacroOperation::Press(KeyCode::A),
                MacroOperation::Release(KeyCode::A),
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
            let macro_sequences = &[Vec::from_slice(&[MacroOperation::Tap(KeyCode::A)]).expect("too many elements")];

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
                MacroOperation::Press(KeyCode::LShift),
                MacroOperation::Tap(KeyCode::A),
                MacroOperation::Release(KeyCode::LShift),
                MacroOperation::Tap(KeyCode::B),
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
                MacroOperation::Tap(KeyCode::A),
                MacroOperation::Delay(50 << 8), // 50 ms
                MacroOperation::Tap(KeyCode::B),
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
    }
}
