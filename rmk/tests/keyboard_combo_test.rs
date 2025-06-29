pub mod common;

use embassy_time::Duration;
use heapless::Vec;
use rmk::combo::Combo;
use rmk::config::CombosConfig;
use rmk::k;

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
        ]),
        timeout: Duration::from_millis(100),
    }
}

mod combo_test {

    use embassy_futures::block_on;
    use rmk::config::BehaviorConfig;
    use rmk::keycode::KeyCode;
    use rusty_fork::rusty_fork_test;

    use super::*;
    use crate::common::{create_test_keyboard_with_config, run_key_sequence_test, KC_LSHIFT};

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

    }
}
