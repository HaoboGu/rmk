/// Test cases for bilateral keys in matrix_map with unilateral_tap
///
/// Keys marked as Hand::Bilateral in the matrix_map are exempt from unilateral_tap,
/// allowing same-hand key combinations to use normal mod-tap resolution.
///
/// Keyboard layout (1 row, 5 cols, 2 layers):
///   Col:  0     1                    2                  3           4
///   L0: [A,  mt!(B, LShift),  mt!(C, LGui),  lt!(1, D),  mt!(E, LAlt)]
///   L1: [Kp1,     Kp2,            Kp3,           Kp4,        Kp5]
///
/// Hand config: [Bilateral, Left, Right, Right, Right]
///   - Col 0 is Bilateral (exempt from unilateral_tap)
///   - Col 1 is Left hand
///   - Cols 2-4 are Right hand
pub mod common;

use embassy_time::Duration;
use rmk::config::{BehaviorConfig, Hand, MorsesConfig};
use rmk::keyboard::Keyboard;
use rmk_types::action::{MorseMode, MorseProfile};
use rusty_fork::rusty_fork_test;

use crate::common::KC_LSHIFT;
use crate::common::morse::create_morse_keyboard;

/// Create a keyboard with col 0 marked as Bilateral in the matrix_map.
/// Hand: [Bilateral, Left, Right, Right, Right]
fn create_bilateral_keyboard() -> Keyboard<'static, 1, 5, 2> {
    let hand = [[Hand::Bilateral, Hand::Left, Hand::Right, Hand::Right, Hand::Right]];
    create_morse_keyboard(
        BehaviorConfig {
            morse: MorsesConfig {
                enable_flow_tap: true,
                prior_idle_time: Duration::from_millis(120),
                default_profile: MorseProfile::new(
                    Some(true), // unilateral_tap enabled
                    Some(MorseMode::PermissiveHold),
                    Some(250u16),
                    Some(250u16),
                ),
                ..Default::default()
            },
            ..Default::default()
        },
        hand,
    )
}

rusty_fork_test! {
    /// mt!(B, LShift) (col 1, Left) + A (col 0, Bilateral) should NOT trigger unilateral tap
    /// because Bilateral keys have a different Hand value than Left/Right.
    /// Instead, permissive hold should activate because A is released before mt!(B, LShift).
    #[test]
    fn test_bilateral_exempts_from_unilateral_tap() {
        key_sequence_test! {
            keyboard: create_bilateral_keyboard(),
            sequence: [
                [0, 1, true, 150],  // Press mt!(B, LShift) on Left hand
                [0, 0, true, 10],   // Press A on Left hand (bilateral) -> should NOT trigger unilateral tap
                [0, 0, false, 10],  // Release A -> permissive hold triggers for mt!(B, LShift)
                [0, 1, false, 10],  // Release mt!(B, LShift)
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],                  // Permissive hold (LShift held)
                [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]],      // Press A with shift
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],                  // Release A
                [0, [0, 0, 0, 0, 0, 0]],                          // Release mt!(B, LShift)
            ]
        };
    }

    /// Cross-hand press should still use permissive hold (bilateral doesn't change cross-hand behavior).
    /// mt!(B, LShift) (col 1, Left) + mt!(C, LGui) (col 2, Right) = cross-hand -> permissive hold.
    #[test]
    fn test_bilateral_cross_hand_unchanged() {
        key_sequence_test! {
            keyboard: create_bilateral_keyboard(),
            sequence: [
                [0, 1, true, 150],  // Press mt!(B, LShift) on Left hand
                [0, 2, true, 10],   // Press mt!(C, LGui) on Right hand -> cross-hand, no unilateral tap
                [0, 2, false, 10],  // Release mt!(C, LGui) -> permissive hold
                [0, 1, false, 10],  // Release mt!(B, LShift)
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],                  // Permissive hold (LShift)
                [KC_LSHIFT, [kc_to_u8!(C), 0, 0, 0, 0, 0]],      // Press C with shift
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],                  // Release C
                [0, [0, 0, 0, 0, 0, 0]],                          // Release mt!(B, LShift)
            ]
        };
    }

    /// Same-hand press with a NON-bilateral key should still trigger unilateral tap.
    /// mt!(C, LGui) (col 2, Right) + lt!(1, D) (col 3, Right, NOT bilateral) = same hand, unilateral tap.
    #[test]
    fn test_non_bilateral_same_hand_still_unilateral() {
        key_sequence_test! {
            keyboard: create_bilateral_keyboard(),
            sequence: [
                [0, 2, true, 150],  // Press mt!(C, LGui) on Right hand
                [0, 3, true, 10],   // Press lt!(1, D) on Right hand -> Flow tap won't be triggered because the previous morse key is not resolved yet.
                [0, 3, false, 10],  // Release lt!(1, D) -> Unilateral tap still applies since col 3 is NOT bilateral
                [0, 2, false, 10],  // Release mt!(C, LGui)
            ],
            expected_reports: [
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],              // Unilateral tap for mt!(C, LGui)
                [0, [kc_to_u8!(C), kc_to_u8!(D), 0, 0, 0, 0]],   // Press D
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],               // Release D
                [0, [0, 0, 0, 0, 0, 0]],                          // Release mt!(C, LGui)
            ]
        };
    }

    /// Bilateral key with hold timeout: mt!(B, LShift) held past timeout should still activate hold.
    /// Bilateral only affects unilateral_tap decision, not the hold timeout.
    #[test]
    fn test_bilateral_hold_timeout_unchanged() {
        key_sequence_test! {
            keyboard: create_bilateral_keyboard(),
            sequence: [
                [0, 1, true, 150],  // Press mt!(B, LShift)
                [0, 1, false, 300], // Release after hold timeout
            ],
            expected_reports: [
                [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],  // Hold LShift
                [0, [0, 0, 0, 0, 0, 0]],           // Release
            ]
        };
    }

    /// Bilateral key with reversed release order:
    /// mt!(B, LShift) (col 1, Left) pressed, then A (col 0, Left, bilateral) pressed,
    /// then mt!(B, LShift) released first, then A released.
    /// Because A is bilateral, unilateral tap should NOT trigger.
    /// However, releasing mt key first still resolves it as tap (B) via normal morse tap prediction.
    #[test]
    fn test_bilateral_reversed_release() {
        key_sequence_test! {
            keyboard: create_bilateral_keyboard(),
            sequence: [
                [0, 1, true, 150],  // Press mt!(B, LShift) on Left hand
                [0, 0, true, 10],   // Press A on Left hand (bilateral)
                [0, 1, false, 10],  // Release mt!(B, LShift) first -> resolves as tap (B)
                [0, 0, false, 10],  // Release A
            ],
            expected_reports: [
                [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]],              // Tap B (mod-tap released first)
                [0, [kc_to_u8!(B), kc_to_u8!(A), 0, 0, 0, 0]],   // Press A
                [0, [0, kc_to_u8!(A), 0, 0, 0, 0]],               // Release B
                [0, [0, 0, 0, 0, 0, 0]],                          // Release A
            ]
        };
    }
}
