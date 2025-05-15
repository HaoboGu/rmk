mod common;
pub(crate) use crate::common::*;

mod combo_test {

    use super::*;
    use embassy_futures::block_on;
    use rmk::config::BehaviorConfig;
    use rmk::keycode::KeyCode;
    use rusty_fork::rusty_fork_test;

    // Init logger for tests
    #[ctor::ctor]
    pub fn init_log() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();
    }

    rusty_fork_test! {

    #[test]
    fn test_combo_timeout_and_ignore() {
        let main = async {
            let mut keyboard = create_test_keyboard_with_config(BehaviorConfig {
                combo: get_combos_config(),
                ..Default::default()
            });

            let sequence = key_sequence![
                [3, 4, true, 10],   // Press V
                [3, 4, false, 100], // Release V
            ];

            let expected_reports = key_report![
                [0, [KeyCode::V as u8, 0, 0, 0, 0, 0]],
            ];

            run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
        };

        block_on(main);
    }

    #[test]
    fn test_combo_with_mod_then_mod_timeout() {
        let main = async {
            let mut keyboard = create_test_keyboard_with_config(BehaviorConfig {
                combo: get_combos_config(),
                ..Default::default()
            });
            let sequence = key_sequence![
                [3, 4, true, 10], // Press V
                [3, 5, true, 10], // Press B
                [1, 4, true, 50], // Press R
                [1, 4, false, 90], // Release R
                [3, 4, false, 150], // Release V
                [3, 5, false, 170], // Release B
            ];

            let expected_reports = key_report![
                [KC_LSHIFT, [0; 6]],
                [KC_LSHIFT, [KeyCode::R as u8, 0, 0, 0, 0, 0]],
                [KC_LSHIFT, [0; 6]],
                [0, [0; 6]],
            ];

            run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
        };

        block_on(main);
    }

    #[test]
    fn test_combo_with_mod() {
        let main = async {
            let mut keyboard = create_test_keyboard_with_config(BehaviorConfig {
                combo: get_combos_config(),
                ..Default::default()
            });

            let sequence = key_sequence![
                [3, 4, true, 10],   // Press V
                [3, 5, true, 10],   // Press B
                [3, 6, true, 50],   // Press N
                [3, 6, false, 70],  // Release N
                [3, 4, false, 100], // Release V
                [3, 5, false, 110], // Release B
            ];

            let expected_reports = key_report![
                [KC_LSHIFT, [0; 6]],
                [KC_LSHIFT, [KeyCode::N as u8, 0, 0, 0, 0, 0]],
                [KC_LSHIFT, [0; 6]],
                [0, [0; 6]],
            ];

            run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
        };

        block_on(main);
    }

    } //fork ends
}
