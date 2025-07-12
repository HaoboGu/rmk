pub mod common;

use embassy_time::Duration;
use rmk::config::TapHoldConfig;

fn tap_hold_config_with_hrm_and_permissive_hold() -> TapHoldConfig {
    TapHoldConfig {
        enable_hrm: true,
        permissive_hold: true,
        chordal_hold: false,
        hold_on_other_press: false,
        post_wait_time: Duration::from_millis(0),
        ..TapHoldConfig::default()
    }
}

fn tap_hold_config_with_hrm_and_chordal_hold() -> TapHoldConfig {
    TapHoldConfig {
        enable_hrm: true,
        chordal_hold: true,
        permissive_hold: true,
        hold_on_other_press: false,
        post_wait_time: Duration::from_millis(0),
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
    use crate::common::{create_test_keyboard, create_test_keyboard_with_config, wrap_keymap, KC_LGUI, KC_LSHIFT};

    rusty_fork_test! {

        #[test]
        fn test_taphold_tap() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),

                sequence: [
                    [2, 1, true, 10],  // Press TH shift A
                    // Release before hold timeout
                    [2, 1, false, 100], // Release A
                ],

                expected_reports: [
                    // Should be a tapping A
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                ]
            };
        }

        #[test]
        fn test_taphold_hold() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [2, 1, true, 10],  // Press th!(A, LShift)
                    [2, 1, false, 300], // Release A after hold timeout
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold LShift
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        #[test]
        fn test_tap_hold_permissive_hold_timeout_and_release() {
        key_sequence_test! {
            keyboard: create_test_keyboard_with_config(BehaviorConfig {
                tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                .. BehaviorConfig::default()
            }),
            sequence: [
                [2, 1, true, 10], // Press th!(A, lshift)
                [2, 3, true, 200],  // Press D
                [2, 3, false, 100], // Release D  <-- Release D after "permissive hold" interval, but also after the hold-timeout
                [2, 1, false, 100], // Release A
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold LShift
                [KC_LSHIFT, [kc_to_u8!(D), 0, 0, 0, 0, 0]], // Press D
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Release D
                [0, [0, 0, 0, 0, 0, 0]], // All released
            ]
            };
        }


        #[test]
        fn test_tap_hold_permissive_hold_timeout_and_release_2() {
        key_sequence_test! {
            keyboard: create_test_keyboard_with_config(BehaviorConfig {
                tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                .. BehaviorConfig::default()
            }),
            sequence: [
                [2, 1, true, 10], // Press th!(A, lshift)
                [2, 2, true, 200],  // Press th!(S,lgui)
                [2, 2, false, 100], // Release S  <-- Release S after "permissive hold" interval, but also after the hold-timeout
                [2, 1, false, 100], // Release A
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold LShift
                [KC_LSHIFT, [kc_to_u8!(S), 0, 0, 0, 0, 0]], // Press S
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Release S
                [0, [0, 0, 0, 0, 0, 0]], // All released
            ]
            };
        }

        #[test]
        fn test_tap_hold_key_post_wait_in_new_version_1() {
            let config =BehaviorConfig {
                tap_hold: TapHoldConfig {
                    enable_hrm: true,
                    permissive_hold: true,
                    post_wait_time: Duration::from_millis(0),
                    ..Default::default()
                },
                ..Default::default()
            };
            let keymap:&mut RefCell<KeyMap<1, 2, 1>> = wrap_keymap(
                [[[
                    th!(B, LShift),
                    k!(A)
                ]]],
                config
            );
            key_sequence_test! {
                keyboard: Keyboard::new(keymap),
                sequence: [
                    [0, 0, true, 10],  // press th b
                    [0, 1, true, 10],  // Press a
                    [0, 0, false, 300], // Release th b
                    [0, 1, false, 10],  // Press a within post wait timeout

                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [ kc_to_u8!(A) , 0, 0, 0, 0, 0]],
                    [0, [ kc_to_u8!(A) , 0, 0, 0, 0, 0]],
                    [0, [ 0, 0, 0, 0, 0, 0]],
                ]
            }
        }

        #[test]
        fn test_tap_hold_key_post_wait_in_new_version_2() {
            let config = BehaviorConfig {
                tap_hold: TapHoldConfig {
                    enable_hrm: true,
                    permissive_hold: true,
                    post_wait_time: Duration::from_millis(0),
                    ..Default::default()
                },
                ..Default::default()
            };
            let keymap:&mut RefCell<KeyMap<1, 2, 1>> = wrap_keymap(
                [[[
                    th!(B, LShift),
                    k!(A)
                ]]],
                config
            );

            key_sequence_test! {
                keyboard: Keyboard::new(keymap),
                sequence: [
                    [0, 0, true, 10],  // press th b
                    [0, 1, true, 10],  // Press a
                    [0, 0, false, 300], // Release th b
                    [0, 1, false, 100],  // Press a out of post wait timeout
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                    [0, [ 0, 0, 0, 0, 0, 0]],
                ]
            }
        }

        #[test]
        fn test_tap_hold_key_multi_hold() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [2, 1, true, 10], // Press th!(A, lshift)
                    [2, 2, true, 10], // Press th!(S, lgui)
                    [2, 3, true, 270],  // Press D (after hold timeout)
                    [2, 3, false, 290], // Release D
                    [2, 1, false, 380], // Release A
                    [2, 2, false, 400], // Release S
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold LShift
                    [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]], // Hold LShift + LGui
                    [KC_LSHIFT | KC_LGUI, [kc_to_u8!(D), 0, 0, 0, 0, 0]], // Press D
                    [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]], // Release D
                    [KC_LGUI, [0, 0, 0, 0, 0, 0]], // Hold LGui
                    [0, [0, 0, 0, 0, 0, 0]], // All released
                ]
            };
        }

        // Normal tap hold tests
        #[test]
        fn test_tap_hold_key_release_rolling_should_tap_in_order() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold:tap_hold_config_with_hrm_and_permissive_hold(),
                    .. BehaviorConfig::default()
                }),
                sequence: [
                    [2, 1, true, 10], // Press th!(A,shift)
                    [2, 2, true, 30], // Press th!(S,lgui)
                    [2, 3, true, 30], // Press D
                    [2, 1, false, 50], // Release A
                    [2, 2, false, 100], // Release s
                    [2, 3, false, 100],  //  press d
                ],
                expected_reports: [
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                    [0, [kc_to_u8!(A), kc_to_u8!(S), 0, 0, 0, 0]],
                    [0, [kc_to_u8!(A), kc_to_u8!(S), kc_to_u8!(D), 0, 0, 0]],
                    [0, [0, kc_to_u8!(S), kc_to_u8!(D), 0, 0, 0]],
                    [0, [0, 0, kc_to_u8!(D), 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }

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

        // Hold after tapping
        #[test]
        fn test_tap_hold_hold_after_tapping() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                    .. BehaviorConfig::default()
                }),
                sequence: [
                    [2, 1, true, 10], // Tap th!(A,shift)
                    [2, 1, false, 50],
                    // last release should record as tap
                    [2, 1, true, 100], // Hold th!(A,shift) after tapping
                    [2, 1, false, 400],
                ],
                expected_reports: [
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Tap A
                    [0, [0, 0, 0, 0, 0, 0]], // Release A
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Hold after tapping
                    [0, [0, 0, 0, 0, 0, 0]], // Release A
                ]
            }
        }

        // Hold after tapping timeout
        #[test]
        fn test_tap_hold_hold_after_tapping_timeout() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                    .. BehaviorConfig::default()
                }),
                sequence: [
                    [2, 1, true, 10], // Tap th!(A,shift)
                    [2, 1, false, 50],
                    [2, 1, true, 300], // Hold th!(A,shift) after tapping timeout
                    [2, 1, false, 400],
                ],
                expected_reports: [
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Tap A
                    [0, [0, 0, 0, 0, 0, 0]], // Release A
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // Hold after tapping timeout
                    [0, [0, 0, 0, 0, 0, 0]], // Release A
                ]
            }
        }

        #[test]
        fn test_tap_hold_hold_on_smesh_key_press() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                        .. BehaviorConfig::default()
                    }),

                sequence:[
                    [2, 5, true, 30], // Press G
                    [2, 1, true, 10], // Press th!(A,shift)
                    [2, 5, false, 10], // Release G
                    [3, 3, true, 10], // Press C
                    [2, 2, true, 30], // Press th!(S,lgui)
                    [3, 3, false, 10], // Release C
                    [2, 1, false, 10], // Release A
                    [2, 2, false, 100], // Release S
                ],

                expected_reports: [
                    [0, [kc_to_u8!(G), 0, 0, 0, 0, 0]], // #0
                    [0, [kc_to_u8!(G), kc_to_u8!(A),  0, 0, 0, 0]],
                    [0, [0, kc_to_u8!(A),  0, 0, 0, 0]],
                    [0, [kc_to_u8!(C), kc_to_u8!(A), 0, 0, 0, 0]], // #0
                    [0, [kc_to_u8!(C), kc_to_u8!(A), kc_to_u8!(S), 0, 0, 0]], // #0
                    // key release trigger a hold
                    //should not trigger hold , if all prefix key is not mod key
                    [0, [0, kc_to_u8!(A), kc_to_u8!(S), 0, 0, 0]], // #0
                    [0, [0, 0, kc_to_u8!(S), 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }

        #[test]
        fn test_tap_hold_key_mixed_release_hold() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                    .. BehaviorConfig::default()
                }),
                sequence: [
                    [2, 1, true, 10], // Press th!(A,shift)
                    [2, 2, true, 30], // Press th!(S,lgui)
                    [2, 3, true, 30], // Press D
                    [2, 1, false, 50], // Release A
                    [2, 2, false, 20], // Release s
                    [2, 3, false, 10],  // Release d
                ],
                expected_reports: [
                    [0,  [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                    [0,  [kc_to_u8!(A), kc_to_u8!(S), 0, 0, 0, 0]],
                    [0,  [kc_to_u8!(A), kc_to_u8!(S), kc_to_u8!(D), 0, 0, 0]],
                    [0,  [0, kc_to_u8!(S), kc_to_u8!(D), 0, 0, 0]],
                    [0,  [0, 0, kc_to_u8!(D), 0, 0, 0]],
                    [0,  [0, 0, 0, 0, 0, 0]],
                ]
            };
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
        fn test_tap_hold_key_chord_reversed_cross_tap_should_be_tap() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                    ..BehaviorConfig::default()
                }),
                sequence: [
                    [2, 8, true, 50],  // Press K
                    [2, 1, true, 20], // Press th!(A,shift)
                    [2, 1, false, 20], // Release A
                    [2, 8, false, 50], // Release K
                ],
                expected_reports: [
                    [0, [kc_to_u8!(K), 0, 0, 0, 0, 0]], // Tap K
                    [0, [kc_to_u8!(K), kc_to_u8!(A), 0, 0, 0, 0]], // Tap A
                    [0, [kc_to_u8!(K), 0, 0, 0, 0, 0]], // Release A
                    [0, [0, 0, 0, 0, 0, 0]], // Release K
                ]
            }
        }

        #[test]
        fn test_chordal_cross_hand_flow_tap_should_tap() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                    ..BehaviorConfig::default()
                }),
                sequence: [
                    [2, 8, true, 50],  // Press K
                    [2, 8, false, 50],  // Release K
                    [2, 1, true, 20], // Press th!(A,shift)
                    [2, 2, true, 20], // Press th!(S,)
                    [2, 1, false, 20], // Release A
                    [2, 2, false, 20], // Release S
                ],
                expected_reports: [
                    [0, [kc_to_u8!(K), 0, 0, 0, 0, 0]], // Tap K
                    [0, [0, 0, 0, 0, 0, 0]], // Release K
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Tap A
                    [0, [kc_to_u8!(A), kc_to_u8!(S), 0, 0, 0, 0]], // Tap S
                    [0, [0, kc_to_u8!(S), 0, 0, 0, 0]], // Release A
                    [0, [0, 0, 0, 0, 0, 0]], // Release S
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
        fn test_taphold_with_layer_tap() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                    ..BehaviorConfig::default()
                }),
                sequence: [
                    [4, 5, true, 10],  // Press lt!(1, Space)
                    [3, 2, true, 10],
                    [3, 2, false, 10],  // Press shifted x
                    [4, 5, false, 10],  // Release lt!(1, Space)
                    [0, 1, true, 10],  // Press 1
                    [0, 1, false, 10],  // Release 1
                    [4, 5, true, 250],  // Press lt!(1, Space)
                    [3, 2, true, 10],
                    [3, 2, false, 10],  // Press shifted x
                    [4, 5, false, 10],  // Release lt!(1, Space)
                ],
                expected_reports: [
                    [KC_LSHIFT, [kc_to_u8!(X), 0, 0, 0, 0, 0]], // Shifted X
                    [0, [0, 0, 0, 0, 0, 0]], // Release Shifted X
                    [0, [kc_to_u8!(Kc1), 0, 0, 0, 0, 0]], // Shifted X
                    [0, [0, 0, 0, 0, 0, 0]], // Release Shifted X
                    [KC_LSHIFT, [kc_to_u8!(X), 0, 0, 0, 0, 0]], // Shifted X
                    [0, [0, 0, 0, 0, 0, 0]], // Release Shifted X
                ]
            }
        }

        #[test]
        fn test_taphold_rolling_with_layer_tap() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                    ..BehaviorConfig::default()
                }),
                sequence: [
                    [4, 5, true, 10],  // Press lt!(1, Space)
                    [3, 2, true, 10],
                    [4, 5, false, 100],  // Release lt!(1, Space)
                    [3, 2, false, 10],  // Release shifted x
                    [4, 5, true, 250],  // Press lt!(1, Space)
                    [3, 2, true, 10],
                    [3, 2, false, 10],  // Release shifted x
                    [4, 5, false, 100],  // Release lt!(1, Space)
                    [4, 5, true, 250],  // Press lt!(1, Space)
                    [3, 2, true, 10],
                    [4, 5, false, 100],  // Release lt!(1, Space)
                    [3, 2, false, 10],  // Release shifted x
                ],
                expected_reports: [
                    [0, [kc_to_u8!(Space), 0, 0, 0, 0, 0]], // Space
                    [0, [kc_to_u8!(Space), kc_to_u8!(X), 0, 0, 0, 0]], // Space + X
                    [0, [0, kc_to_u8!(X), 0, 0, 0, 0]], // Release Space
                    [0, [0, 0, 0, 0, 0, 0]], // Release X
                    [KC_LSHIFT, [kc_to_u8!(X), 0, 0, 0, 0, 0]], // Shifted X
                    [0, [0, 0, 0, 0, 0, 0]], // Release Shifted X
                    [0, [kc_to_u8!(Space), 0, 0, 0, 0, 0]], // Space
                    [0, [kc_to_u8!(Space), kc_to_u8!(X), 0, 0, 0, 0]], // Space + X
                    [0, [0, kc_to_u8!(X), 0, 0, 0, 0]], // Release Space
                    [0, [0, 0, 0, 0, 0, 0]], // Release X
                ]
            }
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

        #[test]
        fn test_taphold_flow_tap() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(
                    BehaviorConfig {
                        tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                        ..BehaviorConfig::default()
                    }
                ),

                sequence: [
                    [2, 3, true, 30],  // Press d
                    [2, 3, false, 30], // Release d
                    [2, 1, true, 20],  // Press th!(A,shift)
                    [2, 2, true, 10],  // Press th!(S,LGui)
                    [2, 1, false, 40], // Release A
                    [2, 2, false, 10], // Release S
                ],
                expected_reports: [
                    [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]], // Tap d
                    [0, [0, 0, 0, 0, 0, 0]], // Release D
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // quick tapping, Tap A
                    [0, [kc_to_u8!(A), kc_to_u8!(S), 0, 0, 0, 0]], // quick taping
                    [0, [0, kc_to_u8!(S), 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }

        #[test]
        fn test_taphold_with_combo() {
            key_sequence_test! {
                keyboard: {
                    let behavior_config = BehaviorConfig {
                        tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                        combo: CombosConfig {
                            combos: heapless::Vec::from_iter([
                                Combo::new(
                                    [th!(A, LShift), th!(S, LGui)],
                                    k!(B),
                                    None,
                                )
                            ]),
                            timeout: Duration::from_millis(50),
                        },
                        ..BehaviorConfig::default()
                    };
                    create_test_keyboard_with_config(behavior_config)
                },
                sequence: [
                    [2, 1, true, 20],  // Press th!(A,shift)
                    [2, 2, true, 60],  // Press th!(S,LGui)
                    [2, 2, false, 10], // Release S
                    [2, 1, false, 300], // Release A
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [kc_to_u8!(S), 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }

        #[test]
        fn test_taphold_with_combo_2() {
            key_sequence_test! {
                keyboard: {
                    let behavior_config = BehaviorConfig {
                        tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                        combo: CombosConfig {
                            combos: heapless::Vec::from_iter([
                                Combo::new(
                                    [th!(A, LShift), th!(S, LGui)],
                                    k!(B),
                                    None,
                                )
                            ]),
                            timeout: Duration::from_millis(50),
                        },
                        ..BehaviorConfig::default()
                    };
                    create_test_keyboard_with_config(behavior_config)
                },
                sequence: [
                    [2, 1, true, 20],  // Press th!(A,shift)
                    [2, 2, true, 60],  // Press th!(S,LGui)
                    [2, 1, false, 10], // Release A
                    [2, 2, false, 10], // Release S
                ],
                expected_reports: [
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                    [0, [kc_to_u8!(A), kc_to_u8!(S), 0, 0, 0, 0]],
                    [0, [0, kc_to_u8!(S), 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }

        #[test]
        fn test_taphold_with_combo_3() {
            key_sequence_test! {
                keyboard: {
                    let behavior_config = BehaviorConfig {
                        tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                        combo: CombosConfig {
                            combos: heapless::Vec::from_iter([
                                Combo::new(
                                    [th!(A, LShift), th!(S, LGui), th!(Z, LAlt)],
                                    k!(C),
                                    None,
                                )
                            ]),
                            timeout: Duration::from_millis(50),
                        },
                        ..BehaviorConfig::default()
                    };
                    create_test_keyboard_with_config(behavior_config)
                },
                sequence: [
                    [2, 1, true, 20],  // Press th!(A,shift)
                    [2, 2, true, 20],  // Press th!(S,LGui)
                    [3, 1, true, 100],  // Press th!(Z,LAlt)
                    [2, 1, false, 10], // Release A
                    [2, 2, false, 10], // Release S
                    [3, 1, false, 10], // Release Z
                ],
                expected_reports: [
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                    [0, [kc_to_u8!(A), kc_to_u8!(S), 0, 0, 0, 0]],
                    [0, [kc_to_u8!(A), kc_to_u8!(S), kc_to_u8!(Z), 0, 0, 0]],
                    [0, [0, kc_to_u8!(S), kc_to_u8!(Z), 0, 0, 0]],
                    [0, [0, 0, kc_to_u8!(Z), 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }

    }
}
