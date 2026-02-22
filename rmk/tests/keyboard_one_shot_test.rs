pub mod common;

use embassy_time::Duration;
use rmk::config::{BehaviorConfig, OneShotModifiersConfig};
use rmk::types::modifier::ModifierCombination;

mod one_shot_test {
    use std::cell::RefCell;

    use rmk::config::{OneShotConfig, PositionalConfig};
    use rmk::keyboard::Keyboard;
    use rmk::keymap::KeyMap;
    use rmk::types::action::KeyAction;
    use rmk::{k, osl, osm, th, wm};
    use rusty_fork::rusty_fork_test;

    use super::*;
    use crate::common::{KC_LCTRL, KC_LGUI, KC_LSHIFT, wrap_keymap};

    // KEYMAP
    // Layer 0: OSM(LShift)        OSL(1)  A  TH(B)  OSM(LCtrl)  WM(B)
    // Layer 1: OSM(LShift|LCtrl)  No      C  D      E           F

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
        static BEHAVIOR_CONFIG: static_cell::StaticCell<BehaviorConfig> = static_cell::StaticCell::new();
        let behavior_config = BEHAVIOR_CONFIG.init(BehaviorConfig::default());
        static KEY_CONFIG: static_cell::StaticCell<PositionalConfig<1, 6>> = static_cell::StaticCell::new();
        let per_key_config = KEY_CONFIG.init(PositionalConfig::default());
        let keymap: &RefCell<KeyMap<1, 6, 2>> = wrap_keymap(KEYMAP, per_key_config, behavior_config);
        Keyboard::new(keymap)
    }

    fn create_test_keyboard_with_behavior_config(config: BehaviorConfig) -> Keyboard<'static, 1, 6, 2> {
        static BEHAVIOR_CONFIG: static_cell::StaticCell<BehaviorConfig> = static_cell::StaticCell::new();
        let behavior_config = BEHAVIOR_CONFIG.init(config);
        static KEY_CONFIG: static_cell::StaticCell<PositionalConfig<1, 6>> = static_cell::StaticCell::new();
        let per_key_config = KEY_CONFIG.init(PositionalConfig::default());
        let keymap: &RefCell<KeyMap<1, 6, 2>> = wrap_keymap(KEYMAP, per_key_config, behavior_config);
        Keyboard::new(keymap)
    }

    fn create_test_keyboard_with_one_shot_modifiers_config(
        config: OneShotModifiersConfig,
    ) -> Keyboard<'static, 1, 6, 2> {
        static BEHAVIOR_CONFIG: static_cell::StaticCell<BehaviorConfig> = static_cell::StaticCell::new();
        let behavior_config = BEHAVIOR_CONFIG.init(BehaviorConfig {
            one_shot_modifiers: config,
            ..BehaviorConfig::default()
        });
        static KEY_CONFIG: static_cell::StaticCell<PositionalConfig<1, 6>> = static_cell::StaticCell::new();
        let per_key_config = KEY_CONFIG.init(PositionalConfig::default());
        let keymap: &RefCell<KeyMap<1, 6, 2>> = wrap_keymap(KEYMAP, per_key_config, behavior_config);
        Keyboard::new(keymap)
    }

    rusty_fork_test! {
        /// OSM Test Case 1
        ///
        /// Config:
        /// - timeout: 1000ms
        /// - activate_on_keypress: false
        /// - send_on_second_press: false
        ///
        /// Sequence:
        /// - Press and Release OSM LShift
        /// - Press and Release regular key A
        ///
        /// Expected:
        /// - A with LShift
        /// - All released
        #[test]
        fn test_osm_basic_single_behavior() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    // Press and Release OSM LShift
                    [0, 0, true, 10],
                    [0, 0, false, 10],
                    // Press and Release A
                    [0, 2, true, 10],
                    [0, 2, false, 10],
                ],
                expected_reports: [
                    [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // A with LShift
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        /// OSM Test Case 2
        ///
        /// Config:
        /// - timeout: 100ms
        /// - activate_on_keypress: false
        /// - send_on_second_press: false
        ///
        /// Sequence:
        /// - Press and Release OSM LShift
        /// - Press and Release A after timeout (delay > 100ms)
        ///
        /// Expected:
        /// - A is sent without LShift
        /// - All released
        #[test]
        fn test_osm_timeout() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_behavior_config(
                    BehaviorConfig {
                        one_shot: OneShotConfig {
                            timeout: Duration::from_millis(100),
                            ..OneShotConfig::default()
                        },
                        ..BehaviorConfig::default()
                    }
                ),
                sequence: [
                    // Press and Release OSM LShift
                    [0, 0, true, 10],
                    [0, 0, false, 10],
                    // Press and Release A after timeout (delay > 100ms)
                    [0, 2, true, 150],
                    [0, 2, false, 10],
                ],
                expected_reports: [
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // A without LShift (timeout)
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        /// OSM Test Case 3
        ///
        /// Config:
        /// - timeout: 1000ms
        /// - activate_on_keypress: false
        /// - send_on_second_press: false
        ///
        /// Sequence:
        /// - Press OSM LShift
        /// - Press A while OSM is held
        /// - Release A
        /// - Release OSM LShift
        ///
        /// Expected:
        /// - A with LShift
        /// - LShift is still held
        /// - All released
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

        /// OSM Test Case 4
        ///
        /// Config:
        /// - timeout: 1000ms
        /// - activate_on_keypress: false
        /// - send_on_second_press: false
        ///
        /// Sequence:
        /// - Press and Release OSM LShift
        /// - Press and Release regular key A
        /// - Press and Release regular key B
        ///
        /// Expected:
        /// - A with LShift
        /// - All released
        /// - B without LShift
        /// - All released
        #[test]
        fn test_osm_multiple_keys() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    // Press and Release OSM LShift
                    [0, 0, true, 10],
                    [0, 0, false, 10],
                    // Press and Release A
                    [0, 2, true, 10],
                    [0, 2, false, 10],
                    // Press and Release B
                    [0, 3, true, 10],
                    [0, 3, false, 10],
                ],
                expected_reports: [
                    [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // A with LShift
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                    [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // B without LShift
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }


        /// OSM Test Case 5
        ///
        /// Config:
        /// - timeout: 1000ms
        /// - activate_on_keypress: false
        /// - send_on_second_press: false
        ///
        /// Sequence:
        /// - Press OSM LShift
        /// - Press B
        /// - Release OSM LShift
        /// - Release B
        ///
        /// Expected:
        /// - B with LShift
        /// - All released
        #[test]
        fn test_osm_rolling_with_tap_hold() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [0, 0, true, 10],   // Press OSM LShift
                    [0, 3, true, 10],   // Press B
                    [0, 0, false, 10],  // Release OSM LShift
                    [0, 3, false, 10],  // Release B
                ],
                expected_reports: [
                    [KC_LSHIFT, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // B with LShift
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        /// OSM Test Case 6
        ///
        /// Config:
        /// - timeout: 1000ms
        /// - activate_on_keypress: false
        /// - send_on_second_press: false
        ///
        /// Sequence:
        /// - Press and Release OSM LShift
        /// - Press and Release OSM LCtrl
        /// - Press and Release regular key A
        ///
        /// Expected:
        /// - A with LShift+LCtrl
        /// - All released
        #[test]
        fn test_osm_combined_modifiers() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    // Press and Release OSM LShift
                    [0, 0, true, 10],
                    [0, 0, false, 10],
                    // Press and Release OSM LCtrl
                    [0, 4, true, 10],
                    [0, 4, false, 10],
                    // Press and Release A
                    [0, 2, true, 10],
                    [0, 2, false, 10],
                ],
                expected_reports: [
                    [KC_LSHIFT | KC_LCTRL, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // A with LShift+LCtrl
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        /// OSM Test Case 7
        ///
        /// Config:
        /// - timeout: 100ms
        /// - activate_on_keypress: false
        /// - send_on_second_press: false
        ///
        /// Sequence:
        /// - Press and Release OSM LShift
        /// - Press and Release OSM LCtrl
        /// - Press and Release WM(B, LGui)
        ///
        /// Expected:
        /// - B is sent with LShift + LCtrl + LGui
        /// - All released
        #[test]
        fn test_osm_multiple_osm_with_wm() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    // Press and Release OSM LShift
                    [0, 0, true, 10],
                    [0, 0, false, 10],
                    // Press and Release OSM LCtrl
                    [0, 4, true, 10],
                    [0, 4, false, 10],
                    // Press and Release WM(B, LGui)
                    [0, 5, true, 10],
                    [0, 5, false, 10],
                ],
                expected_reports: [
                    [KC_LSHIFT | KC_LCTRL | KC_LGUI, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // B with LShift + LCtrl + LGui
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        /// OSM Test Case 8
        ///
        /// Config:
        /// - timeout: 100ms
        /// - activate_on_keypress: true
        /// - send_on_second_press: false
        ///
        /// Sequence:
        /// - Press OSM LShift
        /// - Release OSM LShift
        /// - Press A
        /// - Release A
        ///
        /// Expected:
        /// - LShift is sent from the start
        /// - A with LShift
        /// - All released
        #[test]
        fn test_osm_activate_on_keypress() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_one_shot_modifiers_config(OneShotModifiersConfig {
                    activate_on_keypress: true,
                    send_on_second_press: false,
                    ..OneShotModifiersConfig::default()
                }),
                sequence: [
                    [0, 0, true, 10],   // Press OSM LShift
                    [0, 0, false, 10],  // Release OSM LShift
                    [0, 2, true, 10],   // Press A
                    [0, 2, false, 10],  // Release A
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // LShift is sent from the start
                    [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // A with LShift
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            }
        }

        /// OSM Test Case 9
        ///
        /// Config:
        /// - timeout: 100ms
        /// - activate_on_keypress: true
        /// - send_on_second_press: false
        ///
        /// Sequence:
        /// - Press and Release OSM LShift
        /// - Press and Release OSM LCtrl
        /// - Press and Release regular key A
        ///
        /// Expected:
        /// - LShift is sent first
        /// - LCtrl is added to combination
        /// - A with LShift+LCtrl
        /// - All released
        #[test]
        fn test_osm_combined_modifiers_with_activate_on_keypress() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_one_shot_modifiers_config(OneShotModifiersConfig {
                    activate_on_keypress: true,
                    send_on_second_press: false,
                    ..OneShotModifiersConfig::default()
                }),
                sequence: [
                    // Press and Release OSM LShift
                    [0, 0, true, 10],
                    [0, 0, false, 10],
                    // Press and Release OSM LCtrl
                    [0, 4, true, 10],
                    [0, 4, false, 10],
                    // Press and Release A
                    [0, 2, true, 10],
                    [0, 2, false, 10],
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // LShift is sent first
                    [KC_LSHIFT | KC_LCTRL, [0, 0, 0, 0, 0, 0]], // LCtrl is added to combination
                    [KC_LSHIFT | KC_LCTRL, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // A with LShift+LCtrl
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        /// OSM Test Case 10
        ///
        /// Config:
        /// - timeout: 100ms
        /// - activate_on_keypress: false
        /// - send_on_second_press: true
        ///
        /// Sequence:
        /// - Press and Release OSM LShift
        /// - Press and Release OSM LShift
        /// - Press and Release A
        ///
        /// Expected:
        /// - LShift is sent on second press
        /// - A is sent without LShift
        #[test]
        fn test_osm_release_osm_with_send_on_second_press() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_one_shot_modifiers_config(OneShotModifiersConfig {
                    activate_on_keypress: false,
                    send_on_second_press: true,
                    ..OneShotModifiersConfig::default()
                }),
                sequence: [
                    // Press and Release OSM LShift
                    [0, 0, true, 10],    // Initial
                    [0, 0, false, 10],   // Send event to unprocessed_events
                    // Press and Release OSM LShift
                    [0, 0, true, 10],
                    [0, 0, false, 10],
                    // Press and Release A
                    [0, 2, true, 10],
                    [0, 2, false, 10],
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [0 , [0, 0, 0, 0, 0, 0]], // All released
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Only A is sent
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        /// OSM Test Case 11
        ///
        /// Config:
        /// - timeout: 100ms
        /// - activate_on_keypress: false
        /// - send_on_second_press: true
        ///
        /// Sequence:
        /// - Press OSM LShift
        /// - Press OSM LCtrl
        /// - Release OSM LCtrl
        /// - Press OSM LCtrl
        /// - Press regular key A
        ///
        /// Expected:
        /// - LShift is sent first
        /// - LShift + LCtrl is sent second
        /// - Only LShift is sent after LCtrl pressed again
        #[test]
        fn test_osm_combined_mods_with_send_on_second_press() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_one_shot_modifiers_config(OneShotModifiersConfig {
                    activate_on_keypress: false,
                    send_on_second_press: true,
                    ..OneShotModifiersConfig::default()
                }),
                sequence: [
                    [0, 0, true, 10],   // Press   OSM LShift
                    [0, 4, true, 10],   // Press   OSM LCtrl
                    [0, 4, false, 10],  // Release OSM LCtrl
                    [0, 4, true, 10],   // Press   OSM LCtrl
                    [0, 4, false, 10],  // Release OSM LCtrl
                    [0, 0, false, 10],  // Release OSM LShift
                    // Press and Release A
                    [0, 2, true, 10],
                    [0, 2, false, 10],
                ],
                expected_reports: [
                    [KC_LSHIFT | KC_LCTRL, [0, 0, 0, 0, 0, 0]], // Ctrl with Shift is sent
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Ctrl with Shift is sent
                    [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // A with Shift only
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        /// OSM Test Case 12
        ///
        /// Config:
        /// - timeout: 100ms
        /// - activate_on_keypress: true
        /// - send_on_second_press: true
        ///
        /// Sequence:
        /// - Press and Release OSM LShift
        /// - Press and Release OSM LShift
        /// - Press and Release regular key A
        ///
        /// Expected:
        /// - LShift is sent first
        /// - LShift is sent again and unstuck
        /// - A is sent without LShift
        #[test]
        fn test_osm_activate_on_keypress_and_send_on_second_press() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_one_shot_modifiers_config(OneShotModifiersConfig {
                    activate_on_keypress: true,
                    send_on_second_press: true,
                    ..OneShotModifiersConfig::default()
                }),
                sequence: [
                    // Press and Release OSM LShift
                    [0, 0, true, 10],
                    [0, 0, false, 10],
                    // Press and Release OSM LShift
                    [0, 0, true, 10],
                    [0, 0, false, 10],
                    // Press and Release A
                    [0, 2, true, 10],
                    [0, 2, false, 10],
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],    // Shift is sent from beginning
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],    // Shift is sent (second press)
                    [0, [0, 0, 0, 0, 0, 0]],            // All released
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // A sent without Shift
                    [0, [0, 0, 0, 0, 0, 0]],            // All released
                ]
            };
        }

        /// OSM Test Case 13
        ///
        /// Config:
        /// - timeout: 100ms
        /// - activate_on_keypress: true
        /// - send_on_second_press: true
        ///
        /// Sequence:
        /// - Press OSM LShift
        /// - Press OSM LCtrl
        /// - Release OSM LCtrl
        /// - Press OSM LCtrl
        /// - Release OSM LCtrl
        /// - Press regular key A
        /// - Release regular key A
        /// - Release OSM LShift
        ///
        /// Expected:
        /// - LShift is sent first
        /// - LShift + LCtrl
        /// _ LShift + LCtrl because of LCtrl pressed again
        /// - Only LShift is sent after LCtrl pressed again
        /// - A sent with LShift because LShift is held
        /// - Only LShift is being held
        /// - All released
        #[test]
        fn test_osm_activate_on_keypress_and_send_on_second_press_multiple() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_one_shot_modifiers_config(OneShotModifiersConfig {
                    activate_on_keypress: true,
                    send_on_second_press: true,
                    ..OneShotModifiersConfig::default()
                }),
                sequence: [
                    [0, 0, true, 10],   // Press   OSM LShift
                    [0, 4, true, 10],   // Press   OSM LCtrl
                    [0, 4, false, 10],  // Release OSM LCtrl
                    [0, 4, true, 10],   // Press   OSM LCtrl
                    [0, 4, false, 10],  // Release OSM LCtrl
                    [0, 2, true, 10],   // Press   A
                    [0, 2, false, 10],  // Release A
                    [0, 0, false, 10],  // Release OSM LShift
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Shift is sent from beginning
                    [KC_LSHIFT | KC_LCTRL, [0, 0, 0, 0, 0, 0]], // Ctrl is added to combination
                    [KC_LSHIFT | KC_LCTRL, [0, 0, 0, 0, 0, 0]], // Ctrl is pressed again, so send it again
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Shift is still being pressed
                    [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // A with Shift only
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
                keyboard: create_test_keyboard_with_behavior_config(
                    BehaviorConfig {
                        one_shot: OneShotConfig {
                            timeout: Duration::from_millis(100),
                            ..OneShotConfig::default()
                        },
                        one_shot_modifiers: OneShotModifiersConfig {
                            ..OneShotModifiersConfig::default()
                        },
                        ..BehaviorConfig::default()
                    }
                ),
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
                keyboard: create_test_keyboard_with_behavior_config(
                    BehaviorConfig {
                        one_shot: OneShotConfig {
                            timeout: Duration::from_millis(100),
                            ..OneShotConfig::default()
                        },
                        one_shot_modifiers: OneShotModifiersConfig {
                            ..OneShotModifiersConfig::default()
                        },
                        ..BehaviorConfig::default()
                    }
                ),
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
    }
}
