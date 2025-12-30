pub mod common;

use embassy_time::Duration;
use rmk::combo::{Combo, ComboConfig};
use rmk::config::{BehaviorConfig, CombosConfig};
use rmk::k;
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};
use rmk::types::modifier::ModifierCombination;
use rusty_fork::rusty_fork_test;

use crate::common::morse::create_simple_morse_keyboard;
use crate::common::{KC_LGUI, KC_LSHIFT};

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
    fn test_morse_mt_1() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 0, true, 10], // Press A
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
    fn test_morse_mt_2() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 0, true, 10], // Press A
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
    fn test_morse_mt_3() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
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
    fn test_morse_mt_4() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
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
    fn test_morse_mt_5() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
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
    fn test_morse_mt_6() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
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
    fn test_morse_mt_timeout_1() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 0, true, 10], // Press A
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
    fn test_morse_mt_timeout_2() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 0, true, 10], // Press A
                [0, 1, false, 260], // Release mt!(B, LShift)
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press mt!(B, LShift)
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Release mt!(B, LShift)
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_morse_mt_timeout_3() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
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
    fn test_morse_mt_timeout_4() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
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
    fn test_morse_mt_timeout_5() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
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
    fn test_morse_mt_timeout_6() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 1, false, 270], // Release mt!(B, LShift)
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
    fn test_morse_mt_timeout_7() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
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
    fn test_morse_mt_timeout_8() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 1, true, 10], // Press mt!(B, LShift)
                [0, 0, true, 10], // Press A
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
    fn test_morse_mt_timeout_9() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
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
    fn test_morse_mt_timeout_10() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
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
    fn test_morse_lt_1() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 3, true, 10], // Press lt!(1, D)
                [0, 0, true, 10], // Press A
                [0, 0, false, 10], // Release A
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
    fn test_morse_lt_2() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 3, true, 10], // Press lt!(1, D)
                [0, 0, true, 10], // Press A
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
    fn test_morse_lt_3() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 0, true, 10], // Press A
                [0, 3, true, 10], // Press lt!(1, D)
                [0, 0, false, 10], // Release A
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
    fn test_morse_lt_4() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 0, true, 10], // Press A
                [0, 3, true, 10], // Press lt!(1, D)
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
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 0, true, 10], // Press A
                [0, 0, false, 10], // Release A
                [0, 3, true, 10], // Press lt!(1, D)
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
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 3, true, 10], // Press lt!(1, D)
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
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 3, true, 10], // Press lt!(1, D)
                [0, 0, true, 10], // Press A
                [0, 0, false, 260], // Release A
                [0, 3, false, 10], // Release lt!(1, D)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_morse_lt_timeout_2() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 3, true, 10], // Press lt!(1, D)
                [0, 0, true, 10], // Press A
                [0, 3, false, 260], // Release lt!(1, D)
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_morse_lt_timeout_3() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 0, true, 10], // Press A
                [0, 3, true, 10], // Press lt!(1, D)
                [0, 0, false, 260], // Release A
                [0, 3, false, 10], // Release lt!(1, D)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_morse_lt_timeout_4() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 0, true, 10], // Press A
                [0, 3, true, 10], // Press lt!(1, D)
                [0, 3, false, 260], // Release lt!(1, D)
                [0, 0, false, 10], // Release A
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_morse_lt_timeout_5() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 0, true, 10], // Press A
                [0, 0, false, 10], // Release A
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
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 3, true, 10], // Press lt!(1, D)
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
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 0, true, 10], // Press A
                [0, 3, true, 10], // Press lt!(1, D)
                [0, 0, false, 10], // Release A
                [0, 3, false, 260], // Release lt!(1, D)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_morse_lt_timeout_8() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 3, true, 10], // Press lt!(1, D)
                [0, 0, true, 10], // Press A
                [0, 0, false, 10], // Release A
                [0, 3, false, 260], // Release lt!(1, D)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Press A
                [0, [0, 0, 0, 0, 0, 0]], // Release A
            ]
        };
    }

    #[test]
    fn test_morse_lt_timeout_9() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 3, true, 10], // Press lt!(1, D)
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
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                [0, 3, true, 10], // Press lt!(1, D)
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
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Press B
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
                [0, 2, false, 150], // Release C (timeout)
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

    #[test]
    fn test_morse_with_combo() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig {
                    combo: CombosConfig {
                        combos: [
                            Some(Combo::new(ComboConfig::new(
                                [KeyAction::TapHold(Action::Key(KeyCode::Hid(HidKeyCode::B)), Action::Modifier(ModifierCombination::LSHIFT), Default::default()),
                                 KeyAction::TapHold(Action::Key(KeyCode::Hid(HidKeyCode::C)), Action::Modifier(ModifierCombination::LGUI), Default::default())],
                                k!(X),
                                None,
                            ))), None, None, None, None, None, None, None
                        ],
                        timeout: Duration::from_millis(50),
                    },
                    ..BehaviorConfig::default()
                }),
            sequence: [
                [0, 1, true, 20],  // Press mt!(B, LShift)
                [0, 2, true, 60],  // Press mt!(C, LGui)
                [0, 2, false, 10], // Release C
                [0, 1, false, 300], // Release B
            ],
            expected_reports: [
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_morse_with_combo_2() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig {
                    combo: CombosConfig {
                        combos: [
                            Some(Combo::new(ComboConfig::new(
                                [KeyAction::TapHold(Action::Key(KeyCode::Hid(HidKeyCode::B)), Action::Modifier(ModifierCombination::LSHIFT), Default::default()),
                                 KeyAction::TapHold(Action::Key(KeyCode::Hid(HidKeyCode::C)), Action::Modifier(ModifierCombination::LGUI), Default::default())],
                                k!(X),
                                None,
                            ))), None, None, None, None, None, None, None
                        ],
                        timeout: Duration::from_millis(50),
                    },
                    ..BehaviorConfig::default()
                }),
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
    fn test_morse_abc_c() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                //C
                [0, 4, true, 300],
                [0, 4, false, 300], //-
                [0, 4, true, 80],
                [0, 4, false, 80], //.
                [0, 4, true, 80],
                [0, 4, false, 300], //-
                [0, 4, true, 80],
                [0, 4, false, 80], //.
            ],
            expected_reports: [
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_morse_abc_s_o_s() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                //S
                [0, 4, true, 300],
                [0, 4, false, 10], //.
                [0, 4, true, 10],
                [0, 4, false, 10], //.
                [0, 4, true, 10],
                [0, 4, false, 10], //.

                //O
                [0, 4, true, 300],
                [0, 4, false, 300], //-
                [0, 4, true, 10],
                [0, 4, false, 300], //-
                [0, 4, true, 10],
                [0, 4, false, 300], //-

                //S
                [0, 4, true, 300],
                [0, 4, false, 10], //.
                [0, 4, true, 10],
                [0, 4, false, 10], //.
                [0, 4, true, 10],
                [0, 4, false, 10], //.
            ],
            expected_reports: [
                [0, [kc_to_u8!(S), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(O), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(S), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_morse_rmk() {
        key_sequence_test! {
            keyboard: create_simple_morse_keyboard(BehaviorConfig::default()),
            sequence: [
                //R .-.
                [0, 4, true, 300],
                [0, 4, false, 10], //.
                [0, 4, true, 10],
                [0, 4, false, 300], //-
                [0, 4, true, 10],
                [0, 4, false, 10], //.

                //M --
                [0, 4, true, 300],
                [0, 4, false, 300], //-
                [0, 4, true, 10],
                [0, 4, false, 300], //-

                //K -.-
                [0, 4, true, 300],
                [0, 4, false, 300], //-
                [0, 4, true, 10],
                [0, 4, false, 10], //.
                [0, 4, true, 10],
                [0, 4, false, 300], //-
            ],
            expected_reports: [
                [0, [kc_to_u8!(R), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(M), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(K), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }
}
