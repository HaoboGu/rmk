pub mod common;

use embassy_time::Duration;
use rmk::combo::{Combo, ComboConfig};
use rmk::config::{BehaviorConfig, CombosConfig, MorsesConfig, OneShotConfig};
use rmk::types::keycode::KeyCode;
use rmk::types::modifier::ModifierCombination;
use rmk::{k, osm, th};
use rmk_types::action::{MorseMode, MorseProfile};
use rusty_fork::rusty_fork_test;

use crate::common::{KC_LSHIFT, create_test_keyboard_with_config};

// Get tested combo config
pub fn get_combos_config() -> CombosConfig {
    // Define the function to return the appropriate combo configuration
    CombosConfig {
        combos: [
            Some(Combo::new(ComboConfig::new(
                [
                    k!(V), //3,4
                    k!(B), //3,5
                ]
                .to_vec(),
                k!(LShift),
                Some(0),
            ))),
            Some(Combo::new(ComboConfig::new(
                [
                    k!(R), //1,4
                    k!(T), //1,5
                ]
                .to_vec(),
                k!(LAlt),
                Some(0),
            ))),
            Some(Combo::new(ComboConfig::new(
                [
                    k!(E), //1,3
                    k!(T), //1,5
                ]
                .to_vec(),
                osm!(ModifierCombination::new_from(false, false, false, true, false)), // one-shot LShift
                Some(0),
            ))),
            Some(Combo::new(ComboConfig::new(
                [
                    k!(E), //1,3
                    k!(R), //1,4
                ]
                .to_vec(),
                k!(A), // A
                Some(0),
            ))),
            Some(Combo::new(ComboConfig::new(
                [
                    k!(E), //1,3
                    k!(R), //1,4
                    k!(T), //1,5
                ]
                .to_vec(),
                k!(Space),
                Some(0),
            ))),
            None,
            None,
            None,
        ],
        timeout: Duration::from_millis(100),
    }
}

rusty_fork_test! {
    #[test]
    fn test_single_key_in_combo() {
        key_sequence_test! {
            keyboard: create_test_keyboard_with_config(BehaviorConfig {
                combo: get_combos_config(),
                ..Default::default()
            }),
            sequence: [
                [1, 3, true, 10],
                [1, 3, false, 50],
                [1, 4, true, 10],
                [1, 4, false, 50],
                [1, 5, true, 10],
                [1, 5, false, 10],
            ],
            expected_reports: [
                [0, [KeyCode::E as u8, 0, 0, 0, 0, 0]],
                [0, [0; 6]],
                [0, [KeyCode::R as u8, 0, 0, 0, 0, 0]],
                [0, [0; 6]],
                [0, [KeyCode::T as u8, 0, 0, 0, 0, 0]],
                [0, [0; 6]],
            ]
        }
    }
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
    fn test_combo_hold_one_key() {
        key_sequence_test! {
            keyboard: create_test_keyboard_with_config(BehaviorConfig {
                combo: get_combos_config(),
                ..Default::default()
            }),
            sequence: [
                [1, 3, true, 10],
                [1, 4, true, 10],
                [1, 4, false, 50],
                [1, 4, true, 100],
                [1, 4, false, 50],
                [1, 4, true, 100],
                [1, 4, false, 50],
                [1, 3, false, 10],
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(A), kc_to_u8!(R), 0, 0, 0, 0]],
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [0, [kc_to_u8!(A), kc_to_u8!(R), 0, 0, 0, 0]],
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
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
                [3, 4, true, 10], // V
                [3, 5, true, 10], // B
                [3, 6, true, 50], // N, trigger V + B = LShift
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
    fn test_fully_overlapped_combo_timeout() {
        key_sequence_test! {
            keyboard: create_test_keyboard_with_config(BehaviorConfig {
                combo: get_combos_config(),
                ..Default::default()
            }),
            sequence: [
                [1, 3, true, 10], // E
                [1, 4, true, 10], // T
                [1, 3, false, 170], // Timeout, should trigger E+T = A because E+T are triggered within the timeout window
                [1, 4, false, 10],
            ],
            expected_reports: [
                [0, [KeyCode::A as u8, 0, 0, 0, 0, 0]],
                [0, [0; 6]],
            ]
        }
    }

    #[test]
    fn test_fully_overlapped_combo_trigger_smaller() {
        key_sequence_test! {
            keyboard: create_test_keyboard_with_config(BehaviorConfig {
                combo: get_combos_config(),
                ..Default::default()
            }),
            sequence: [
                [1, 3, true, 10], // E
                [1, 4, true, 10], // T
                [1, 3, false, 10],
                [1, 4, false, 10],
            ],
            expected_reports: [
                [0, [KeyCode::A as u8, 0, 0, 0, 0, 0]],
                [0, [0; 6]],
            ]
        }
    }

    #[test]
    fn test_fully_overlapped_combo() {
        key_sequence_test! {
            keyboard: create_test_keyboard_with_config(BehaviorConfig {
                combo: get_combos_config(),
                ..Default::default()
            }),
            sequence: [
                [1, 3, true, 10], // E
                [1, 5, true, 10], // T
                [1, 4, true, 10], // R
                [1, 3, false, 50],
                [1, 5, false, 10],
                [1, 4, false, 50],
                [1, 3, true, 10], // E
                [1, 5, true, 10], // T
                [1, 3, false, 50],
                [1, 5, false, 10],
                [1, 3, true, 10], // E
                [1, 4, true, 10], // R
                [1, 3, false, 50],
                [1, 4, false, 50],
                [1, 3, true, 10], // E
                [1, 5, true, 10], // T
                [1, 4, true, 10], // R
                [1, 3, false, 50],
                [1, 5, false, 10],
                [1, 4, false, 50],

            ],
            expected_reports: [
                [0, [KeyCode::Space as u8, 0, 0, 0, 0, 0]],
                [0, [0; 6]],
                [KC_LSHIFT, [KeyCode::A as u8, 0, 0, 0, 0, 0]],
                [0, [0; 6]],
                [0, [KeyCode::Space as u8, 0, 0, 0, 0, 0]],
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
                    morse: MorsesConfig {
                        default_profile: MorseProfile::new(
                            Some(false),
                            Some(MorseMode::PermissiveHold),
                            Some(250u16),
                            Some(250u16)
                        ),
                        ..Default::default()
                    },
                    combo: CombosConfig {
                        combos: [
                            Some(Combo::new(ComboConfig::new(
                                [th!(A, LShift), th!(S, LGui), th!(Z, LAlt)],
                                k!(C),
                                None,
                            ))), None, None, None, None, None, None, None
                        ],
                        timeout: Duration::from_millis(50),
                    },
                    ..Default::default()
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
