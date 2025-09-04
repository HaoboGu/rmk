/// Test cases for tap-dance like morses
pub mod common;

use heapless::Vec;
use rmk::action::Action;
use rmk::config::{BehaviorConfig, MorsesConfig, TapHoldProfile};
use rmk::keyboard::Keyboard;
use rmk::keycode::{KeyCode, ModifierCombination};
use rmk::morse::Morse;
use rmk::{k, td};
use rusty_fork::rusty_fork_test;

use crate::common::wrap_keymap;

pub fn create_tap_dance_test_keyboard() -> Keyboard<'static, 1, 4, 2> {
    let keymap = [
        [[td!(0), td!(1), td!(2), k!(A)]],
        [[k!(Kp1), k!(Kp2), k!(Kp3), k!(Kp4)]],
    ];

    let behavior_config = BehaviorConfig {
        morse: MorsesConfig {
            morses: Vec::from_slice(&[
                Morse::new_from_vial(
                    Action::Key(KeyCode::A),
                    Action::Key(KeyCode::B),
                    Action::Key(KeyCode::C),
                    Action::Key(KeyCode::D),
                    TapHoldProfile::new().with_is_filled(false),
                ),
                Morse::new_from_vial(
                    Action::Key(KeyCode::X),
                    Action::Key(KeyCode::Y),
                    Action::Key(KeyCode::Z),
                    Action::Key(KeyCode::Space),
                    TapHoldProfile::new().with_is_filled(false),
                ),
                Morse::new_from_vial(
                    Action::Key(KeyCode::Kp1),
                    Action::Modifier(ModifierCombination::SHIFT),
                    Action::Key(KeyCode::Kp2),
                    Action::Modifier(ModifierCombination::GUI),
                    TapHoldProfile::new().with_is_filled(false),
                ),
            ])
            .unwrap(),
        },
        tap_hold: rmk::config::TapHoldConfig {
            default_profile: TapHoldProfile::new()
                .with_is_filled(true)
                .with_unilateral_tap(false)
                .with_hold_on_other_press(true)
                .with_hold_timeout_ms(250u16)
                .with_gap_timeout_ms(250u16),
            ..Default::default()
        },
        ..Default::default()
    };

    static BEHAVIOR_CONFIG: static_cell::StaticCell<BehaviorConfig<1, 4>> = static_cell::StaticCell::new();
    let behavior_config = BEHAVIOR_CONFIG.init(behavior_config);
    Keyboard::new(wrap_keymap(keymap, behavior_config))
}

rusty_fork_test! {
    #[test]
    fn test_tap() {
        key_sequence_test! {
            keyboard: create_tap_dance_test_keyboard(),
            sequence: [
                [0, 0, true, 150],  // Press td!(0)
                [0, 0, false, 10], // Release td!(0)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_hold() {
        key_sequence_test! {
            keyboard: create_tap_dance_test_keyboard(),
            sequence: [
                [0, 0, true, 150],  // Press td!(0)
                [0, 0, false, 300], // Release td!(0)
            ],
            expected_reports: [
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_hold_after_tap() {
        key_sequence_test! {
            keyboard: create_tap_dance_test_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press td!(0)
                [0, 0, false, 240], // Release td!(0)
                [0, 0, true, 240], // Press td!(0)
                [0, 0, false, 300], // Release td!(0)
            ],
            expected_reports: [
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_double_tap() {
        key_sequence_test! {
            keyboard: create_tap_dance_test_keyboard(),
            sequence: [
                [0, 0, true, 150],  // Press td!(0)
                [0, 0, false, 200], // Release td!(0)
                [0, 0, true, 200],  // Press td!(0)
                [0, 0, false, 200], // Release td!(0)
            ],
            expected_reports: [
                [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_tap_on_other_press() {
        key_sequence_test! {
            keyboard: create_tap_dance_test_keyboard(),
            sequence: [
                [0, 1, true, 150],  // Press td!(1)
                [0, 1, false, 10], // Release td!(1)
                [0, 3, true, 10], // Press A
                [0, 3, false, 10], // Press A
            ],
            expected_reports: [
                [0, [kc_to_u8!(X), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_hold_on_other_press() {
        key_sequence_test! {
            keyboard: create_tap_dance_test_keyboard(),
            sequence: [
                [0, 1, true, 150],  // Press td!(1)
                [0, 3, true, 10], // Press A
                [0, 3, false, 10], // Press A
                [0, 1, false, 10], // Release td!(1)
            ],
            expected_reports: [
                [0, [kc_to_u8!(Y), 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(Y), kc_to_u8!(A), 0, 0, 0, 0]],
                [0, [kc_to_u8!(Y), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_hold_after_tap_on_other_press() {
        key_sequence_test! {
            keyboard: create_tap_dance_test_keyboard(),
            sequence: [
                [0, 1, true, 150],  // Press td!(1)
                [0, 1, false, 100], // Release td!(1)
                [0, 1, true, 100],  // Press td!(1)
                [0, 3, true, 10], // Press A
                [0, 3, false, 10], // Press A
                [0, 1, false, 10], // Release td!(1)
            ],
            expected_reports: [
                [0, [kc_to_u8!(Z), 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(Z), kc_to_u8!(A), 0, 0, 0, 0]],
                [0, [kc_to_u8!(Z), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_multiple_tap() {
        key_sequence_test! {
            keyboard: create_tap_dance_test_keyboard(),
            sequence: [
                [0, 0, true, 150],  // Press td!(0)
                [0, 0, false, 10], // Release td!(0)
                [0, 0, true, 260],  // Press td!(0)
                [0, 0, false, 10], // Release td!(0)
                [0, 1, true, 260],  // Press td!(1)
                [0, 1, false, 10], // Release td!(1)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(X), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_tap_after_double_tap() {
        key_sequence_test! {
            keyboard: create_tap_dance_test_keyboard(),
            sequence: [
                [0, 0, true, 150],  // Press td!(0)
                [0, 0, false, 10], // Release td!(0)
                [0, 0, true, 150],  // Press td!(0)
                [0, 0, false, 10], // Release td!(0)
                [0, 0, true, 260],  // Press td!(0)
                [0, 0, false, 10], // Release td!(0)
            ],
            expected_reports: [
                [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_rolling() {
        key_sequence_test! {
            keyboard: create_tap_dance_test_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press td!(0)
                [0, 0, false, 10], // Release td!(0)
                [0, 0, true, 150], // Press td!(0)
                [0, 1, true, 10], // Press td!(1) -> Trigger hold-after-tap of td!(0)
                [0, 0, false, 100], // Release td!(0)
                [0, 1, false, 10], // Release td!(1)
            ],
            expected_reports: [
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(X), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_rolling_2() {
        key_sequence_test! {
            keyboard: create_tap_dance_test_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press td!(0)
                [0, 0, false, 10], // Release td!(0)
                [0, 0, true, 150], // Press td!(0)
                [0, 1, true, 260], // Press td!(1) -> td!(0) timeout
                [0, 0, false, 260], // Release td!(0) -> td!(1) timeout
                [0, 1, false, 10], // Release td!(1)
            ],
            expected_reports: [
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(C), kc_to_u8!(Y), 0, 0, 0, 0]],
                [0, [0, kc_to_u8!(Y), 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_rolling_3() {
        key_sequence_test! {
            keyboard: create_tap_dance_test_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press td!(0)
                [0, 0, false, 10], // Release td!(0)
                [0, 0, true, 150], // Press td!(0)
                [0, 1, true, 260], // Press td!(1),      td!(0) timeout (tap-hold) -> press "C"
                [0, 1, false, 260], // Release td!(1) -> td(1) hold, gap -> tap "Y"
                [0, 0, false, 260], // Release td!(0) -> release "C"
            ],
            expected_reports: [
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(C), kc_to_u8!(Y), 0, 0, 0, 0]],
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_multiple_tap_dance_keys() {
        key_sequence_test! {
            keyboard: create_tap_dance_test_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press td!(0)
                [0, 0, false, 10], // Release td!(0)
                [0, 0, true, 150], // Press td!(0)
                [0, 1, true, 10], // Press td!(1) -> Trigger hold-after-tap of td!(0)
                [0, 1, false, 10], // Release td!(1)
                [0, 0, false, 100], // Release td!(0)
            ],
            expected_reports: [
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(X), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }


    #[test]
    fn test_multiple_tap_dance_keys_2() {
        key_sequence_test! {
            keyboard: create_tap_dance_test_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press td!(0)
                [0, 0, false, 10], // Release td!(0)
                [0, 0, true, 150], // Press td!(0)
                [0, 1, true, 10], // Press td!(1) -> Trigger hold-after-tap of td!(0)
                [0, 1, false, 10], // Release td!(1)
                [0, 0, false, 300], // Release td!(0) -> td!(1) Timeout!
            ],
            expected_reports: [
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(C), kc_to_u8!(X), 0, 0, 0, 0]],
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn test_multiple_tap_dance_keys_3() {
        key_sequence_test! {
            keyboard: create_tap_dance_test_keyboard(),
            sequence: [
                [0, 0, true, 150], // Press td!(0)
                [0, 0, false, 10], // Release td!(0)
                [0, 0, true, 150], // Press td!(0)
                [0, 1, true, 10], // Press td!(1) -> Trigger hold-after-tap of td!(0)
                [0, 1, false, 310], // Release td!(1) -> td!(1) Timeout!
                [0, 0, false, 10], // Release td!(0)
            ],
            expected_reports: [
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(C), kc_to_u8!(Y), 0, 0, 0, 0]],
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }


}
