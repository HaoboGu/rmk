pub mod common;

use embassy_time::Duration;
use rmk::action::{Action, KeyAction};
use rmk::combo::Combo;
use rmk::config::{BehaviorConfig, CombosConfig, TapHoldConfig};
use rmk::keyboard::Keyboard;
use rmk::keycode::{KeyCode, ModifierCombination};
use rmk::morse::{Morse, MorseKeyMode};
use rmk::{k, lt, mt};
use rusty_fork::rusty_fork_test;

use crate::common::{KC_LGUI, KC_LSHIFT, wrap_keymap};

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

fn create_hold_on_other_key_press_keyboard() -> Keyboard<'static, 1, 4, 2> {
    create_simple_morse_keyboard(BehaviorConfig {
        tap_hold: TapHoldConfig {
            enable_hrm: true,
            mode: MorseKeyMode::HoldOnOtherPress,
            chordal_hold: false,
            ..TapHoldConfig::default()
        },
        ..BehaviorConfig::default()
    })
}

fn create_hold_on_other_key_press_keyboard_with_combo() -> Keyboard<'static, 1, 4, 2> {
    let combo_key = KeyAction::Morse(Morse::new_tap_hold_with_config(
        Action::Key(KeyCode::B),
        Action::Modifier(ModifierCombination::SHIFT),
        250,
        MorseKeyMode::HoldOnOtherPress,
        false,
    ));
    let combo_key_2 = KeyAction::Morse(Morse::new_tap_hold_with_config(
        Action::Key(KeyCode::C),
        Action::Modifier(ModifierCombination::GUI),
        250,
        MorseKeyMode::HoldOnOtherPress,
        false,
    ));
    let combo_key_3 = KeyAction::Morse(Morse::new_tap_hold_with_config(
        Action::Key(KeyCode::D),
        Action::LayerOn(1),
        250,
        MorseKeyMode::HoldOnOtherPress,
        false,
    ));
    create_simple_morse_keyboard(BehaviorConfig {
        tap_hold: TapHoldConfig {
            enable_hrm: true,
            mode: MorseKeyMode::HoldOnOtherPress,
            chordal_hold: false,
            ..TapHoldConfig::default()
        },
        combo: CombosConfig {
            combos: heapless::Vec::from_iter([
                Combo::new([combo_key, combo_key_2], k!(X), None),
                Combo::new([k!(A), combo_key], k!(Y), None),
                Combo::new([combo_key, combo_key_2, combo_key_3], k!(Z), None),
            ]),
            timeout: Duration::from_millis(50),
        },
        ..BehaviorConfig::default()
    })
}

rusty_fork_test! {
    #[test]
    fn test_tap() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
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
    fn test_hold() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
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
    fn test_mt_1() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 0, true, 10], // Press A
                [0, 0, false, 10], // Release A
                [0, 1, false, 10], // Release mt!(B, LShift)
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Permissive hold
                [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Release A
                [0, [0, 0, 0, 0, 0, 0]], // Release mt!(B, LShift)
            ]
        };
    }

    #[test]
    fn test_mt_2() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 0, true, 10], // Press A
                [0, 1, false, 10], // Release mt!(B, LShift)
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Permissive hold
                [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Release mt!(B, LShift)
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_mt_3() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 0, true, 10], // Press A
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 0, false, 10], // Release A
                [0, 1, false, 10], // Release mt!(B, LShift)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Press B
                [0, [0, 0, 0, 0, 0, 0]], // Release B
            ]
        };
    }

    #[test]
    fn test_mt_4() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 0, true, 10], // Press A
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 1, false, 10], // Release mt!(B, LShift)
                [0, 0, false, 10], // Release A
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
    fn test_mt_5() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 0, true, 10], // Press A
                [0, 0, false, 10], // Release A
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 1, false, 10], // Release mt!(B, LShift)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Press B
                [0, [0, 0, 0, 0, 0, 0]], // Release B
            ]
        };
    }

    #[test]
    fn test_mt_6() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 1, false, 10], // Release mt!(B, LShift)
                [0, 0, true, 10], // Press A
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Press B
                [0, [0, 0, 0, 0, 0, 0]], // Release B
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_mt_timeout_1() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 0, true, 10], // Press A
                [0, 0, false, 260], // Release A
                [0, 1, false, 10], // Release mt!(B, LShift)
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Timeout
                [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Release A
                [0, [0, 0, 0, 0, 0, 0]], // Release mt!(B, LShift)
            ]
        };
    }

    #[test]
    fn test_mt_timeout_2() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 0, true, 10], // Press A
                [0, 1, false, 260], // Release mt!(B, LShift)
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Timeout
                [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Release mt!(B, LShift)
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_mt_timeout_3() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 0, true, 10], // Press A
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 0, false, 260], // Release A
                [0, 1, false, 10], // Release mt!(B, LShift)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Timeout
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Release A
                [0, [0, 0, 0, 0, 0, 0]], // Release mt!(B, LShift)
            ]
        };
    }

    #[test]
    fn test_mt_timeout_4() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 0, true, 10], // Press A
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 1, false, 260], // Release mt!(B, LShift)
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Timeout
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Release mt!(B, LShift)
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_mt_timeout_5() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 0, true, 10], // Press A
                [0, 0, false, 10], // Release A
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 1, false, 260], // Release mt!(B, LShift)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Press mt!(B, LShift)
                [0, [0, 0, 0, 0, 0, 0]], // Release mt!(B, LShift)
            ]
        };
    }

    #[test]
    fn test_mt_timeout_6() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 1, false, 260], // Release mt!(B, LShift)
                [0, 0, true, 10], // Press A
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Press mt!(B, LShift)
                [0, [0, 0, 0, 0, 0, 0]], // Release mt!(B, LShift)
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_mt_timeout_7() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 0, true, 10], // Press A
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 0, false, 10], // Release A
                [0, 1, false, 260], // Release mt!(B, LShift)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Timeout
                [0, [0, 0, 0, 0, 0, 0]], // Release mt!(B, LShift)
            ]
        };
    }

    #[test]
    fn test_mt_timeout_8() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 0, true, 10], // Press A
                [0, 0, false, 10], // Release A
                [0, 1, false, 260], // Release mt!(B, LShift)
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Permissve hold
                [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Release A
                [0, [0, 0, 0, 0, 0, 0]], // Release mt!(B, LShift)
            ]
        };
    }

    #[test]
    fn test_mt_timeout_9() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 0, true, 260], // Press A
                [0, 0, false, 10], // Release A
                [0, 1, false, 10], // Release mt!(B, LShift)
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Timeout
                [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Release A
                [0, [0, 0, 0, 0, 0, 0]], // Release mt!(B, LShift)
            ]
        };
    }

    #[test]
    fn test_mt_timeout_10() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 0, true, 260], // Press A
                [0, 1, false, 10], // Release mt!(B, LShift)
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Timeout
                [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Release mt!(B, LShift)
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_trigger() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
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
    fn test_with_combo_1() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard_with_combo(),
            sequence: [
                [0, 1, true, 20],  // Press mt!(B, LShift)
                [0, 2, true, 60],  // Press mt!(C, LGui)
                [0, 2, false, 10], // Release C
                [0, 1, false, 300], // Release B
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                [KC_LSHIFT, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_with_combo_2() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard_with_combo(),
            sequence: [
                [0, 1, true, 20],  // Press mt!(B, LShift)
                [0, 2, true, 20],  // Press mt!(C, LGui)
                [0, 2, false, 10], // Release C
                [0, 1, false, 300], // Release B
            ],
            expected_reports: [
                [0, [kc_to_u8!(X), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_with_combo_3() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard_with_combo(),
            sequence: [
                [0, 1, true, 20],  // Press mt!(B, LShift)
                [0, 2, true, 20],  // Press mt!(C, LGui)
                [0, 1, false, 20], // Release B
                [0, 2, false, 10], // Release C
            ],
            expected_reports: [
                [0, [kc_to_u8!(X), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_with_combo_4() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard_with_combo(),
            sequence: [
                [0, 1, true, 20],  // Press mt!(B, LShift)
                [0, 2, true, 60],  // Press mt!(C, LGui) -> Resolve B, note that mt!(C, LGui) is not resolved yet
                [0, 1, false, 20], // Release B
                [0, 2, false, 10], // Release C -> mt!(C, LGui) is resolved now
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_with_combo_5() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard_with_combo(),
            sequence: [
                [0, 1, true, 20],  // Press mt!(B, LShift)
                [0, 2, true, 20],  // Press mt!(C, LGui)
                [0, 1, false, 260], // Release B
                [0, 2, false, 260], // Release C
            ],
            expected_reports: [
                [0, [kc_to_u8!(X), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_with_combo_6() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard_with_combo(),
            sequence: [
                [0, 1, true, 20],  // Press mt!(B, LShift)
                [0, 3, true, 20],  // Press lt!(1, D)
                [0, 2, true, 60],  // Press mt!(C, LGui) -> Kp3 on layer 1
                [0, 1, false, 20], // Release B
                [0, 3, false, 10], // Release D
                [0, 2, false, 10], // Release C
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                [KC_LSHIFT, [kc_to_u8!(Kp3), 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(Kp3), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_with_combo_7() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard_with_combo(),
            sequence: [
                [0, 1, true, 20],  // Press mt!(B, LShift)
                [0, 3, true, 20],  // Press lt!(1, D)
                [0, 2, true, 20],  // Press mt!(C, LGui)
                [0, 1, false, 20], // Release B
                [0, 2, false, 10], // Release C
                [0, 3, false, 10], // Release D
            ],
            expected_reports: [
                [0, [kc_to_u8!(Z), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_with_combo_8() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard_with_combo(),
            sequence: [
                [0, 1, true, 20],  // Press mt!(B, LShift)
                [0, 3, true, 20],  // Press lt!(1, D)
                [0, 2, true, 60],  // Press mt!(C, LGui)
                [0, 1, false, 20], // Release B
                [0, 2, false, 10], // Release C
                [0, 3, false, 10], // Release D
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                [KC_LSHIFT, [kc_to_u8!(Kp3), 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(Kp3), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_timeout() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
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
    fn test_quick_tap() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 0, true, 10],  // Press A
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 1, false, 100], // Release mt!(B, LShift)
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
    fn test_multi_tap() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 0, true, 10],  // Press A
                [0, 0, false, 100], // Release A
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 2, true, 60], // Press mt!(C, LGui)
                [0, 1, false, 60], // Release mt!(B, LShift)
                [0, 2, false, 60], // Release mt!(C, LGui)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // mt!(B, LShift)
                [0, [0, 0, 0, 0, 0, 0]], // Release C
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]], // mt!(B, LShift)
                [0, [0, 0, 0, 0, 0, 0]], // Release C
            ]
        };
    }

    #[test]
    fn test_layer_tap() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
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
    fn test_rolling_with_layer_tap() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 3, true, 10], // Press lt!(1, D)
                [0, 0, true, 10], // Press A -> Kp1 on layer 1
                [0, 3, false, 10], // Release lt!(1, D)
                [0, 0, false, 10], // Release A
                [0, 3, true, 250], // Press lt!(1, D)
                [0, 0, true, 10], // Press A Press A -> Kp1 on layer 1
                [0, 0, false, 10], // Release A
                [0, 3, false, 100], // Release lt!(1, D)
                [0, 3, true, 250], // Press lt!(1, D)
                [0, 0, true, 10], // Press A Press A -> Kp1 on layer 1
                [0, 3, false, 100], // Release lt!(1, D)
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0]], // Kp1 on layer 1
                [0, [0, 0, 0, 0, 0, 0]], // Release Kp1
                [0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0]], // Kp1 on layer 1
                [0, [0, 0, 0, 0, 0, 0]], // Release Kp1
                [0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0]], // Kp1 on layer 1
                [0, [0, 0, 0, 0, 0, 0]], // Release Kp1
            ]
        }
    }

    #[test]
    fn test_timeout_rolled_release() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
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
    fn test_timeout_rolled_release_2() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
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
    fn test_timeout_and_release() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 0, true, 20],  // Press A
                [0, 0, false, 260], // Release A
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
    fn test_timeout_and_release_with_other_morse_key() {
    key_sequence_test! {
        keyboard: create_hold_on_other_key_press_keyboard(),
        sequence: [
            [0, 1, true, 10], // Press mt!(B, LShift)
            [0, 2, true, 200],  // Press mt!(C, LGui)
            [0, 2, false, 100], // Release C
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
    fn test_rolling_release_order() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 2, true, 30], // Press mt!(C, LGui)
                [0, 0, true, 30], // Press A -> Triggers mt!(B, LShift) and mt!(C, LGui)
                [0, 1, false, 50], // Release mt!(B, LShift)
                [0, 2, false, 100], // Release mt!(C, LGui)
                [0, 0, false, 100],  // Release A
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]],
                [KC_LSHIFT | KC_LGUI, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [KC_LGUI, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]]
            ]
        };
    }

    #[test]
    fn test_rolling_release_order_2() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 2, true, 30], // Press mt!(C, LGui)
                [0, 0, true, 30], // Press A -> Triggers mt!(B, LShift) and mt!(C, LGui)
                [0, 2, false, 100], // Release C
                [0, 1, false, 50], // Release B
                [0, 0, false, 100],  // Release A
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]],
                [KC_LSHIFT | KC_LGUI, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]]
            ]
        };
    }

    #[test]
    fn test_rolling_release_order_3() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 2, true, 30], // Press mt!(C, LGui)
                [0, 0, true, 30], // Press A -> Triggers mt!(B, LShift) and mt!(C, LGui)
                [0, 2, false, 100], // Release C
                [0, 0, false, 100],  // Release A
                [0, 1, false, 50], // Release B
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]],
                [KC_LSHIFT | KC_LGUI, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]]
            ]
        };
    }


    #[test]
    fn test_multiple_mt_triggered() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 2, true, 30], // Press mt!(C, LGui)
                [0, 0, true, 30], // Press A -> Triggers mt!(B, LShift) and mt!(C, LGui)
                [0, 0, false, 100], // Release A
                [0, 1, false, 50], // Release B
                [0, 2, false, 100], // Release C
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]],
                [KC_LSHIFT | KC_LGUI, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]],
                [KC_LGUI, [0, 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]]
            ]
        };
    }

    #[test]
    fn test_complex_rolling() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
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
                [KC_LSHIFT, [kc_to_u8!(Kp3), 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(Kp3), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_flow_tap() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 0, true, 30],  // Press A
                [0, 0, false, 30], // Release A
                [0, 1, true, 20],  // Press mt!(B, LShift)
                [0, 2, true, 10],  // Press mt!(C, LGui)
                [0, 1, false, 40], // Release B
                [0, 2, false, 10], // Release C
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Press B
                [0, [0, 0, 0, 0, 0, 0]], // Release B
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]], // Press C
                [0, [0, 0, 0, 0, 0, 0]], // Release C
            ]
        };
    }

    // Ref: https://github.com/HaoboGu/rmk/pull/496
    #[test]
    fn test_previous_rolling_keypress() {
        key_sequence_test! {
            keyboard: create_hold_on_other_key_press_keyboard(),
            sequence: [
                [0, 0, true, 30],  // Press A
                [0, 3, true, 20],  // Press lt!(1, D)
                [0, 0, false, 30], // Release A
                [0, 1, true, 20], // Press Kp2 on layer 1
                [0, 1, false, 40], // Release Kp2 on layer 1
                [0, 3, false, 10], // Release lt!(1, D)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
                [0, [kc_to_u8!(Kp2), 0, 0, 0, 0, 0]], // Press Kp2
                [0, [0, 0, 0, 0, 0, 0]], // Release Kp2
            ]
        };
    }
}
