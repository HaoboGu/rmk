pub mod common;

use embassy_time::Duration;
use heapless::Vec;
use rmk::combo::Combo;
use rmk::config::CombosConfig;
use rmk::keycode::ModifierCombination;
use rmk::{k, osm};

// Get tested combo config
pub fn get_combos_config() -> CombosConfig {
    // Define the function to return the appropriate combo configuration
    CombosConfig {
        combos: Vec::from_iter([
            Combo::new(
                [
                    k!(V), //3,4
                    k!(B), //3,5
                ]
                .to_vec(),
                k!(LShift),
                Some(0),
            ),
            Combo::new(
                [
                    k!(R), //1,4
                    k!(T), //1,5
                ]
                .to_vec(),
                k!(LAlt),
                Some(0),
            ),
            Combo::new(
                [
                    k!(E), //1,3
                    k!(T), //1,5
                ]
                .to_vec(),
                osm!(ModifierCombination::new_from(false, false, false, true, false)), // one-shot LShift
                Some(0),
            ),
            Combo::new(
                [
                    k!(E), //1,3
                    k!(R), //1,4
                ]
                .to_vec(),
                k!(A), // A
                Some(0),
            ),
        ]),
        timeout: Duration::from_millis(100),
    }
}

mod combo_test {
    use embassy_futures::block_on;
    use rmk::config::{BehaviorConfig, OneShotConfig, TapHoldConfig};
    use rmk::keycode::KeyCode;
    use rmk::th;
    use rusty_fork::rusty_fork_test;

    use super::*;
    use crate::common::{create_test_keyboard_with_config, KC_LSHIFT};

    rusty_fork_test! {
        #[test]
        fn test_combo_timeout_and_ignore() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    combo: get_combos_config(),
                    ..Default::default()
                }),
                sequence: [
                    [3, 4, true, 10],
                    [3, 4, false, 100],
                ],
                expected_reports: [
                    [0, [kc_to_u8!(V), 0, 0, 0, 0, 0]],
                ]
            }
        }

        #[test]
        fn test_combo_with_mod_then_mod_timeout() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    combo: get_combos_config(),
                    ..Default::default()
                }),
                sequence: [
                    [3, 4, true, 10], // Press V
                    [3, 5, true, 10], // Press B
                    [1, 4, true, 50], // Press R
                    [1, 4, false, 90], // Release R
                    [3, 4, false, 150], // Release V
                    [3, 5, false, 170], // Release B
                ],
                expected_reports: [
                    [KC_LSHIFT, [0; 6]], // V + B = LShift
                    [KC_LSHIFT, [KeyCode::R as u8, 0, 0, 0, 0, 0]], // Press R
                    [KC_LSHIFT, [0; 6]], // Release R
                    [0, [0; 6]], // Release V + B
                ]
            }
        }


        #[test]
        fn test_combo_with_one_shot_mod() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    combo: get_combos_config(),
                    one_shot: OneShotConfig { timeout: Duration::from_millis(300) },
                    ..Default::default()
                }),
                sequence: [
                    [1, 3, true, 10],
                    [1, 5, true, 10],
                    [1, 3, false, 50],
                    [1, 5, false, 70],
                    [1, 3, true, 50],
                    [1, 3, false, 110],
                ],
                expected_reports: [
                    [KC_LSHIFT, [KeyCode::E as u8, 0, 0, 0, 0, 0]],
                    [0, [0; 6]],
                ]
            }
        }

        #[test]
        fn test_combo_with_mod() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    combo: get_combos_config(),
                    ..Default::default()
                }),
                sequence: [
                    [3, 4, true, 10],
                    [3, 5, true, 10],
                    [3, 6, true, 50],
                    [3, 6, false, 70],
                    [3, 4, false, 100],
                    [3, 5, false, 110],
                ],
                expected_reports: [
                    [KC_LSHIFT, [0; 6]],
                    [KC_LSHIFT, [KeyCode::N as u8, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [0; 6]],
                    [0, [0; 6]],
                ]
            }
        }

        #[test]
        fn test_overlapped_combo() {
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    combo: get_combos_config(),
                    ..Default::default()
                }),
                sequence: [
                    [1, 3, true, 10],
                    [1, 5, true, 10],
                    [1, 3, false, 50],
                    [1, 5, false, 10],
                    [1, 4, true, 100],
                    [1, 3, true, 10],
                    [1, 4, false, 50],
                    [1, 3, false, 10],
                ],
                expected_reports: [
                    [KC_LSHIFT, [KeyCode::A as u8, 0, 0, 0, 0, 0]],
                    [0, [0; 6]],
                ]
            }
        }

        #[test]
        fn test_taphold_with_combo() {
            key_sequence_test! {
                keyboard: {
                    let behavior_config = BehaviorConfig {
                        tap_hold: TapHoldConfig {
                            enable_hrm: true,
                            permissive_hold: true,
                            chordal_hold: false,
                            post_wait_time: Duration::from_millis(0),
                            ..TapHoldConfig::default()
                        },
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
                    [3, 1, true, 20],  // Press th!(Z,LAlt)
                    [2, 1, false, 10], // Release A
                    [2, 2, false, 10], // Release S
                    [3, 1, false, 10], // Release Z
                ],
                expected_reports: [
                    [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }

    }
}
