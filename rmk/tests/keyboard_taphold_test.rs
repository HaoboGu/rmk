pub mod common;

use embassy_time::Duration;
use rmk::config::{MorseKeyMode, TapHoldConfig};

fn tap_hold_config_with_hrm_and_permissive_hold() -> TapHoldConfig {
    TapHoldConfig {
        enable_hrm: true,
        mode: MorseKeyMode::PermissiveHold,
        chordal_hold: false,
        ..TapHoldConfig::default()
    }
}

fn tap_hold_config_with_hrm_and_chordal_hold() -> TapHoldConfig {
    TapHoldConfig {
        enable_hrm: true,
        chordal_hold: true,
        mode: MorseKeyMode::PermissiveHold,
        ..TapHoldConfig::default()
    }
}

mod tap_hold_test {

    use std::cell::RefCell;

    use embassy_futures::block_on;
    use rmk::combo::Combo;
    use rmk::config::{BehaviorConfig, CombosConfig};
    use rmk::keyboard::Keyboard;
    use rmk::keymap::KeyMap;
    use rmk::{k, th};
    use rusty_fork::rusty_fork_test;

    use super::*;
    use crate::common::{KC_LGUI, KC_LSHIFT, create_test_keyboard, create_test_keyboard_with_config, wrap_keymap};

    rusty_fork_test! {
        //permissive hold test cases
        #[test]
        fn test_tap_hold_hold_on_other_release() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                    .. BehaviorConfig::default()
                }),
                sequence: [
                    [2, 1, true, 10], // Press th!(A,shift)
                    [2, 2, true, 30], //  press th!(S,lgui)
                    [2, 3, true, 30],  //  press d
                    [2, 3, false, 10],  // Release d, active permissive hold
                    [2, 1, false, 50], // Release A
                    [2, 2, false, 100], // Release S
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold LShift
                    [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]], // Hold LShift + LGui
                    [KC_LSHIFT | KC_LGUI,  [kc_to_u8!(D), 0, 0, 0, 0, 0]], // Press D
                    [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]], // Release D
                    [KC_LGUI, [ 0, 0, 0, 0, 0, 0]], // Hold LGui
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            }
        }

        #[test]
        fn test_tap_hold_key_chord_cross_hand_should_be_hold() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(
                    BehaviorConfig {
                        tap_hold: tap_hold_config_with_hrm_and_chordal_hold(),
                        ..BehaviorConfig::default()
                    }
                ),

                // rolling A , then ctrl d
                sequence: [
                    [2, 1, true, 20], // Press th!(A,shift)
                    [2, 8, true, 50],  // Press K
                    [2, 8, false, 50],  //Release K
                    [2, 1, false, 20], // Release A

                ],
                expected_reports: [
                    // chord hold , should become (shift x)
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [kc_to_u8!(K), 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],

                ]
            }
        }

        #[test]
        fn test_chordal_reversed_rolling_should_tap() {
            key_sequence_test!  {
                keyboard: create_test_keyboard_with_config(
                    BehaviorConfig {
                        tap_hold: tap_hold_config_with_hrm_and_chordal_hold(),
                        ..BehaviorConfig::default()
                    }
                ),

                // rolling A , then ctrl d
                sequence: [
                    [2, 8, true, 50],  // Press K
                    [2, 1, true, 20],  // Press th!(A,shift)
                    [2, 8, false, 50], // Release k
                    [2, 1, false, 20], // Release A

                ],
                expected_reports: [
                    [0, [kc_to_u8!(K), 0, 0, 0, 0, 0]],  // Tap K
                    [0, [kc_to_u8!(K), kc_to_u8!(A),  0, 0, 0, 0]], // Rolling Tap A
                    [0, [0, kc_to_u8!(A),  0, 0, 0, 0]], // Release K
                    [0, [0, 0, 0, 0, 0, 0]],  // Relase Tap K

                ]
            }
        }

        #[test]
        fn test_chordal_same_hand_quick_pressing_should_be_tap() {
            //core case
            //should buffer next key and output
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(
                    BehaviorConfig {
                        tap_hold: tap_hold_config_with_hrm_and_chordal_hold(),
                        ..BehaviorConfig::default()
                    }
                ),

                // rolling A , then ctrl d
                sequence: [
                    [2, 1, true, 20], // Press th!(A,shift)
                    [2, 5, true, 50],  // Press g
                    [2, 1, false, 20], // Release A
                    [2, 5, false, 50],  // Release g

                ],
                expected_reports: [
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                    [0, [kc_to_u8!(A),kc_to_u8!(G), 0, 0, 0, 0]],
                    [0, [0 ,kc_to_u8!(G), 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }

        #[test]
        fn test_chordal_multi_hold_key_cross_hand_should_be_hold() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hrm_and_chordal_hold(),
                    ..BehaviorConfig::default()
                }),
                sequence: [
                    [2, 1, true, 20],  // Press th!(A,shift)
                    [2, 2, true, 10],   // Press th!(S,lgui)
                    [2, 8, true, 20],   // Press K
                    [2, 8, false, 20],  // Release K should trigger permissive hold
                    [2, 1, false, 20],  // Release A
                    [2, 2, false, 20], // Release S
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold LShift
                    [KC_LSHIFT| KC_LGUI, [0, 0, 0, 0, 0, 0]], // Hold LShift + LGui
                    [KC_LSHIFT| KC_LGUI, [kc_to_u8!(K), 0, 0, 0, 0, 0]], // Press K
                    [KC_LSHIFT|KC_LGUI , [0, 0, 0, 0, 0, 0]], // Release K
                    [KC_LGUI, [0, 0, 0, 0, 0, 0]], // Release S
                    [0, [0, 0, 0, 0, 0, 0]], // Release A
                ]
            }
        }

        // Ref: https://github.com/HaoboGu/rmk/pull/496
        #[test]
        fn test_taphold_with_previous_rolling_keypress() {
            key_sequence_test! {
                keyboard: {
                    let behavior_config = BehaviorConfig {
                        tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                        ..BehaviorConfig::default()
                    };
                    create_test_keyboard_with_config(behavior_config)
                },
                sequence: [
                    [1, 1, true, 20],  // Press Q
                    [4, 5, true, 150],  // Press lt!(1, space)
                    [1, 1, false, 20],  // Release Q
                    [1, 3, true, 10], // Press E(on layer 1 is W)
                    [1, 3, false, 10], // Release E(should trigger W)
                    [4, 5, false, 10], // Release lt!(1, space)
                ],
                expected_reports: [
                    [0, [kc_to_u8!(Q), 0, 0, 0, 0, 0]], // Tap Q
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                    [0, [kc_to_u8!(W), 0, 0, 0, 0, 0]], // Tap W
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }
    }
}
