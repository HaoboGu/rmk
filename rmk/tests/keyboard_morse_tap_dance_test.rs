/// Test cases for tap-dance like morses
pub mod common;

use heapless::Vec;
use rmk::config::{BehaviorConfig, MorsesConfig, PositionalConfig};
use rmk::keyboard::Keyboard;
use rmk::morse::Morse;
use rmk::types::action::{Action, KeyAction, MorseMode, MorseProfile};
use rmk::types::keycode::{HidKeyCode, KeyCode};
use rmk::types::modifier::ModifierCombination;
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
            enable_flow_tap: false,
            default_profile: MorseProfile::new(
                Some(false),
                Some(MorseMode::HoldOnOtherPress),
                Some(250u16),
                Some(250u16),
            ),
            morses: Vec::from_slice(&[
                Morse::new_from_vial(
                    Action::Key(KeyCode::Hid(HidKeyCode::A)),
                    Action::Key(KeyCode::Hid(HidKeyCode::B)),
                    Action::Key(KeyCode::Hid(HidKeyCode::C)),
                    Action::Key(KeyCode::Hid(HidKeyCode::D)),
                    MorseProfile::const_default(),
                ),
                Morse::new_from_vial(
                    Action::Key(KeyCode::Hid(HidKeyCode::X)),
                    Action::Key(KeyCode::Hid(HidKeyCode::Y)),
                    Action::Key(KeyCode::Hid(HidKeyCode::Z)),
                    Action::Key(KeyCode::Hid(HidKeyCode::Space)),
                    MorseProfile::const_default(),
                ),
                Morse::new_from_vial(
                    Action::Key(KeyCode::Hid(HidKeyCode::Kp1)),
                    Action::Modifier(ModifierCombination::LSHIFT),
                    Action::Key(KeyCode::Hid(HidKeyCode::Kp2)),
                    Action::Modifier(ModifierCombination::LGUI),
                    MorseProfile::const_default(),
                ),
            ])
            .unwrap(),
            ..Default::default()
        },
        ..Default::default()
    };

    static BEHAVIOR_CONFIG: static_cell::StaticCell<BehaviorConfig> = static_cell::StaticCell::new();
    let behavior_config = BEHAVIOR_CONFIG.init(behavior_config);
    static KEY_CONFIG: static_cell::StaticCell<PositionalConfig<1, 4>> = static_cell::StaticCell::new();
    let per_key_config = KEY_CONFIG.init(PositionalConfig::default());
    Keyboard::new(wrap_keymap(keymap, per_key_config, behavior_config))
}

/// Create a keyboard with a morse key at (0,4) td!(0) that has:
///   tap = Enter, hold_after_tap = Enter (no double_tap)
/// This triggers the early fire optimization: tap fires immediately on release,
/// hold_after_tap still works on re-press.
/// Uses HoldOnOtherPress mode to reproduce the double-press bug scenario.
fn create_early_fire_keyboard() -> Keyboard<'static, 1, 5, 2> {
    let keymap = [
        [[k!(A), k!(B), k!(C), k!(D), KeyAction::Morse(0)]],
        [[k!(Kp1), k!(Kp2), k!(Kp3), k!(Kp4), k!(Kp5)]],
    ];

    let behavior_config = BehaviorConfig {
        morse: MorsesConfig {
            morses: Vec::from_slice(&[Morse::new_from_vial(
                Action::Key(KeyCode::Hid(HidKeyCode::Enter)),
                Action::Key(KeyCode::Hid(HidKeyCode::B)),
                Action::Key(KeyCode::Hid(HidKeyCode::Enter)),
                Action::No,
                MorseProfile::const_default(),
            )])
            .unwrap(),
            enable_flow_tap: false,
            default_profile: MorseProfile::new(
                Some(false),
                Some(MorseMode::HoldOnOtherPress),
                Some(250u16),
                Some(250u16),
            ),
            ..MorsesConfig::default()
        },
        ..Default::default()
    };

    static BEHAVIOR_CONFIG: static_cell::StaticCell<BehaviorConfig> = static_cell::StaticCell::new();
    let behavior_config = BEHAVIOR_CONFIG.init(behavior_config);
    static KEY_CONFIG: static_cell::StaticCell<PositionalConfig<1, 5>> = static_cell::StaticCell::new();
    let per_key_config = KEY_CONFIG.init(PositionalConfig::default());
    Keyboard::new(wrap_keymap(keymap, per_key_config, behavior_config))
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

    /// Test that early fire does not produce double key press when another key is pressed shortly after.
    ///
    /// Scenario: Press td!(0) (tap=Enter, hold_after_tap=Enter), release quickly (early fire triggers Enter),
    /// then press normal key 'A' shortly after.
    ///
    /// Expected: Enter press, Enter release, A press (NOT: Enter press, Enter release, Enter press, Enter release, A press)
    #[test]
    fn test_early_fire_no_double_press_on_next_key() {
        key_sequence_test! {
            keyboard: create_early_fire_keyboard(),
            sequence: [
                [0, 4, true, 10],   // Press td!(0) morse key
                [0, 4, false, 50],  // Release td!(0) quickly — early fire triggers Enter
                [0, 0, true, 50],   // Press A shortly after
                [0, 0, false, 10],  // Release A
                [0, 0, true, 50],   // Press A shortly after
                [0, 0, false, 300],  // Release A
            ],
            expected_reports: [
                // Early fire: Enter tap (press + release)
                [0, [kc_to_u8!(Enter), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                // Normal key A (press + release)
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                // Normal key A (press + release)
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    /// Test that after early fire, re-pressing and holding the same key triggers hold_after_tap.
    ///
    /// Scenario: Press td!(0), release quickly (early fire triggers Enter),
    /// then re-press td!(0) and hold past timeout.
    ///
    /// Expected: Enter press, Enter release (early fire), then hold_after_tap triggers Enter again
    #[test]
    fn test_early_fire_then_hold_after_tap() {
        key_sequence_test! {
            keyboard: create_early_fire_keyboard(),
            sequence: [
                [0, 4, true, 10],    // Press td!(0) morse key
                [0, 4, false, 50],   // Release td!(0) quickly — early fire triggers Enter
                [0, 4, true, 50],    // Re-press td!(0)
                [0, 4, false, 300],  // Hold past timeout, then release — hold_after_tap fires Enter
            ],
            expected_reports: [
                // Early fire: Enter tap (press + release)
                [0, [kc_to_u8!(Enter), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
                // hold_after_tap: Enter (hold, then release)
                [0, [kc_to_u8!(Enter), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }


}
