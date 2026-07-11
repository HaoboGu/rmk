/// Test cases for bilateral keys in layout map with unilateral_tap
///
/// Keys marked as Hand::Bilateral in the layout map are exempt from unilateral_tap,
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

use rmk::sim::{SimKeyboard, SimKeyboardSetup, SimMorseSetup};

use rmk::config::Hand;
use rmk_types::morse::{MorseMode, MorseProfile};

use crate::common::KC_LSHIFT;
use crate::common::morse::{MORSE_KEYMAP, TEST_MORSE_PATTERNS};

const BILATERAL_PROFILE: MorseProfile =
    MorseProfile::new(Some(true), Some(MorseMode::PermissiveHold), Some(250u16), Some(250u16));
const BILATERAL_SETUP: SimKeyboardSetup<1, 5> = SimKeyboardSetup::new()
    .hands([[Hand::Bilateral, Hand::Left, Hand::Right, Hand::Right, Hand::Right]])
    .morse(
        SimMorseSetup::new()
            .patterns(&TEST_MORSE_PATTERNS)
            .profile(BILATERAL_PROFILE)
            .flow_tap(true)
            .prior_idle_ms(120),
    );

/// mt!(B, LShift) (col 1, Left) + A (col 0, Bilateral) should NOT trigger unilateral tap
/// because Bilateral keys have a different Hand value than Left/Right.
/// Instead, permissive hold should activate because A is released before mt!(B, LShift).
#[test]
fn test_bilateral_exempts_from_unilateral_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(BILATERAL_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift) on Left hand
            .delay(10)
            .press(0, 0) // Press A on Left hand (bilateral) -> should NOT trigger unilateral tap
            .delay(10)
            .release(0, 0) // Release A -> permissive hold triggers for mt!(B, LShift)
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Permissive hold (LShift held)
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A with shift
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .run()
            .await;
    });
}

/// Cross-hand press should still use permissive hold (bilateral doesn't change cross-hand behavior).
/// mt!(B, LShift) (col 1, Left) + mt!(C, LGui) (col 2, Right) = cross-hand -> permissive hold.
#[test]
fn test_bilateral_cross_hand_unchanged() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(BILATERAL_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift) on Left hand
            .delay(10)
            .press(0, 2) // Press mt!(C, LGui) on Right hand -> cross-hand, no unilateral tap
            .delay(10)
            .release(0, 2) // Release mt!(C, LGui) -> permissive hold
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Permissive hold (LShift)
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(C), 0, 0, 0, 0, 0])) // Press C with shift
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Release C
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .run()
            .await;
    });
}

/// Same-hand press with a NON-bilateral key should still trigger unilateral tap.
/// mt!(C, LGui) (col 2, Right) + lt!(1, D) (col 3, Right, NOT bilateral) = same hand, unilateral tap.
#[test]
fn test_non_bilateral_same_hand_still_unilateral() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(BILATERAL_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 2) // Press mt!(C, LGui) on Right hand
            .delay(10)
            .press(0, 3) // Press lt!(1, D) on Right hand -> Flow tap won't be triggered because the previous morse key is not resolved yet.
            .delay(10)
            .release(0, 3) // Release lt!(1, D) -> Unilateral tap still applies since col 3 is NOT bilateral
            .delay(10)
            .release(0, 2) // Release mt!(C, LGui)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), 0, 0, 0, 0, 0])) // Unilateral tap for mt!(C, LGui)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), kc_to_u8!(D), 0, 0, 0, 0])) // Press D
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), 0, 0, 0, 0, 0])) // Release D
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release mt!(C, LGui)
            .run()
            .await;
    });
}

/// Bilateral key with hold timeout: mt!(B, LShift) held past timeout should still activate hold.
/// Bilateral only affects unilateral_tap decision, not the hold timeout.
#[test]
fn test_bilateral_hold_timeout_unchanged() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(BILATERAL_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(300)
            .release(0, 1) // Release after hold timeout
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Hold LShift
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release
            .run()
            .await;
    });
}

/// Bilateral key with reversed release order:
/// mt!(B, LShift) (col 1, Left) pressed, then A (col 0, Left, bilateral) pressed,
/// then mt!(B, LShift) released first, then A released.
/// Because A is bilateral, unilateral tap should NOT trigger.
/// However, releasing mt key first still resolves it as tap (B) via normal morse tap prediction.
#[test]
fn test_bilateral_reversed_release() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(BILATERAL_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift) on Left hand
            .delay(10)
            .press(0, 0) // Press A on Left hand (bilateral)
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift) first -> resolves as tap (B)
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Tap B (mod-tap released first)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), kc_to_u8!(A), 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, kc_to_u8!(A), 0, 0, 0, 0])) // Release B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}
