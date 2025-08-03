/// Test cases for home row mod(HRM)
///
/// For HRM, `enable_hrm` and `unilateral_tap` is enabled, `prior-idle-time` will be considered.
pub mod common;

use embassy_time::Duration;
use rmk::action::{Action, KeyAction};
use rmk::combo::Combo;
use rmk::config::{BehaviorConfig, CombosConfig, MorseConfig};
use rmk::k;
use rmk::keyboard::Keyboard;
use rmk::keycode::{KeyCode, ModifierCombination};
use rmk::morse::{Morse, MorseKeyMode};
use rusty_fork::rusty_fork_test;

use crate::common::morse::create_simple_morse_keyboard;
use crate::common::{KC_LGUI, KC_LSHIFT};

fn create_hrm_keyboard() -> Keyboard<'static, 1, 4, 2> {
    create_simple_morse_keyboard(BehaviorConfig {
        morse: MorseConfig {
            enable_hrm: true,
            mode: MorseKeyMode::PermissiveHold,
            unilateral_tap: true,
            ..MorseConfig::default()
        },
        ..BehaviorConfig::default()
    })
}

fn create_hrm_keyboard_with_combo() -> Keyboard<'static, 1, 4, 2> {
    let combo_key = KeyAction::Morse(Morse::new_hrm(Action::Key(KeyCode::B), ModifierCombination::SHIFT));
    let combo_key_2 = KeyAction::Morse(Morse::new_hrm(Action::Key(KeyCode::C), ModifierCombination::GUI));
    let combo_key_3 = KeyAction::Morse(Morse::new_tap_hold_with_config(
        Action::Key(KeyCode::D),
        Action::LayerOn(1),
        250,
        MorseKeyMode::PermissiveHold,
        true,
    ));
    create_simple_morse_keyboard(BehaviorConfig {
        morse: MorseConfig {
            enable_hrm: true,
            mode: MorseKeyMode::PermissiveHold,
            unilateral_tap: true,
            ..MorseConfig::default()
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
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150],  // Press mt!(B, LShift)
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
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150],  // Press mt!(B, LShift)
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
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
                [0, 0, true, 10], // Press A -> unilateral tap
                [0, 0, false, 10], // Release A
                [0, 1, false, 10], // Release mt!(B, LShift)
            ],
            expected_reports: [
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Unilateral tap
                [0, [kc_to_u8!(B), kc_to_u8!(A), 0, 0, 0, 0]], // Press A
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Release A
                [0, [0, 0, 0, 0, 0, 0]], // Release mt!(B, LShift)
            ]
        };
    }

    #[test]
    fn test_mt_1_1() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
                [0, 3, true, 10], // Press lt!(1, D) -> Flow tap won't be triggered because the previous morse key is not resolved yet.
                [0, 3, false, 10], // Release lt!(1, D) -> Permissive hold triggered
                [0, 1, false, 10], // Release mt!(B, LShift)
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Permissive hold
                [KC_LSHIFT, [kc_to_u8!(D), 0, 0, 0, 0, 0]], // Press D
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Release D
                [0, [0, 0, 0, 0, 0, 0]], // Release mt!(B, LShift)
            ]
        };
    }

    #[test]
    fn test_mt_2() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
                [0, 0, true, 10], // Press A -> Unilateral tap
                [0, 1, false, 10], // Release mt!(B, LShift)
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Press B
                [0, [kc_to_u8!(B), kc_to_u8!(A), 0, 0, 0, 0]], // Press A
                [0, [0, kc_to_u8!(A), 0, 0, 0, 0]], // Release B
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_mt_2_1() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
                [0, 3, true, 10], // Press lt!(1, D)
                [0, 1, false, 10], // Release mt!(B, LShift)
                [0, 3, false, 10], // Release lt!(1, D)
            ],
            expected_reports: [
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Press B
                [0, [0, 0, 0, 0, 0, 0]], // Release B
                [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]], // Press D
                [0, [0, 0, 0, 0, 0, 0]], // Release D
            ]
        };
    }

    #[test]
    fn test_mt_2_2() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 2, true, 150], // Press mt!(C, LGui)
                [0, 3, true, 10], // Press lt!(1, D)
                [0, 2, false, 10], // Release mt!(C, LGui)
                [0, 3, false, 10], // Release lt!(1, D)
            ],
            expected_reports: [
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]], // Press C
                [0, [0, 0, 0, 0, 0, 0]], // Release C
                [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]], // Press D
                [0, [0, 0, 0, 0, 0, 0]], // Release D
            ]
        };
    }

    #[test]
    fn test_mt_2_3() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 2, true, 150], // Press mt!(C, LGui)
                [0, 3, true, 10], // Press lt!(1, D) -> Unilateral tap
                [0, 3, false, 10], // Release lt!(1, D)
                [0, 2, false, 10], // Release mt!(C, LGui)
            ],
            expected_reports: [
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]], // Press C
                [0, [kc_to_u8!(C), kc_to_u8!(D), 0, 0, 0, 0]], // Press D
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]], // Release D
                [0, [0, 0, 0, 0, 0, 0]], // Release C
            ]
        };
    }

    #[test]
    fn test_mt_3() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press A
                [0, 1, true, 10], // Press mt!(B, LShift) -> Flow Tap
                [0, 0, false, 10], // Release A
                [0, 1, false, 10], // Release mt!(B, LShift)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [kc_to_u8!(A), kc_to_u8!(B), 0, 0, 0, 0]], // Press B
                [0, [0, kc_to_u8!(B), 0, 0, 0, 0]], // Release A
                [0, [0, 0, 0, 0, 0, 0]], // Release B
            ]
        };
    }

    #[test]
    fn test_mt_4() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press A
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
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press A
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
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
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
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
                [0, 0, true, 10], // Press A
                [0, 0, false, 260], // Release A -> Timeout
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
    fn test_mt_timeout_1_1() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 2, true, 150], // Press mt!(C, LGui)
                [0, 3, true, 10], // Press lt!(1, D)
                [0, 3, false, 260], // Release lt!(1, D)
                [0, 2, false, 10], // Release mt!(C, LGui)
            ],
            expected_reports: [
                [KC_LGUI, [0, 0, 0, 0, 0, 0]], // Timeout
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_mt_timeout_1_2() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 2, true, 150], // Press mt!(C, LGui)
                [0, 3, true, 10], // Press lt!(1, D)
                [0, 3, false, 10], // Release lt!(1, D) -> Unilateral tap
                [0, 2, false, 260], // Release mt!(C, LGui)
            ],
            expected_reports: [
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]], // Press C
                [0, [kc_to_u8!(C), kc_to_u8!(D), 0, 0, 0, 0]], // Press D
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]], // Release D
                [0, [0, 0, 0, 0, 0, 0]], // Release C
            ]
        };
    }

    #[test]
    fn test_mt_timeout_2() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
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
    fn test_mt_timeout_2_1() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 2, true, 150], // Press mt!(C, LGui)
                [0, 3, true, 10], // Press lt!(1, D)
                [0, 2, false, 260], // Release mt!(C, LGui)
                [0, 3, false, 10], // Release lt!(1, D)
            ],
            expected_reports: [
                [KC_LGUI, [0, 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_mt_timeout_3() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press A
                [0, 1, true, 10], // Press mt!(B, LShift) -> Flow Tap
                [0, 0, false, 260], // Release A
                [0, 1, false, 10], // Release mt!(B, LShift)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [kc_to_u8!(A), kc_to_u8!(B), 0, 0, 0, 0]], // Press B
                [0, [0, kc_to_u8!(B), 0, 0, 0, 0]], // Release A
                [0, [0, 0, 0, 0, 0, 0]], // Release B
            ]
        };
    }

    #[test]
    fn test_mt_timeout_4() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press A
                [0, 1, true, 10], // Press mt!(B, LShift) -> Flow Tap
                [0, 1, false, 260], // Release mt!(B, LShift)
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
    fn test_mt_timeout_5() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press A
                [0, 0, false, 10], // Release A
                [0, 1, true, 10], // Press mt!(B, LShift) -> Flow Tap
                [0, 1, false, 260], // Release mt!(B, LShift)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Press mt!(B, LShift)
                [0, [0, 0, 0, 0, 0, 0]], // Release mt!(B, LShift)
            ]
        };
    }

    #[test]
    fn test_mt_timeout_6() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
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
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press A
                [0, 1, true, 10], // Press mt!(B, LShift) -> Flow Tap
                [0, 0, false, 10], // Release A
                [0, 1, false, 260], // Release mt!(B, LShift)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [kc_to_u8!(A), kc_to_u8!(B), 0, 0, 0, 0]], // Press B
                [0, [0, kc_to_u8!(B), 0, 0, 0, 0]], // Release A
                [0, [0, 0, 0, 0, 0, 0]], // Release mt!(B, LShift)
            ]
        };
    }

    #[test]
    fn test_mt_timeout_8() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
                [0, 0, true, 10], // Press A -> Unilateral tap
                [0, 0, false, 10], // Release A
                [0, 1, false, 260], // Release mt!(B, LShift)
            ],
            expected_reports: [
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Unilateral tap
                [0, [kc_to_u8!(B), kc_to_u8!(A), 0, 0, 0, 0]], // Press A
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Release A
                [0, [0, 0, 0, 0, 0, 0]], // Release mt!(B, LShift)
            ]
        };
    }

    #[test]
    fn test_mt_timeout_8_1() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 2, true, 150], // Press mt!(C, LGui)
                [0, 0, true, 10], // Press A
                [0, 0, false, 10], // Release A
                [0, 2, false, 260], // Release mt!(C, LGui)
            ],
            expected_reports: [
                [KC_LGUI, [0, 0, 0, 0, 0, 0]],  // Permissive hold
                [KC_LGUI, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [KC_LGUI, [0, 0, 0, 0, 0, 0]], // Release A
                [0, [0, 0, 0, 0, 0, 0]], // Release mt!(C, LGui)
            ]
        };
    }

    #[test]
    fn test_mt_timeout_9() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
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
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
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
    fn test_morse_lt_1() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 3, true, 150], // Press lt!(1, D)
                [0, 0, true, 10], // Press A
                [0, 0, false, 10], // Release A
                [0, 3, false, 10], // Release lt!(1, D)
            ],
            expected_reports: [
                [0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0]], // Press Kp1
                [0, [0, 0, 0, 0, 0, 0]], // Release Kp1
            ]
        };
    }

    #[test]
    fn test_morse_lt_2() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 3, true, 150], // Press lt!(1, D)
                [0, 0, true, 10], // Press A
                [0, 3, false, 10], // Release lt!(1, D)
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]], // Press D
                [0, [kc_to_u8!(D), kc_to_u8!(A), 0, 0, 0, 0]], // Press A
                [0, [0, kc_to_u8!(A), 0, 0, 0, 0]], // Release D
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_morse_lt_3() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press A
                [0, 3, true, 10], // Press lt!(1, D) -> Flow Tap
                [0, 0, false, 10], // Release A
                [0, 3, false, 10], // Release lt!(1, D)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [kc_to_u8!(A), kc_to_u8!(D), 0, 0, 0, 0]], // Press D
                [0, [0, kc_to_u8!(D), 0, 0, 0, 0]], // Release A
                [0, [0, 0, 0, 0, 0, 0]], // Release D
            ]
        };
    }

    #[test]
    fn test_morse_lt_4() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press A
                [0, 3, true, 10], // Press lt!(1, D) -> Flow Tap
                [0, 3, false, 10], // Release lt!(1, D)
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [kc_to_u8!(A), kc_to_u8!(D), 0, 0, 0, 0]], // Press D
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Release D
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_morse_lt_5() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press A
                [0, 0, false, 10], // Release A
                [0, 3, true, 10], // Press lt!(1, D) -> Flow Tap
                [0, 3, false, 10], // Release lt!(1, D)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
                [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]], // Press D
                [0, [0, 0, 0, 0, 0, 0]], // Release D
            ]
        };
    }

    #[test]
    fn test_morse_lt_6() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 3, true, 150], // Press lt!(1, D)
                [0, 3, false, 10], // Release lt!(1, D)
                [0, 0, true, 10], // Press A
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]], // Press D
                [0, [0, 0, 0, 0, 0, 0]], // Release D
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_morse_lt_timeout_1() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 3, true, 150], // Press lt!(1, D)
                [0, 0, true, 10], // Press A
                [0, 0, false, 260], // Release A -> timeout, trigger Kp1 on layer 1
                [0, 3, false, 10], // Release lt!(1, D)
            ],
            expected_reports: [
                [0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0]], // Press Kp1
                [0, [0, 0, 0, 0, 0, 0]], // Release Kp1
            ]
        };
    }

    #[test]
    fn test_morse_lt_timeout_2() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 3, true, 150], // Press lt!(1, D)
                [0, 0, true, 10], // Press A
                [0, 3, false, 260], // Release lt!(1, D) -> timeout, trigger Kp1 on layer 1
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0]], // Press Kp1
                [0, [0, 0, 0, 0, 0, 0]], // Release Kp1
            ]
        };
    }

    #[test]
    fn test_morse_lt_timeout_3() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press A
                [0, 3, true, 10], // Press lt!(1, D) -> Flow Tap
                [0, 0, false, 260], // Release A
                [0, 3, false, 10], // Release lt!(1, D)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [kc_to_u8!(A), kc_to_u8!(D), 0, 0, 0, 0]], // Press D
                [0, [0, kc_to_u8!(D), 0, 0, 0, 0]], // Release A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_morse_lt_timeout_4() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press A
                [0, 3, true, 10], // Press lt!(1, D) -> Flow Tap
                [0, 3, false, 260], // Release lt!(1, D)
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [kc_to_u8!(A), kc_to_u8!(D), 0, 0, 0, 0]], // Press D
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Release D
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_morse_lt_timeout_5() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press A
                [0, 0, false, 10], // Release A
                [0, 3, true, 10], // Press lt!(1, D) -> Flow tap
                [0, 3, false, 260], // Release lt!(1, D)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
                [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]], // Press D
                [0, [0, 0, 0, 0, 0, 0]], // Release D
            ]
        };
    }

    #[test]
    fn test_morse_lt_timeout_5_1() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press A
                [0, 0, false, 200], // Release A -> Longer than `prior-idle-time`
                [0, 3, true, 10], // Press lt!(1, D)
                [0, 3, false, 260], // Release lt!(1, D)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_morse_lt_timeout_6() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 3, true, 150], // Press lt!(1, D)
                [0, 3, false, 270], // Release lt!(1, D)
                [0, 0, true, 10], // Press A
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_morse_lt_timeout_7() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press A
                [0, 3, true, 10], // Press lt!(1, D) -> Flow Tap
                [0, 0, false, 10], // Release A
                [0, 3, false, 260], // Release lt!(1, D)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [kc_to_u8!(A), kc_to_u8!(D), 0, 0, 0, 0]], // Press D
                [0, [0, kc_to_u8!(D), 0, 0, 0, 0]], // Release A
                [0, [0, 0, 0, 0, 0, 0]], // Release D
            ]
        };
    }

    #[test]
    fn test_morse_lt_timeout_8() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 3, true, 150], // Press lt!(1, D)
                [0, 0, true, 10], // Press A -> permisshive hold: Kp1 on layer 1
                [0, 0, false, 10], // Release A
                [0, 3, false, 260], // Release lt!(1, D)
            ],
            expected_reports: [
                [0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_morse_lt_timeout_9() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 3, true, 150], // Press lt!(1, D)
                [0, 0, true, 260], // Press A -> Kp1 on layer 1
                [0, 0, false, 10], // Release A
                [0, 3, false, 10], // Release lt!(1, D)
            ],
            expected_reports: [
                [0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0]], // Press Kp1 on layer 1
                [0, [0, 0, 0, 0, 0, 0]], // Release Kp1
            ]
        };
    }

    #[test]
    fn test_morse_lt_timeout_10() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 3, true, 150], // Press lt!(1, D)
                [0, 0, true, 260], // Press A -> Kp1 on layer 1
                [0, 3, false, 10], // Release lt!(1, D)
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0]], // Press Kp1 on layer 1
                [0, [0, 0, 0, 0, 0, 0]], // Release Kp1
            ]
        };
    }

    #[test]
    fn test_trigger() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
                [0, 0, true, 50],  // Press A -> Unilateral tap
                [0, 0, false, 10], // Release A
                [0, 1, false, 100], // Release mt!(B, LShift)
            ],
            expected_reports: [
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Press B
                [0, [kc_to_u8!(B), kc_to_u8!(A), 0, 0, 0, 0]], // Press A
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Release A
                [0, [0, 0, 0, 0, 0, 0]], // All released
            ]
        };
    }

    #[test]
    fn test_with_combo_1() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard_with_combo(),
            sequence: [
                [0, 1, true, 200],  // Press mt!(B, LShift)
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
            keyboard: create_hrm_keyboard_with_combo(),
            sequence: [
                [0, 1, true, 200],  // Press mt!(B, LShift)
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
            keyboard: create_hrm_keyboard_with_combo(),
            sequence: [
                [0, 1, true, 200],  // Press mt!(B, LShift)
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
            keyboard: create_hrm_keyboard_with_combo(),
            sequence: [
                [0, 1, true, 200],  // Press mt!(B, LShift)
                [0, 2, true, 60],  // Press mt!(C, LGui)
                [0, 1, false, 20], // Release B
                [0, 2, false, 10], // Release C
            ],
            expected_reports: [
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_with_combo_5() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard_with_combo(),
            sequence: [
                [0, 1, true, 200],  // Press mt!(B, LShift)
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
    fn test_with_combo_6() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard_with_combo(),
            sequence: [
                [0, 1, true, 200],  // Press mt!(B, LShift)
                [0, 3, true, 20],  // Press lt!(1, D)
                [0, 2, true, 60],  // Press mt!(C, LGui)
                [0, 1, false, 20], // Release B
                [0, 3, false, 10], // Release D
                [0, 2, false, 10], // Release C
            ],
            expected_reports: [
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]],
                // [0, [kc_to_u8!(D), kc_to_u8!(C), 0, 0, 0, 0]],
                // [0, [0, kc_to_u8!(C), 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_with_combo_7() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard_with_combo(),
            sequence: [
                [0, 1, true, 200],  // Press mt!(B, LShift)
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
            keyboard: create_hrm_keyboard_with_combo(),
            sequence: [
                [0, 1, true, 200],  // Press mt!(B, LShift)
                [0, 3, true, 20],  // Press lt!(1, D)
                [0, 2, true, 60],  // Press mt!(C, LGui)
                [0, 1, false, 20], // Release B
                [0, 2, false, 10], // Release C  -> Unilateral tap of lt!(1, D) is triggered, before the mt!(B, LShift) is released and triggered
                [0, 3, false, 10], // Release D
            ],
            expected_reports: [
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(D), kc_to_u8!(C), 0, 0, 0, 0]],
                [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_with_combo_8_1() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard_with_combo(),
            sequence: [
                [0, 1, true, 200],  // Press mt!(B, LShift)
                [0, 3, true, 200],  // Press lt!(1, D)
                [0, 2, true, 60],  // Press mt!(C, LGui)
                [0, 1, false, 20], // Release B
                [0, 2, false, 10], // Release C -> Unilateral tap of lt!(1, D) is triggered, before the mt!(B, LShift) is released and triggered
                [0, 3, false, 10], // Release D
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(D), kc_to_u8!(C), 0, 0, 0, 0]],
                [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_timeout() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
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
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150],  // Press A
                [0, 1, true, 10], // Press mt!(B, LShift) -> Flow Tap
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
    fn test_multi_tap() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150],  // Press A
                [0, 0, false, 120], // Release A
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 2, true, 60], // Press mt!(C, LGui)
                [0, 1, false, 60], // Release mt!(B, LShift)
                [0, 2, false, 60], // Release mt!(C, LGui)
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
    fn test_multi_tap_2() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150],  // Press A
                [0, 0, false, 10], // Release A
                [0, 1, true, 10], // Press mt!(B, LShift) -> Flow Tap
                [0, 2, true, 200], // Press mt!(C, LGui)
                [0, 1, false, 60], // Release mt!(B, LShift)
                [0, 2, false, 60], // Release mt!(C, LGui)
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
    fn test_multi_tap_3() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150],  // Press A
                [0, 0, false, 10], // Release A
                [0, 1, true, 10], // Press mt!(B, LShift) -> Flow Tap
                [0, 2, true, 40], // Press mt!(C, LGui) -> Flow Tap
                [0, 1, false, 60], // Release mt!(B, LShift)
                [0, 2, false, 60], // Release mt!(C, LGui)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Press B
                [0, [kc_to_u8!(B), kc_to_u8!(C), 0, 0, 0, 0]], // Press C
                [0, [0, kc_to_u8!(C), 0, 0, 0, 0]], // Release B
                [0, [0, 0, 0, 0, 0, 0]], // Release C
            ]
        };
    }

    #[test]
    fn test_layer_tap() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 3, true, 150], // Press lt!(1, D)
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 1, false, 100], // Release B
                [0, 3, false, 10], // Release lt!(1, D)
                [0, 0, true, 10], // Press A
                [0, 0, false, 10], // Release A
                [0, 3, true, 10], // Press lt!(1, D) -> Flow Tap after A
                [0, 1, true, 50], // Press mt!(B, LShift) -> Flow Tap
                [0, 1, false, 100], // Release B
                [0, 3, false, 10], // Release lt!(1, D)
            ],
            expected_reports: [
                [0, [kc_to_u8!(Kp2), 0, 0, 0, 0, 0]], // Press Kp2 on layer 1
                [0, [0, 0, 0, 0, 0, 0]], // Release Kp2
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
                [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]], // Press D
                [0, [kc_to_u8!(D), kc_to_u8!(B), 0, 0, 0, 0]], // Press B
                [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]], // Release B
                [0, [0, 0, 0, 0, 0, 0]], // Release D
            ]
        };
    }

    #[test]
    fn test_rolling_with_layer_tap() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 3, true, 150], // Press lt!(1, D)
                [0, 0, true, 10], // Press A
                [0, 3, false, 10], // Release lt!(1, D)
                [0, 0, false, 10], // Release A
                [0, 3, true, 250], // Press lt!(1, D)
                [0, 0, true, 10], // Press A
                [0, 0, false, 10], // Release A
                [0, 3, false, 100], // Release lt!(1, D)
                [0, 3, true, 250], // Press lt!(1, D)
                [0, 0, true, 10], // Press A
                [0, 3, false, 100], // Release lt!(1, D)
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]], // D
                [0, [kc_to_u8!(D), kc_to_u8!(A), 0, 0, 0, 0]], // D + A
                [0, [0, kc_to_u8!(A), 0, 0, 0, 0]], // Release D
                [0, [0, 0, 0, 0, 0, 0]], // Release A
                [0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0]], // Kp1 on layer 1
                [0, [0, 0, 0, 0, 0, 0]], // Release Kp1
                [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]], // D
                [0, [kc_to_u8!(D), kc_to_u8!(A), 0, 0, 0, 0]], // D + A
                [0, [0, kc_to_u8!(A), 0, 0, 0, 0]], // Release D
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        }
    }

    #[test]
    fn test_timeout_rolled_release() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
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
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
                [0, 0, true, 10],  // Press A
                [0, 1, false, 300], // Release B
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Timeout B
                [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Release A
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_timeout_and_release() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
                [0, 0, true, 20],  // Press A
                [0, 0, false, 260], // Release A
                [0, 1, false, 100], // Release B
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]], // All released
            ]
        };
    }

    #[test]
    fn test_timeout_and_release_with_other_morse_key() {
    key_sequence_test! {
        keyboard: create_hrm_keyboard(),
        sequence: [
            [0, 1, true, 150], // Press mt!(B, LShift)
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
    fn test_rolling_release_order() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
                [0, 2, true, 30], // Press mt!(C, LGui)
                [0, 0, true, 30], // Press A
                [0, 1, false, 50], // Release mt!(B, LShift)
                [0, 2, false, 100], // Release mt!(C, LGui)
                [0, 0, false, 100],  // Release A
            ],
            expected_reports: [
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // FIXME: Maybe B-C-A is the expected order
                [0, [kc_to_u8!(B), kc_to_u8!(A), 0, 0, 0, 0]],
                [0, [0, kc_to_u8!(A), 0, 0, 0, 0]],
                [0, [kc_to_u8!(C), kc_to_u8!(A), 0, 0, 0, 0]],
                [0, [0, kc_to_u8!(A), 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_rolling_release_order_2() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
                [0, 2, true, 30], // Press mt!(C, LGui)
                [0, 0, true, 30], // Press A
                [0, 2, false, 100], // Release C -> Permissive hold for mt!(B, LShift)
                [0, 1, false, 50], // Release B
                [0, 0, false, 100],  // Release A
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                [KC_LSHIFT, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                [KC_LSHIFT, [kc_to_u8!(C), kc_to_u8!(A), 0, 0, 0, 0]],
                [KC_LSHIFT, [0, kc_to_u8!(A), 0, 0, 0, 0]],
                [0, [0, kc_to_u8!(A), 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_rolling_release_order_3() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
                [0, 2, true, 30], // Press mt!(C, LGui)
                [0, 0, true, 30], // Press A
                [0, 2, false, 100], // Release C -> Permissive hold for mt!(B, LShift)
                [0, 0, false, 100],  // Release A
                [0, 1, false, 50], // Release B
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                [KC_LSHIFT, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                [KC_LSHIFT, [kc_to_u8!(C), kc_to_u8!(A), 0, 0, 0, 0]],
                [KC_LSHIFT, [0, kc_to_u8!(A), 0, 0, 0, 0]],
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }


    #[test]
    fn test_multiple_permissive_hold() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 1, true, 150], // Press mt!(B, LShift)
                [0, 2, true, 30], // Press mt!(C, LGui)
                [0, 0, true, 30], // Press A -> Unilateral tap for mt!(B, LShift)
                [0, 0, false, 100], // Release A -> Permissive hold triggered of mt!(C, LGui)
                [0, 1, false, 50], // Release B
                [0, 2, false, 100], // Release C
            ],
            expected_reports: [
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]],
                [KC_LGUI, [kc_to_u8!(B), 0, 0, 0, 0, 0]],
                [KC_LGUI, [kc_to_u8!(B), kc_to_u8!(A), 0, 0, 0, 0]],
                [KC_LGUI, [kc_to_u8!(B), 0, 0, 0, 0, 0]],
                [KC_LGUI, [0, 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_complex_rolling() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 160], // Press A
                [0, 1, true, 10], // Press mt!(B, LShift) -> Flow Tap
                [0, 0, false, 10], // Release A
                [0, 3, true, 30], // Press lt!(1, D) -> Flow Tap
                [0, 2, true, 30], // Press mt!(C, LGui) -> Flow Tap
                [0, 3, false, 100], // Release D
                [0, 1, false, 50], // Release B
                [0, 2, false, 10], // Release C
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(A), kc_to_u8!(B), 0, 0, 0, 0]],
                [0, [0, kc_to_u8!(B), 0, 0, 0, 0]],
                [0, [kc_to_u8!(D), kc_to_u8!(B), 0, 0, 0, 0]],
                [0, [kc_to_u8!(D), kc_to_u8!(B), kc_to_u8!(C), 0, 0, 0]],
                [0, [0, kc_to_u8!(B), kc_to_u8!(C), 0, 0, 0]],
                [0, [0, 0, kc_to_u8!(C), 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_flow_tap() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150],  // Press A
                [0, 0, false, 30], // Release A
                [0, 1, true, 20],  // Press mt!(B, LShift) -> Flow Tap
                [0, 2, true, 10],  // Press mt!(C, LGui) -> Flow Tap
                [0, 1, false, 40], // Release B
                [0, 2, false, 10], // Release C
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Press B
                [0, [kc_to_u8!(B), kc_to_u8!(C), 0, 0, 0, 0]], // Press C
                [0, [0, kc_to_u8!(C), 0, 0, 0, 0]], // Release B
                [0, [0, 0, 0, 0, 0, 0]], // Release C
            ]
        };
    }

    // Ref: https://github.com/HaoboGu/rmk/pull/496
    #[test]
    fn test_previous_rolling_keypress() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 0, true, 150],  // Press A
                [0, 3, true, 150],  // Press lt!(1, D)
                [0, 0, false, 30], // Release A
                [0, 1, true, 150], // Press Kp2 on layer 1
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

    #[test]
    fn test_multi_hold_cross_hand() {
        key_sequence_test! {
            keyboard: create_hrm_keyboard(),
            sequence: [
                [0, 2, true, 150], // Press mt!(C, LGui)
                [0, 3, true, 150], // Press lt!(1, D)
                [0, 0, true, 10], // Press A
                [0, 0, false, 10], // Release A -> Permisive hold
                [0, 2, false, 40], // Release Kp2 on layer 1
                [0, 3, false, 10], // Release lt!(1, D)
            ],
            expected_reports: [
                [KC_LGUI, [0, 0, 0, 0, 0, 0]],
                [KC_LGUI, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0]],
                [KC_LGUI, [0, 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }
}
