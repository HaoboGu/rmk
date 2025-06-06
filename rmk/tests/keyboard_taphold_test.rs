extern crate rmk;

use embassy_time::Duration;
use rmk::config::TapHoldConfig;

mod common;
pub(crate) use crate::common::*;

// Init logger for tests
#[ctor::ctor]
pub fn init_log() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(true)
        .try_init();
}

/**
* hrm config
*/
fn get_th_config_for_permissive_hold_test() -> TapHoldConfig {
    TapHoldConfig {
        enable_hrm: true,
        permissive_hold: true,
        post_wait_time: Duration::from_millis(0),
        ..TapHoldConfig::default()
    }
}
/**
* hrm config
*/
fn get_th_config_for_test() -> TapHoldConfig {
    TapHoldConfig {
        enable_hrm: true,
        permissive_hold: false,
        chordal_hold: true,
        post_wait_time: Duration::from_millis(0),
        ..TapHoldConfig::default()
    }
}

mod tap_hold_test {

    use super::*;
    use embassy_futures::block_on;
    use embassy_time::Duration;
    use rmk::{
        config::{BehaviorConfig, TapHoldConfig},
        k,
        keyboard::Keyboard,
        keycode::KeyCode,
        keymap::KeyMap,
        th,
    };
    use rusty_fork::rusty_fork_test;
    use std::cell::RefCell;

    rusty_fork_test! {

        #[test]
        fn test_taphold_tap() {
            let main = async {
                let mut keyboard = create_test_keyboard();

                let sequence = key_sequence![
                    [2, 1, true, 10],  // Press TH shift A
                    //release before hold timeout
                    [2, 1, false, 100], // Release A
                ];

                let expected_reports = key_report![
                    //should be a tapping A
                    [0, [0x04, 0, 0, 0, 0, 0]],
                ];

                run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
            };
            block_on(main);
        }


        #[test]
        fn test_taphold_hold() {
            let main = async {
                let mut keyboard = create_test_keyboard();

                let sequence = key_sequence![
                    [2, 1, true, 10],  // Press TH shift A
                    [2, 1, false, 300], // Release A
                ];

                let expected_reports = key_report![
                    //tap on a
                    [2, [0, 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ];

                run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
            };
            block_on(main);
        }

        #[test]
        fn test_tap_hold_key_post_wait_in_new_version_1() {
            block_on( async {
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
                        ]]]
                        ,
                        config
                    );
                let mut keyboard = Keyboard::new(keymap);

                println!("first case");

                let sequence = key_sequence![
                    [0, 0, true, 10],  // press th b
                    [0, 1, true, 10],  // Press a 
                    [0, 0, false, 300], // Release th b
                    [0, 1, false, 10],  // Press a within post wait timeout

                ];

                let expected_reports = key_report![
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [ kc8!(A) , 0, 0, 0, 0, 0]],
                    [0, [ kc8!(A) , 0, 0, 0, 0, 0]],
                    [0, [ 0, 0, 0, 0, 0, 0]],

                ];

                run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;

            });
        }
        #[test]
        fn test_tap_hold_key_post_wait_in_new_version_2() {
            block_on( async {
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
                        ]]]
                        ,
                        config
                    );
                let mut keyboard = Keyboard::new(keymap);


                println!("second case");

                let sequence = key_sequence![
                    [0, 0, true, 10],  // press th b
                    [0, 1, true, 10],  // Press a
                    [0, 0, false, 300], // Release th b
                    [0, 1, false, 100],  // Press a out of post wait timeout
                ];

                let expected_reports = key_report![

                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [kc8!(A), 0, 0, 0, 0, 0]],
                    [0, [kc8!(A), 0, 0, 0, 0, 0]],
                    [0, [ 0, 0, 0, 0, 0, 0]],
                ];

                run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
            });
        }

        #[test]
        fn test_tap_hold_key_multi_hold() {
            let main = async {
                let mut keyboard = create_test_keyboard();

                let sequence = key_sequence![
                    [2, 1, true, 10], // Press th!(A,shift)
                    [2, 2, true, 10], //  press th!(S,lgui)
                    //hold timeout
                    [2, 3, true, 270],  //  press d
                    [2, 3, false, 290], // release d
                    [2, 1, false, 380], // Release A
                    [2, 2, false, 400], // Release s
                ];
                let expected_reports = key_report![
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],                          //shift
                    [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]],                //shift
                    [KC_LSHIFT | KC_LGUI, [KeyCode::D as u8, 0, 0, 0, 0, 0]], // 0x7
                    [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]],                //shift
                    [KC_LGUI, [0, 0, 0, 0, 0, 0]],                            //shift and gui
                    [0, [0, 0, 0, 0, 0, 0]],
                ];

                run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
            };
            block_on(main);
        }


        //normal tap hold tests
        #[test]
        fn test_tap_hold_key_release_rolling_should_tap_in_order() {
            // eager hold
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(BehaviorConfig {
                    //perfer hold
                    tap_hold:get_th_config_for_permissive_hold_test(),
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

                run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
            };
            block_on(main);
        }


        //permissive hold test cases
        #[test]
        fn test_tap_hold_hold_on_other_release() {
                // eager hold
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: get_th_config_for_permissive_hold_test(),
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

                run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;

            };
            block_on(main);
        }

        #[test]
        fn test_tap_hold_hold_on_smesh_key_press() {
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: get_th_config_for_permissive_hold_test(),
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

                run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
            };
            block_on(main);
        }

        #[test]
        fn test_tap_hold_key_mixed_release_hold() {
                // eager hold
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: get_th_config_for_permissive_hold_test(),
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

                run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
            };
            block_on(main);
        }

        #[test]
        fn test_tap_hold_key_chord_cross_hand_should_be_hold() {
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(
                    BehaviorConfig {
                        tap_hold: get_th_config_for_permissive_hold_test(),
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

                run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
            };
            block_on(main);
        }


        #[test]
        fn test_tap_hold_key_chord_reversed_cross_tap_should_be_tap() {
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(
                    BehaviorConfig {
                        tap_hold: get_th_config_for_permissive_hold_test(),
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

                run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
            };
            block_on(main);
        }

        #[test]
        fn test_chordal_cross_hand_flow_tap_should_tap() {
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(
                    BehaviorConfig {
                        tap_hold: get_th_config_for_permissive_hold_test(),
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

                run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
            };
            block_on(main);
        }

        #[test]
        fn test_chordal_reversed_rolling_should_tap() {
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(
                    BehaviorConfig {
                        tap_hold: get_th_config_for_permissive_hold_test(),
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

                run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
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
                        tap_hold: get_th_config_for_permissive_hold_test(),
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

                run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
            };
            block_on(main);
        }

        #[test]
        fn test_chordal_multi_hold_key_cross_hand_should_be_hold() {
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(
                    BehaviorConfig {
                        tap_hold: get_th_config_for_test(),
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

                run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
            };
            block_on(main);
        }

        #[test]
        fn test_tap_timeout() {
            let main = async {
                let mut keyboard = create_test_keyboard_with_config(
                    BehaviorConfig {
                        tap_hold: get_th_config_for_test(),
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

                run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
            };
            block_on(main);
        }

    } // forks end
}
