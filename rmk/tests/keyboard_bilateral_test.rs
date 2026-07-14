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

use rmk::config::Hand;
use rmk::sim::{HandOverride, SimKeyboard, SimKeyboardSetup};
use rmk::types::keycode::HidKeyCode;
use rmk_types::morse::{MorseMode, MorseProfile};

use crate::common::morse::HRM_MORSE_SETUP;
use crate::common::{KC_LSHIFT, TEST_KEYMAP};

const BILATERAL_PROFILE: MorseProfile =
    MorseProfile::new(Some(true), Some(MorseMode::PermissiveHold), Some(250u16), Some(250u16));
const BILATERAL_HANDS: [HandOverride; 5] = [
    HandOverride::new(0, 0, Hand::Bilateral),
    HandOverride::new(0, 1, Hand::Left),
    HandOverride::new(0, 2, Hand::Right),
    HandOverride::new(0, 3, Hand::Right),
    HandOverride::new(0, 4, Hand::Right),
];
const BILATERAL_SETUP: SimKeyboardSetup = HRM_MORSE_SETUP
    .hand_overrides(&BILATERAL_HANDS)
    .morse_profile(BILATERAL_PROFILE)
    .morse_flow_tap(true)
    .morse_prior_idle_ms(120);

/// mt!(B, LShift) (col 1, Left) + A (col 0, Bilateral) should NOT trigger unilateral tap
/// because Bilateral keys have a different Hand value than Left/Right.
/// Instead, permissive hold should activate because A is released before mt!(B, LShift).
#[test]
fn test_bilateral_exempts_from_unilateral_tap() {
    crate::common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(BILATERAL_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift) on Left hand
            .delay(10)
            .press(0, 0) // Press A on Left hand (bilateral) -> should NOT trigger unilateral tap
            .delay(10)
            .release(0, 0) // Release A -> permissive hold triggers for mt!(B, LShift)
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_only_mods(KC_LSHIFT) // Permissive hold (LShift held)
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // Press A with shift
            .expect_only_mods(KC_LSHIFT) // Release A
            .expect_all_up() // Release mt!(B, LShift)
            .run()
            .await;
    });
}

/// Cross-hand press should still use permissive hold (bilateral doesn't change cross-hand behavior).
/// mt!(B, LShift) (col 1, Left) + mt!(C, LGui) (col 2, Right) = cross-hand -> permissive hold.
#[test]
fn test_bilateral_cross_hand_unchanged() {
    crate::common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(BILATERAL_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift) on Left hand
            .delay(10)
            .press(0, 2) // Press mt!(C, LGui) on Right hand -> cross-hand, no unilateral tap
            .delay(10)
            .release(0, 2) // Release mt!(C, LGui) -> permissive hold
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_only_mods(KC_LSHIFT) // Permissive hold (LShift)
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::C]) // Press C with shift
            .expect_only_mods(KC_LSHIFT) // Release C
            .expect_all_up() // Release mt!(B, LShift)
            .run()
            .await;
    });
}

/// Same-hand press with a NON-bilateral key should still trigger unilateral tap.
/// mt!(C, LGui) (col 2, Right) + lt!(1, D) (col 3, Right, NOT bilateral) = same hand, unilateral tap.
#[test]
fn test_non_bilateral_same_hand_still_unilateral() {
    crate::common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(BILATERAL_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 2) // Press mt!(C, LGui) on Right hand
            .delay(10)
            .press(0, 3) // Press lt!(1, D) on Right hand -> Flow tap won't be triggered because the previous morse key is not resolved yet.
            .delay(10)
            .release(0, 3) // Release lt!(1, D) -> Unilateral tap still applies since col 3 is NOT bilateral
            .delay(10)
            .release(0, 2) // Release mt!(C, LGui)
            .expect_keys([HidKeyCode::C]) // Unilateral tap for mt!(C, LGui)
            .expect_keys([HidKeyCode::C, HidKeyCode::D]) // Press D
            .expect_keys([HidKeyCode::C]) // Release D
            .expect_all_up() // Release mt!(C, LGui)
            .run()
            .await;
    });
}

/// Bilateral key with hold timeout: mt!(B, LShift) held past timeout should still activate hold.
/// Bilateral only affects unilateral_tap decision, not the hold timeout.
#[test]
fn test_bilateral_hold_timeout_unchanged() {
    crate::common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(BILATERAL_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(300)
            .release(0, 1) // Release after hold timeout
            .expect_only_mods(KC_LSHIFT) // Hold LShift
            .expect_all_up() // Release
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
    crate::common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(BILATERAL_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift) on Left hand
            .delay(10)
            .press(0, 0) // Press A on Left hand (bilateral)
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift) first -> resolves as tap (B)
            .delay(10)
            .release(0, 0) // Release A
            .expect_keys([HidKeyCode::B]) // Tap B (mod-tap released first)
            .expect_keys([HidKeyCode::B, HidKeyCode::A]) // Press A
            .expect_keys([HidKeyCode::A]) // Release B
            .expect_all_up() // Release A
            .run()
            .await;
    });
}
