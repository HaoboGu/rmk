pub mod common;

use embassy_time::Duration;
use rmk::config::{BehaviorConfig, OneShotConfig};
use rmk::keycode::ModifierCombination;

fn one_shot_config_with_short_timeout() -> OneShotConfig {
    OneShotConfig {
        timeout: Duration::from_millis(100),
    }
}

mod one_shot_test {
    use std::cell::RefCell;

    use embassy_futures::block_on;
    use rmk::action::KeyAction;
    use rmk::keyboard::Keyboard;
    use rmk::keymap::KeyMap;
    use rmk::{k, osl, osm, th, wm};
    use rusty_fork::rusty_fork_test;

    use super::*;
    use crate::common::{wrap_keymap, KC_LCTRL, KC_LGUI, KC_LSHIFT};

    const KEYMAP: [[[KeyAction; 6]; 1]; 2] = [
        [[
            // Layer 0
            osm!(ModifierCombination::new_from(false, false, false, true, false)), // OSM LShift
            osl!(1),                                                               // OSL Layer 1
            k!(A),                                                                 // Regular key A
            th!(B, C),                                                             // Tap-hold key B, C
            osm!(ModifierCombination::new_from(false, false, false, false, true)), // OSM LCtrl
            wm!(B, ModifierCombination::new_from(false, true, false, false, false)), // WM B with LGUI
        ]],
        [[
            // Layer 1
            osm!(ModifierCombination::new_from(false, false, false, true, true)), // OSM LShift + LCtrl
            k!(No),                                                               // No action
            k!(C),                                                                // Layer 1 key C
            k!(D),                                                                // Layer 1 key D
            k!(E),                                                                // Layer 1 key E
            k!(F),                                                                // Layer 1 key F
        ]],
    ];

    fn create_test_keyboard() -> Keyboard<'static, 1, 6, 2> {
        let keymap: &RefCell<KeyMap<1, 6, 2>> = wrap_keymap(KEYMAP, BehaviorConfig::default());
        Keyboard::new(keymap)
    }

    /// Create test keyboard with short timeout
    fn create_test_keyboard_with_short_timeout() -> Keyboard<'static, 1, 6, 2> {
        let keymap: &RefCell<KeyMap<1, 6, 2>> = wrap_keymap(
            KEYMAP,
            BehaviorConfig {
                one_shot: one_shot_config_with_short_timeout(),
                ..BehaviorConfig::default()
            },
        );
        Keyboard::new(keymap)
    }

    rusty_fork_test! {
        #[test]
        fn test_osm_basic_single_behavior() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [0, 0, true, 10],   // Press OSM LShift
                    [0, 0, false, 10],  // Release OSM LShift
                    [0, 2, true, 10],   // Press A
                    [0, 2, false, 10],  // Release A
                ],
                expected_reports: [
                    [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // A with LShift
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_osm_held_behavior() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [0, 0, true, 10],   // Press OSM LShift
                    [0, 2, true, 10],   // Press A while OSM is held
                    [0, 2, false, 10],  // Release A
                    [0, 0, false, 10],  // Release OSM LShift
                ],
                expected_reports: [
                    [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // A with LShift
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Still holding LShift
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_osm_timeout() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_short_timeout(),
                sequence: [
                    [0, 0, true, 10],   // Press OSM LShift
                    [0, 0, false, 10],  // Release OSM LShift
                    [0, 2, true, 150],  // Press A after timeout (delay > 100ms)
                    [0, 2, false, 10],  // Release A
                ],
                expected_reports: [
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // A without LShift (timeout)
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_osm_multiple_keys() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [0, 0, true, 10],   // Press OSM LShift
                    [0, 0, false, 10],  // Release OSM LShift
                    [0, 2, true, 10],   // Press A
                    [0, 2, false, 10],  // Release A
                    [0, 3, true, 10],   // Press B (should not have shift)
                    [0, 3, false, 10],  // Release B
                ],
                expected_reports: [
                    [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // A with LShift
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                    [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // B without LShift
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_osm_combined_modifiers() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [0, 0, true, 10],   // Press OSM LShift
                    [0, 0, false, 10],  // Release OSM LShift
                    [0, 4, true, 10],   // Press OSM LCtrl
                    [0, 4, false, 10],  // Release OSM LCtrl
                    [0, 2, true, 10],   // Press A
                    [0, 2, false, 10],  // Release A
                ],
                expected_reports: [
                    [KC_LSHIFT | KC_LCTRL, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // A with LShift+LCtrl
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        // OSL Tests
        #[test]
        fn test_osl_basic_single_behavior() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [0, 1, true, 10],   // Press OSL Layer 1
                    [0, 1, false, 10],  // Release OSL Layer 1
                    [0, 2, true, 10],   // Press key at (0,2), should get C from layer 1
                    [0, 2, false, 10],  // Release key
                ],
                expected_reports: [
                    [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]], // C from layer 1
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_osl_held_behavior() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [0, 1, true, 10],   // Press OSL Layer 1
                    [0, 2, true, 10],   // Press key at (0,2) while OSL is held
                    [0, 2, false, 10],  // Release key
                    [0, 1, false, 10],  // Release OSL Layer 1
                ],
                expected_reports: [
                    [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]], // C from layer 1
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_osl_timeout() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_short_timeout(),
                sequence: [
                    [0, 1, true, 10],   // Press OSL Layer 1
                    [0, 1, false, 10],  // Release OSL Layer 1
                    [0, 2, true, 150],  // Press key at (0,2) after timeout (delay > 100ms)
                    [0, 2, false, 10],  // Release key
                ],
                expected_reports: [
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // A from layer 0 (timeout)
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_osl_multiple_keys() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [0, 1, true, 10],   // Press OSL Layer 1
                    [0, 1, false, 10],  // Release OSL Layer 1
                    [0, 2, true, 10],   // Press key at (0,2), should get C from layer 1
                    [0, 2, false, 10],  // Release key
                    [0, 3, true, 10],   // Press key at (0,3), should get B from layer 0
                    [0, 3, false, 10],  // Release key
                ],
                expected_reports: [
                    [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]], // C from layer 1
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                    [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // B from layer 0
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_osm_then_osl() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [0, 0, true, 10],   // Press OSM LShift
                    [0, 0, false, 10],  // Release OSM LShift
                    [0, 1, true, 10],   // Press OSL Layer 1
                    [0, 1, false, 10],  // Release OSL Layer 1
                    [0, 2, true, 10],   // Press key at (0,2), should get C from layer 1 with shift
                    [0, 2, false, 10],  // Release key
                ],
                expected_reports: [
                    [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]], // C from layer 1 with LShift
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_osl_then_osm() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [0, 1, true, 10],   // Press OSL Layer 1
                    [0, 1, false, 10],  // Release OSL Layer 1
                    [0, 0, true, 10],   // Press OSM LShift (from layer 1, but No action)
                    [0, 0, false, 10],  // Release OSM LShift (gets from layer 0 due to transparent)
                    [0, 2, true, 10],   // Press key at (0,2), should get A from layer 0 with shift + ctrl
                    [0, 2, false, 10],  // Release key
                ],
                expected_reports: [
                    [KC_LSHIFT | KC_LCTRL, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // A from layer 0 with shift + ctrl
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_osm_and_osl_timeout() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_short_timeout(),
                sequence: [
                    [0, 0, true, 10],   // Press OSM LShift
                    [0, 0, false, 10],  // Release OSM LShift
                    [0, 1, true, 10],   // Press OSL Layer 1
                    [0, 1, false, 10],  // Release OSL Layer 1
                    [0, 2, true, 200], // Press key at (0,2) after timeout (delay > 100ms)
                    [0, 2, false, 10],  // Release key
                ],
                expected_reports: [
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // A from layer 0 (both timeout)
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_multiple_osm() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [0, 0, true, 10],   // Press OSM LShift
                    [0, 0, false, 10],  // Release OSM LShift
                    [0, 4, true, 10],   // Press OSM LCtrl
                    [0, 4, false, 10],  // Release OSM LCtrl
                    [0, 3, true, 10],   // Press key at (0,3), should get B from layer 0 with shift + ctrl
                    [0, 3, false, 10],  // Release key
                ],
                expected_reports: [
                    [KC_LSHIFT | KC_LCTRL, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // B from layer 0 with LShift
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_multiple_osm_with_wm() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [0, 0, true, 10],   // Press OSM LShift
                    [0, 0, false, 10],  // Release OSM LShift
                    [0, 4, true, 10],   // Press OSM LCtrl
                    [0, 4, false, 10],  // Release OSM LCtrl
                    [0, 5, true, 10],   // Press key at (0,5), should get B from layer 0 with LShift + LCtrl + LGui
                    [0, 5, false, 10],  // Release key
                ],
                expected_reports: [
                    [KC_LSHIFT | KC_LCTRL | KC_LGUI, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // B from layer 0 with LShift + LCtrl + LGui
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }
    }
}
