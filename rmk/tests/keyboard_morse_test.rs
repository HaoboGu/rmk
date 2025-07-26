pub mod common;

use rmk::{
    action::KeyAction,
    config::{BehaviorConfig, TapHoldConfig},
    k,
    keyboard::Keyboard,
    keycode::ModifierCombination,
    lt,
    morse::MorseKeyMode,
    mt,
};
use rusty_fork::rusty_fork_test;

use crate::common::wrap_keymap;
use crate::common::{KC_LGUI, KC_LSHIFT};

fn create_simple_morse_keyboard(behavior_config: BehaviorConfig) -> Keyboard<'static, 1, 4, 2> {
    let mut keymap = [
        [[
            k!(A),
            mt!(B, ModifierCombination::SHIFT),
            mt!(C, ModifierCombination::GUI),
            lt!(1, D),
        ]],
        [[k!(Kp1), k!(Kp2), k!(Kp3), k!(Kp4)]],
    ];

    // Update all keys according to behavior config
    for layer in keymap.iter_mut() {
        for row in layer {
            for key in row {
                if let KeyAction::Morse(morse) = key {
                    if behavior_config.tap_hold.chordal_hold {
                        morse.chordal_hold = true;
                    }
                    morse.mode = behavior_config.tap_hold.mode;
                }
            }
        }
    }

    Keyboard::new(wrap_keymap(keymap, behavior_config))
}

mod morse_key_normal_test {
    use super::*;

    rusty_fork_test! {
        #[test]
        fn test_morse_tap() {
            key_sequence_test! {
                keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
                sequence: [
                    [0, 1, true, 10],  // Press mt!(B, LShift)
                    // Release before hold timeout
                    [0, 1, false, 100], // Release B
                ],
                expected_reports: [
                    [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Press B
                    [0, [0, 0, 0, 0, 0, 0]], // Release B
                ]
            };
        }


        #[test]
        fn test_morse_hold() {
            key_sequence_test! {
                keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
                sequence: [
                    [0, 1, true, 10],  // Press mt!(B, LShift)
                    [0, 1, false, 300], // Release B after hold timeout
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold LShift
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_morse_multi_hold() {
            key_sequence_test! {
                keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
                sequence: [
                    [0, 1, true, 10], // Press mt!(B, lshift)
                    [0, 2, true, 10], // Press mt!(C, lgui)
                    [0, 0, true, 270],  // Press A (after hold timeout)
                    [0, 0, false, 290], // Release A
                    [0, 1, false, 380], // Release B
                    [0, 2, false, 400], // Release C
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold LShift
                    [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]], // Hold LShift + LGui
                    [KC_LSHIFT | KC_LGUI, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                    [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]], // Release A
                    [KC_LGUI, [0, 0, 0, 0, 0, 0]], // Hold LGui
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        #[ignore]
        fn test_morse_hold_after_last_tapping() {
            key_sequence_test! {
                keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
                sequence: [
                    [0, 1, true, 10],  // Press mt!(B, LShift)
                    [0, 1, false, 100], // Release B
                    [0, 1, true, 100], // Hold mt!(B, LShift) after tapping
                    [0, 1, false, 400],
                ],
                expected_reports: [
                    [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Press B
                    [0, [0, 0, 0, 0, 0, 0]], // Release B
                    [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Press B
                    [0, [0, 0, 0, 0, 0, 0]], // Release B
                ]
            };
        }

        #[test]
        fn test_morse_hold_after_last_tapping_timeout() {
            key_sequence_test! {
                keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
                sequence: [
                    [0, 1, true, 10],  // Press mt!(B, LShift)
                    [0, 1, false, 100], // Release B
                    [0, 1, true, 300], // Hold mt!(B, LShift) after tapping timeout
                    [0, 1, false, 400],
                ],
                expected_reports: [
                    [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Press B
                    [0, [0, 0, 0, 0, 0, 0]], // Release B
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Press LShift
                    [0, [0, 0, 0, 0, 0, 0]], // Release LShift
                ]
            };
        }

        #[test]
        fn test_morse_rolling() {
            // For normal mode, each morse keys are independently resolved
            key_sequence_test! {
                keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
                sequence: [
                    [0, 0, true, 30], // Press A
                    [0, 1, true, 10], // Press mt!(B, LShift)
                    [0, 0, false, 10], // Release A
                    [0, 3, true, 30], // Press lt!(1, D)
                    [0, 2, true, 30], // Press mt!(C, LGui)
                    [0, 3, false, 100], // Release D
                    [0, 1, false, 10], // Release B
                    [0, 2, false, 100], // Release C
                ],
                expected_reports: [
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                    [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                    [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                    [KC_LGUI, [0, 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }
    }
}

mod morse_key_permissive_hold_test {
    use super::*;

    fn create_permissive_hold_keyboard() -> Keyboard<'static, 1, 4, 2> {
        create_simple_morse_keyboard(BehaviorConfig {
            tap_hold: TapHoldConfig {
                enable_hrm: true,
                mode: MorseKeyMode::PermissiveHold,
                chordal_hold: false,
                ..TapHoldConfig::default()
            },
            ..BehaviorConfig::default()
        })
    }

    rusty_fork_test! {
        #[test]
        fn test_morse_permissive_hold_trigger() {
            key_sequence_test! {
                keyboard: create_permissive_hold_keyboard(),
                sequence: [
                    [0, 1, true, 10], // Press mt!(B, LShift)
                    [0, 0, true, 50],  // Press A
                    [0, 0, false, 10], // Release A
                    [0, 1, false, 100], // Release mt!(B, LShift)
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold LShift
                    [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Release A
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_morse_permissive_hold_timeout() {
            key_sequence_test! {
                keyboard: create_permissive_hold_keyboard(),
                sequence: [
                    [0, 1, true, 10], // Press mt!(B, LShift)
                    [0, 0, true, 260],  // Press A after hold timeout
                    [0, 0, false, 100], // Release A
                    [0, 1, false, 100], // Release B
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold LShift
                    [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Release A
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_morse_permissive_hold_tap() {
            key_sequence_test! {
                keyboard: create_permissive_hold_keyboard(),
                sequence: [
                    [0, 0, true, 10],  // Press A
                    [0, 1, true, 10], // Press mt!(B, LShift)
                    [0, 1, false, 100], // Release B
                    [0, 0, false, 100], // Release A
                ],
                expected_reports: [
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                    [0, [kc_to_u8!(A), kc_to_u8!(B), 0, 0, 0, 0]], // Press B
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Release B
                    [0, [0, 0, 0, 0, 0, 0]], // Release A
                ]
            };
        }

        #[test]
        fn test_morse_permissive_hold_multi_tap() {
            key_sequence_test! {
                keyboard: create_permissive_hold_keyboard(),
                sequence: [
                    [0, 0, true, 10],  // Press A
                    [0, 0, false, 100], // Release A
                    [0, 1, true, 10], // Press mt!(B, LShift)
                    [0, 2, true, 10], // Press mt!(C, LShift)
                    [0, 1, false, 100], // Release B
                    [0, 2, false, 100], // Release C
                ],
                expected_reports: [
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                    [0, [0, 0, 0, 0, 0, 0]], // Release A
                    [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Press B
                    [0, [0, 0, 0, 0, 0, 0]], // Release B
                    [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]], // Release C
                    [0, [0, 0, 0, 0, 0, 0]], // Release C
                ]
            };
        }

        #[test]
        fn test_morse_permissive_hold_layer_tap() {
            key_sequence_test! {
                keyboard: create_permissive_hold_keyboard(),
                sequence: [
                    [0, 3, true, 10], // Press lt!(1, D)
                    [0, 1, true, 10], // Press mt!(B, LShift)
                    [0, 1, false, 100], // Release B
                    [0, 3, false, 10], // Release lt!(1, D)
                    [0, 0, true, 10], // Press A
                    [0, 0, false, 10], // Release A
                    [0, 3, true, 10], // Press lt!(1, D)
                    [0, 1, true, 10], // Press mt!(B, LShift)
                    [0, 1, false, 100], // Release B
                    [0, 3, false, 10], // Release lt!(1, D)
                ],
                expected_reports: [
                    [0, [kc_to_u8!(Kp2), 0, 0, 0, 0, 0]], // Press Kp2 on layer 1
                    [0, [0, 0, 0, 0, 0, 0]], // Release Kp2
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                    [0, [0, 0, 0, 0, 0, 0]], // Release A
                    [0, [kc_to_u8!(Kp2), 0, 0, 0, 0, 0]], // Press Kp2 on layer 1
                    [0, [0, 0, 0, 0, 0, 0]], // Release Kp2
                ]
            };
        }

        #[test]
        fn test_morse_permissive_hold_timeout_rolled_release() {
            key_sequence_test! {
                keyboard: create_permissive_hold_keyboard(),
                sequence: [
                    [0, 1, true, 10], // Press mt!(B, LShift)
                    [0, 0, true, 260],  // Press A after hold timeout
                    [0, 1, false, 100], // Release B
                    [0, 0, false, 100], // Release A
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold LShift
                    [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Release A
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_morse_permissive_hold_timeout_rolled_release_2() {
            key_sequence_test! {
                keyboard: create_permissive_hold_keyboard(),
                sequence: [
                    [0, 1, true, 10], // Press mt!(B, LShift)
                    [0, 0, true, 10],  // Press A
                    [0, 1, false, 300], // Release B after timeout
                    [0, 0, false, 10], // Release A
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold LShift
                    [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Release A
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_morse_permissive_hold_timeout_and_release() {
            key_sequence_test! {
                keyboard: create_permissive_hold_keyboard(),
                sequence: [
                    [0, 1, true, 10], // Press mt!(B, LShift)
                    [0, 0, true, 20],  // Press A
                    [0, 0, false, 260], // Release A  <-- Release A after "permissive hold" interval, but also after the hold-timeout
                    [0, 1, false, 100], // Release B
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold LShift
                    [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Release A
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_morse_permissive_hold_timeout_and_release_with_other_morse_key() {
        key_sequence_test! {
            keyboard: create_permissive_hold_keyboard(),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 2, true, 200],  // Press mt!(C, LGui)
                [0, 2, false, 100], // Release C  <-- Release C after "permissive hold" interval, but also after the hold-timeout
                [0, 1, false, 100], // Release B
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold LShift
                [KC_LSHIFT, [kc_to_u8!(C), 0, 0, 0, 0, 0]], // Press C
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Release C
                [0, [0, 0, 0, 0, 0, 0]], // All released
            ]
            };
        }

        #[test]
        fn test_morse_permissive_hold_rolling() {
            key_sequence_test! {
                keyboard: create_permissive_hold_keyboard(),
                sequence: [
                    [0, 1, true, 10], // Press mt!(B, LShift)
                    [0, 0, true, 50],  // Press A
                    [0, 1, false, 50], // Release B
                    [0, 0, false, 50], // Release A
                ],
                expected_reports: [
                    [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Press B
                    [0, [0, 0, 0, 0, 0, 0]], // Released B
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                    [0, [0, 0, 0, 0, 0, 0]], // Released A
                ]
            };
        }

        #[test]
        fn test_morse_permissive_hold_rolling_release_order() {
            key_sequence_test! {
                keyboard: create_permissive_hold_keyboard(),
                sequence: [
                    [0, 1, true, 10], // Press mt!(B, LShift)
                    [0, 2, true, 30], // Press mt!(C, LGui)
                    [0, 0, true, 30], // Press A
                    [0, 1, false, 50], // Release B
                    [0, 2, false, 100], // Release C
                    [0, 0, false, 100],  // Release A
                ],
                expected_reports: [
                    [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                    [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }

        #[test]
        fn test_morse_permissive_hold_rolling_release_order_2() {
            key_sequence_test! {
                keyboard: create_permissive_hold_keyboard(),
                sequence: [
                    [0, 1, true, 10], // Press mt!(B, LShift)
                    [0, 2, true, 30], // Press mt!(C, LGui)
                    [0, 0, true, 30], // Press A
                    [0, 2, false, 100], // Release C
                    [0, 1, false, 50], // Release B
                    [0, 0, false, 100],  // Release A
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }

        #[test]
        fn test_morse_permissive_hold_rolling_release_order_3() {
            key_sequence_test! {
                keyboard: create_permissive_hold_keyboard(),
                sequence: [
                    [0, 1, true, 10], // Press mt!(B, LShift)
                    [0, 2, true, 30], // Press mt!(C, LGui)
                    [0, 0, true, 30], // Press A
                    [0, 2, false, 100], // Release C
                    [0, 0, false, 100],  // Release A
                    [0, 1, false, 50], // Release B
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Trigger A before C is released?
                    [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }


        #[test]
        fn test_morse_multiple_permissive_hold() {
            key_sequence_test! {
                keyboard: create_permissive_hold_keyboard(),
                sequence: [
                    [0, 1, true, 10], // Press mt!(B, LShift)
                    [0, 2, true, 30], // Press mt!(C, LGui)
                    [0, 0, true, 30], // Press A
                    [0, 0, false, 100], // Release A
                    [0, 1, false, 50], // Release B
                    [0, 2, false, 100], // Release C
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold LShift
                    [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]], // Hold LShift + LGui
                    [KC_LSHIFT | KC_LGUI,  [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                    [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]], // Release A
                    [KC_LGUI, [ 0, 0, 0, 0, 0, 0]], // Hold LGui
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_morse_permissive_hold_complex_rolling() {
            key_sequence_test! {
                keyboard: create_permissive_hold_keyboard(),
                sequence: [
                    [0, 0, true, 30], // Press A
                    [0, 1, true, 10], // Press mt!(B, LShift)
                    [0, 0, false, 10], // Release A
                    [0, 3, true, 30], // Press lt!(1, D)
                    [0, 2, true, 30], // Press mt!(C, LGui)
                    [0, 3, false, 100], // Release D
                    [0, 1, false, 50], // Release B
                    [0, 2, false, 10], // Release C
                ],
                expected_reports: [
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [kc_to_u8!(D), 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                    // [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                    // [0, [0, 0, 0, 0, 0, 0]],
                    // [0, [kc_to_u8!(B), kc_to_u8!(D), 0, 0, 0, 0]],
                    // [0, [kc_to_u8!(B), kc_to_u8!(D), kc_to_u8!(C), 0, 0, 0]],
                    // [0, [kc_to_u8!(B), 0, kc_to_u8!(C), 0, 0, 0]],
                    // [0, [0, 0, kc_to_u8!(C), 0, 0, 0]],
                    // [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }
    }
}
