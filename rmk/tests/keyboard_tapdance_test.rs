pub mod common;

use embassy_time::Duration;
use rmk::action::{Action, KeyAction};
use rmk::config::{BehaviorConfig, TapDancesConfig};
use rmk::keycode::KeyCode;
use rmk::tap_dance::TapDance;

fn create_tap_dance_config() -> TapDancesConfig {
    let mut config = TapDancesConfig::default();
    // TapDance 0: Tap A, Hold B, HoldAfterTap C, DoubleTap D
    let td0 = TapDance::new_from_vial(
        KeyAction::Single(Action::Key(KeyCode::A)),
        KeyAction::Single(Action::Key(KeyCode::B)),
        KeyAction::Single(Action::Key(KeyCode::C)),
        KeyAction::Single(Action::Key(KeyCode::D)),
        Duration::from_millis(200),
    );
    config.tap_dances.push(td0).unwrap();

    // TapDance 1: Different actions for testing
    let td1 = TapDance::new_from_vial(
        KeyAction::Single(Action::Key(KeyCode::X)),
        KeyAction::Single(Action::Key(KeyCode::Y)),
        KeyAction::Single(Action::Key(KeyCode::LShift)),
        KeyAction::Single(Action::Key(KeyCode::Space)),
        Duration::from_millis(150),
    );
    config.tap_dances.push(td1).unwrap();

    config
}

mod tap_dance_test {
    use embassy_futures::block_on;
    use rmk::config::TapHoldConfig;
    use rmk::keyboard::Keyboard;
    use rusty_fork::rusty_fork_test;

    use super::*;
    use crate::common::{wrap_keymap, KC_LSHIFT};

    fn create_simple_tap_dance_keyboard() -> Keyboard<'static, 1, 3, 1> {
        let keymap = [[[
            KeyAction::TapDance(0),                     // TapDance 0 at (0,0)
            KeyAction::TapDance(1),                     // TapDance 1 at (0,1)
            KeyAction::Single(Action::Key(KeyCode::A)), // A at (0,2)
        ]]];

        let config = BehaviorConfig {
            tap_dance: create_tap_dance_config(),
            tap_hold: TapHoldConfig {
                enable_hrm: true,
                permissive_hold: true,
                ..Default::default()
            },
            ..Default::default()
        };

        Keyboard::new(wrap_keymap(keymap, config))
    }

    rusty_fork_test! {
        #[test]
        fn test_tap_dance_single_tap() {
            // Test single tap -> should trigger tap action
            key_sequence_test! {
                keyboard: create_simple_tap_dance_keyboard(),
                sequence: [
                    [0, 0, true, 10],   // Press TapDance key
                    [0, 0, false, 100], // Release within tapping_term
                ],
                expected_reports: [
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Tap action (A)
                    [0, [0, 0, 0, 0, 0, 0]], // Release
                ]
            };
        }

        #[test]
        fn test_tap_dance_hold() {
            // Test hold -> should trigger hold action
            key_sequence_test! {
                keyboard: create_simple_tap_dance_keyboard(),
                sequence: [
                    [0, 0, true, 10],   // Press TapDance key
                    [0, 0, false, 250], // Release after tapping_term
                ],
                expected_reports: [
                    [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Hold action (B)
                    [0, [0, 0, 0, 0, 0, 0]], // Release
                ]
            };
        }

        #[test]
        fn test_tap_dance_double_tap() {
            // Test double tap -> should trigger double_tap action
            key_sequence_test! {
                keyboard: create_simple_tap_dance_keyboard(),
                sequence: [
                    [0, 0, true, 10],   // First press
                    [0, 0, false, 190],  // First release (quick)
                    [0, 0, true, 190],   // Second press within tapping_term
                    [0, 0, false, 190],  // Second release (quick)
                ],
                expected_reports: [
                    [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]], // Double tap action (D)
                    [0, [0, 0, 0, 0, 0, 0]], // Release
                ]
            };
        }

        #[test]
        fn test_tap_dance_hold_after_tap() {
            // Test tap then hold -> should trigger hold_after_tap action
            key_sequence_test! {
                keyboard: create_simple_tap_dance_keyboard(),
                sequence: [
                    [0, 0, true, 10],   // First press
                    [0, 0, false, 50],  // First release (quick)
                    [0, 0, true, 190],   // Second press within tapping_term
                    [0, 0, false, 250], // Hold second press
                ],
                expected_reports: [
                    [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]], // Hold after tap action (C)
                    [0, [0, 0, 0, 0, 0, 0]], // Release
                ]
            };
        }

        #[test]
        fn test_tap_dance_timeout_single_tap() {
            // Test single tap with timeout -> should trigger tap action
            key_sequence_test! {
                keyboard: create_simple_tap_dance_keyboard(),
                sequence: [
                    [0, 0, true, 10],   // Press TapDance key
                    [0, 0, false, 190],  // Quick release
                    [0, 1, true, 250], // Tap another key
                    [0, 1, false, 140], // Release another key
                ],
                expected_reports: [
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Tap action (A) after timeout
                    [0, [0, 0, 0, 0, 0, 0]], // Release
                    [0, [kc_to_u8!(X), 0, 0, 0, 0, 0]], // Tap action (X)
                    [0, [0, 0, 0, 0, 0, 0]], // Release
                ]
            };
        }

        #[test]
        fn test_tap_dance_triple_tap() {
            // Test triple tap -> should trigger tap action (fallback for > double tap)
            key_sequence_test! {
                keyboard: create_simple_tap_dance_keyboard(),
                sequence: [
                    [0, 0, true, 10],   // First press
                    [0, 0, false, 190],  // First release
                    [0, 0, true, 190],   // Second press
                    [0, 0, false, 190],  // Second release
                    [0, 0, true, 190],   // Third press
                    [0, 0, false, 190],  // Third release
                ],
                expected_reports: [
                    [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]], // Tap action (D) for double tap
                    [0, [0, 0, 0, 0, 0, 0]], // Release
                ]
            };
        }

        #[test]
        fn test_tap_dance_tap_after_double_tap() {
            key_sequence_test! {
                keyboard: create_simple_tap_dance_keyboard(),
                sequence: [
                    [0, 0, true, 10],   // First press
                    [0, 0, false, 190],  // First release
                    [0, 0, true, 30],   // Second press
                    [0, 0, false, 190],  // Second release
                    [0, 0, true, 300],   // Third press
                    [0, 0, false, 190],  // Third release
                ],
                expected_reports: [
                    [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]], // Tap action (D) for double tap
                    [0, [0, 0, 0, 0, 0, 0]], // Release
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Tap action (A) for tap
                    [0, [0, 0, 0, 0, 0, 0]], // Release
                ]
            };
        }

        #[test]
        fn test_tap_dance_interrupt_by_other_key() {
            // Test tap dance interrupted by other key
            key_sequence_test! {
                keyboard: create_simple_tap_dance_keyboard(),
                sequence: [
                    [0, 0, true, 10],   // Press TapDance key
                    [0, 0, false, 50],  // Release TapDance key
                    [0, 1, true, 50],   // Press other key (should trigger tap action)
                    [0, 1, false, 50],  // Release other key
                ],
                expected_reports: [
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Tap action triggered by interruption
                    [0, [0, 0, 0, 0, 0, 0]], // Release tap action
                    [0, [kc_to_u8!(X), 0, 0, 0, 0, 0]], // Other key (TapDance 1 tap action)
                    [0, [0, 0, 0, 0, 0, 0]], // Release other key
                ]
            };
        }

        #[test]
        fn test_multiple_tap_dance_keys() {
            // Test multiple tap dance keys pressed simultaneously
            key_sequence_test! {
                keyboard: create_simple_tap_dance_keyboard(),
                sequence: [
                    [0, 1, true, 10],   // Press TD(1)
                    [0, 1, false, 10],   // Release TD(1)
                    [0, 1, true, 10],   // Press TD(1)
                    [0, 0, true, 50],   // Press TD(0)
                    [0, 1, false, 210],  // Release TD(1)
                    [0, 0, false, 50],  // Release TD(0)
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // First tap action
                    [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // Release TD(0)
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }

        #[test]
        fn test_tap_dance_different_timing() {
            // Test with different tapping_term (TapDance 1 has 150ms)
            key_sequence_test! {
                keyboard: create_simple_tap_dance_keyboard(),
                sequence: [
                    [0, 1, true, 10],   // Press TapDance 1
                    [0, 1, false, 180], // Release after 180ms (> 150ms tapping_term)
                ],
                expected_reports: [
                    [0, [kc_to_u8!(Y), 0, 0, 0, 0, 0]], // Hold action (Y) for TapDance 1
                    [0, [0, 0, 0, 0, 0, 0]], // Release
                ]
            };
        }

        #[test]
        fn test_tap_dance_hold_with_other_key() {
            key_sequence_test! {
                keyboard: create_simple_tap_dance_keyboard(),
                sequence: [
                    [0, 1, true, 10],   // Press TapDance 1
                    [0, 1, false, 140], // Tap
                    [0, 1, true, 130],   // Then hold TapDance 1
                    [0, 2, true, 160],   // Press A
                    [0, 2, false, 140],   // Release A
                    [0, 1, false, 140], // Release TapDance 1
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold action (Y) for TapDance 1
                    [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Hold action (Y) for TapDance 1
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold action (Y) for TapDance 1
                    [0, [0, 0, 0, 0, 0, 0]], // Release
                ]
            };
        }

        #[test]
        fn test_tap_dance_hold_with_other_key_reversed() {
            key_sequence_test! {
                keyboard: create_simple_tap_dance_keyboard(),
                sequence: [
                    [0, 1, true, 10],   // Press TapDance 1
                    [0, 1, false, 140], // Tap
                    [0, 1, true, 130],   // Then hold TapDance 1
                    [0, 2, true, 160],   // Press A
                    [0, 1, false, 140], // Release TapDance 1
                    [0, 2, false, 140],   // Release A
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold action (Y) for TapDance 1
                    [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Hold action (Y) for TapDance 1
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Hold action (Y) for TapDance 1
                    [0, [0, 0, 0, 0, 0, 0]], // Release
                ]
            };
        }

        #[test]
        fn test_tap_dance_hold_with_permissive_hold() {
            key_sequence_test! {
                keyboard: create_simple_tap_dance_keyboard(),
                sequence: [
                    [0, 1, true, 10],   // Press TapDance 1
                    [0, 1, false, 140], // Tap
                    [0, 1, true, 130],  // Then hold TapDance 1
                    [0, 2, true, 10],   // Press A
                    [0, 2, false, 140], // Release A
                    [0, 1, false, 140], // Release TapDance 1
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold action (Y) for TapDance 1
                    [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Hold action (Y) for TapDance 1
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold action (Y) for TapDance 1
                    [0, [0, 0, 0, 0, 0, 0]], // Release
                ]
            };
        }

        #[test]
        fn test_tap_dance_hold_with_tap() {
            key_sequence_test! {
                keyboard: create_simple_tap_dance_keyboard(),
                sequence: [
                    [0, 1, true, 10],   // Press TapDance 1
                    [0, 1, false, 140], // Tap
                    [0, 1, true, 130],  // Then hold TapDance 1
                    [0, 2, true, 10],   // Press A, TD(1)'s decision is "hold"
                    [0, 1, false, 120], // Release TapDance 1
                    [0, 2, false, 140], // Release A
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold action (Y) for TapDance 1
                    [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Hold action (Y) for TapDance 1
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Hold action (Y) for TapDance 1
                    [0, [0, 0, 0, 0, 0, 0]], // Release
                ]
            };
        }
    }
}
