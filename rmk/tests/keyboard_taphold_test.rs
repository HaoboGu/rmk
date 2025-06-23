pub mod common;

use embassy_time::Duration;
use rmk::config::TapHoldConfig;

fn tap_hold_config_with_hrm_and_permissive_hold() -> TapHoldConfig {
    TapHoldConfig {
        enable_hrm: true,
        permissive_hold: true,
        post_wait_time: Duration::from_millis(0),
        ..TapHoldConfig::default()
    }
}

fn tap_hold_config_with_hrm_and_chordal_hold() -> TapHoldConfig {
    TapHoldConfig {
        enable_hrm: true,
        chordal_hold: true,
        post_wait_time: Duration::from_millis(0),
        ..TapHoldConfig::default()
    }
}

mod tap_hold_test {

    use embassy_futures::block_on;
    use rmk::config::BehaviorConfig;
    use rusty_fork::rusty_fork_test;

    use super::*;
    use crate::common::{
        create_test_keyboard, create_test_keyboard_with_config, run_key_sequence_test, KC_LGUI, KC_LSHIFT,
    };

    rusty_fork_test! {
        #[test]
        fn test_taphold_tap() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [2, 1, true, 10],  // Press TH shift A
                    [2, 1, false, 100], // Release A before hold timeout
                ],
                expected_reports: [
                    [0, [0x04, 0, 0, 0, 0, 0]], // Should be a tapping A
                ]
            }
        }

        #[test]
        fn test_taphold_hold() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [2, 1, true, 10],  // Press th!(A, LShift)
                    [2, 1, false, 300], // Release A
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // LShift
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }

        #[test]
        fn test_tap_hold_key_multi_hold() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [2, 1, true, 10], // Press th!(A, lshift)
                    [2, 2, true, 10], // Press th!(S, lgui)
                    [2, 3, true, 270],  // Press D after hold timeout
                    [2, 3, false, 290], // Release D
                    [2, 1, false, 380], // Release A
                    [2, 2, false, 400], // Release S
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT | KC_LGUI, [kc8!(D), 0, 0, 0, 0, 0]],
                    [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]],
                    [KC_LGUI, [0, 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }

        //normal tap hold tests
        #[test]
        fn test_tap_hold_key_release_rolling_should_tap_in_order() {
            // eager hold
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(BehaviorConfig {
                    //perfer hold
                    tap_hold:tap_hold_config_with_hrm_and_permissive_hold(),
                        .. BehaviorConfig::default()
                    });

                let sequence = key_sequence![
                    [2, 1, true, 10], // Press th!(A,shift)
                    [2, 2, true, 30], //  press th!(S,lgui)
                    [2, 3, true, 30],  //  press d
                    // eager hold and output
                    [2, 1, false, 50], // Release A
                    [2, 2, false, 100], // Release s
                    [2, 3, false, 100],  //  press d
                ];
                let expected_reports = key_report![
                    [0, [kc8!(A), 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                    [0, [kc8!(S), 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                    [0, [kc8!(D), 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ];

                run_key_sequence_test(&mut keyboard, &sequence, &expected_reports).await;
            };
            block_on(main);
        }

        //permissive hold test cases
        #[test]
        fn test_tap_hold_hold_on_other_release() {
                // eager hold
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                        .. BehaviorConfig::default()
                    });

                let sequence = key_sequence![
                    [2, 1, true, 10], // Press th!(A,shift)
                    [2, 2, true, 30], //  press th!(S,lgui)
                    [2, 3, true, 30],  //  press d
                    [2, 3, false, 10],  // Release d
                    // eager hold and output

                    [2, 1, false, 50], // Release A
                    [2, 2, false, 100], // Release s
                ];

                let expected_reports = key_report![
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // #0
                    [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT | KC_LGUI,  [kc8!(D), 0, 0, 0, 0, 0]],
                    [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]],
                    [KC_LGUI, [ 0, 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ];

                run_key_sequence_test(&mut keyboard, &sequence, &expected_reports).await;

            };
            block_on(main);
        }

        #[test]
        fn test_tap_hold_hold_on_smesh_key_press() {
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                        .. BehaviorConfig::default()
                    });

                let sequence = key_sequence![

                    //tap
                    [2, 5, true, 30], // +G

                    //buffer
                    [2, 1, true, 10], // +th!(A,shift)

                    //tap release
                    [2, 5, false, 10], // -G

                    //tap
                    [3, 3, true, 10], // +C

                    //flow tap
                    [2, 2, true, 30], // +th!(S,lgui)

                    [3, 3, false, 10],// -C
                    [2, 1, false, 10], // -A
                    [2, 2, false, 100], // -S
                ];

                let expected_reports = key_report![
                    [0, [kc8!(G), 0, 0, 0, 0, 0]], // #0
                    [0, [0, 0, 0, 0, 0, 0]], // #0
                    [0, [kc8!(A), 0, 0, 0, 0, 0]],
                    [0, [kc8!(A), kc8!(C), 0, 0, 0, 0]], // #0
                    [0, [kc8!(A), 0, 0, 0, 0, 0]], // #0
                    // key release trigger a hold
                    //should not trigger hold , if all prefix key is not mod key
                    [0, [kc8!(A), kc8!(S), 0, 0, 0, 0]], // #0
                    [0, [0, kc8!(S), 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ];

                run_key_sequence_test(&mut keyboard, &sequence, &expected_reports).await;
            };
            block_on(main);
        }

        #[test]
        fn test_tap_hold_key_mixed_release_hold() {
                // eager hold
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                        .. BehaviorConfig::default()
                    });

                let sequence = key_sequence![
                    [2, 1, true, 10], // Press th!(A,shift)
                    [2, 2, true, 30], //  press th!(S,lgui)
                    [2, 3, true, 30],  //  press d
                    // eager hold and output
                    [2, 1, false, 50], // Release A
                    [2, 2, false, 20], // Release s
                    [2, 3, false, 10],  // Release d
                ];
                let expected_reports = key_report![
                    [0,  [kc8!(A), 0, 0, 0, 0, 0]],
                    [0,  [0, 0, 0, 0, 0, 0]],
                    [0,  [kc8!(S), 0, 0, 0, 0, 0]],
                    [0,  [0, 0, 0, 0, 0, 0]],
                    [0,  [kc8!(D), 0, 0, 0, 0, 0]],
                    [0,  [0, 0, 0, 0, 0, 0]],
                ];

                run_key_sequence_test(&mut keyboard, &sequence, &expected_reports).await;
            };
            block_on(main);
        }

        #[test]
        fn test_tap_hold_key_chord_cross_hand_should_be_hold() {
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(
                    BehaviorConfig {
                        tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                        ..BehaviorConfig::default()
                    }
                );

                // rolling A , then ctrl d
                let sequence = key_sequence![
                    [2, 1, true, 200], // +th!(A,shift)
                    [2, 8, true, 50],  // +k
                    [2, 1, false, 20], // -A
                    [2, 8, false, 50],  // -k

                ];
                let expected_reports = key_report![
                    // chord hold , should become (shift x)
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [kc8!(K), 0, 0, 0, 0, 0]],
                    [0, [kc8!(K), 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],

                ];

                run_key_sequence_test(&mut keyboard, &sequence, &expected_reports).await;
            };
            block_on(main);
        }

        #[test]
        fn test_tap_hold_key_chord_reversed_cross_tap_should_be_tap() {
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(
                    BehaviorConfig {
                        tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                        ..BehaviorConfig::default()
                    }
                );

                // rolling A , then ctrl d
                let sequence = key_sequence![
                    [2, 8, true, 50],  // +k
                    [2, 1, true, 20], // +th!(A,shift)
                    [2, 1, false, 20], // -A
                    [2, 8, false, 50],  // -k

                ];

                let expected_reports = key_report![
                    // chord hold , should become (shift x)
                    [0, [kc8!(K), 0, 0, 0, 0, 0]],
                    [0, [kc8!(K), kc8!(A), 0, 0, 0, 0]],
                    [0, [kc8!(K), 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],

                ];

                run_key_sequence_test(&mut keyboard, &sequence, &expected_reports).await;
            };
            block_on(main);
        }

        #[test]
        fn test_chordal_cross_hand_flow_tap_should_tap() {
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(
                    BehaviorConfig {
                        tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                        ..BehaviorConfig::default()
                    }
                );

                // rolling A , then ctrl d
                let sequence = key_sequence![
                    [2, 8, true, 50],  // +k
                    [2, 8, false, 50],  // -k
                    [2, 1, true, 20], // +th!(A,shift)
                    [2, 2, true, 20], // +th!(S,)
                    [2, 1, false, 20], // -A
                    [2, 2, false, 20], // -S

                ];
                let expected_reports = key_report![
                    // chord hold , should become (shift x)
                    [0, [kc8!(K), 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                    [0, [kc8!(A), 0, 0, 0, 0, 0]],
                    [0, [kc8!(A), kc8!(S), 0, 0, 0, 0]],
                    [0, [0, kc8!(S), 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],

                ];

                run_key_sequence_test(&mut keyboard, &sequence, &expected_reports).await;
            };
            block_on(main);
        }

        #[test]
        fn test_chordal_reversed_rolling_should_tap() {
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(
                    BehaviorConfig {
                        tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                        ..BehaviorConfig::default()
                    }
                );

                // rolling A , then ctrl d
                let sequence = key_sequence![
                    [2, 8, true, 50],  // +k
                    [2, 1, true, 20], // +th!(A,shift)
                    [2, 8, false, 50],  // -k
                    [2, 1, false, 20], // -A

                ];
                let expected_reports = key_report![
                    // chord hold , should become (shift x)
                    [0, [kc8!(K), 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                    [0, [kc8!(A), 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],

                ];

                run_key_sequence_test(&mut keyboard, &sequence, &expected_reports).await;
            };
            block_on(main);
        }

        #[test]
        fn test_chordal_same_hand_should_be_tap() {
            //core case
            //should buffer next key and output
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(
                    BehaviorConfig {
                        tap_hold: tap_hold_config_with_hrm_and_permissive_hold(),
                        ..BehaviorConfig::default()
                    }
                );

                // rolling A , then ctrl d
                let sequence = key_sequence![
                    [2, 1, true, 200], // +th!(A,shift)
                    [2, 5, true, 50],  // +g
                    [2, 1, false, 20], // -A
                    [2, 5, false, 50],  // -g

                ];
                let expected_reports = key_report![
                    // non chord hold
                    [0, [kc8!(A), 0, 0, 0, 0, 0]], //4
                    [0, [0, 0, 0, 0, 0, 0]],
                    [0, [kc8!(G), 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ];

                run_key_sequence_test(&mut keyboard, &sequence, &expected_reports).await;
            };
            block_on(main);
        }

        #[test]
        fn test_chordal_multi_hold_key_cross_hand_should_be_hold() {
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(
                    BehaviorConfig {
                        tap_hold: tap_hold_config_with_hrm_and_chordal_hold(),
                        ..BehaviorConfig::default()
                    }
                );

                // rolling A , then ctrl d
                let sequence = key_sequence![
                    [2, 1, true, 200],  // +th!(A,shift)
                    [2, 2, true, 10], // +th!(S,lgui)
                    // cross hand , fire hold
                    [2, 8, true, 50],  // +k
                    [2, 1, false, 20], // -A
                    [2, 8, false, 50], // -k
                    [2, 2, false, 400], // -s
                ];
                let expected_reports = key_report![
                    //multi mod chord hold
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT| KC_LGUI, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT| KC_LGUI, [kc8!(K), 0, 0, 0, 0, 0]],
                    [ KC_LGUI, [kc8!(K), 0, 0, 0, 0, 0]],
                    [KC_LGUI, [0, 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ];

                run_key_sequence_test(&mut keyboard, &sequence, &expected_reports).await;
            };
            block_on(main);
        }

        #[test]
        fn test_tap_timeout() {
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(
                    BehaviorConfig {
                        tap_hold: tap_hold_config_with_hrm_and_chordal_hold(),
                        ..BehaviorConfig::default()
                    }
                );

                let sequence = key_sequence![
                    [2, 3, true, 30],  // +d
                    [2, 3, false, 30], // -d
                    // flow tapping
                    [2, 1, true, 10],  // +th!(A,shift)
                    [2, 2, true, 10],  // +th!(S,lgui)
                    [2, 1, false, 40], // -A
                    [2, 2, false, 10], // -S
                ];
                let expected_reports = key_report![
                    [0, [kc8!(D), 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                    [0, [kc8!(A), 0, 0, 0, 0, 0]],
                    [0, [kc8!(A), kc8!(S), 0, 0, 0, 0]],
                    [0, [0, kc8!(S), 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ];

                run_key_sequence_test(&mut keyboard, &sequence, &expected_reports).await;
            };
            block_on(main);
        }
    }
}
