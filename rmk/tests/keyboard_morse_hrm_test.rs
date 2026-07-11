/// Test cases for home row mod(HRM)
///
/// For HRM, `enable_flow_tap` and `unilateral_tap` is enabled, `prior-idle-time` will be considered.
///
/// Keyboard layout (1 row, 5 cols, 2 layers):
///   Col:  0     1                    2                  3           4
///   L0: [A,  mt!(B, LShift),  mt!(C, LGui),  lt!(1, D),  mt!(E, LAlt)]
///   L1: [Kp1,     Kp2,            Kp3,           Kp4,        Kp5]
///
/// Hand config: [Left, Left, Right, Right, Right]
pub mod common;

use rmk::sim::{KeymapOverride, SimKeyboard, SimKeyboardSetup, SimMorseSetup};

use rmk::config::Hand;
use rmk::{a, k, mo};
use rmk_types::morse::{MorseMode, MorseProfile};

use crate::common::morse::{MORSE_2_KEY_COMBOS, MORSE_3_KEY_COMBOS, MORSE_KEYMAP, TEST_MORSE_PATTERNS};
use crate::common::{KC_LGUI, KC_LSHIFT, TEST_KEYMAP};

const HRM_PROFILE: MorseProfile =
    MorseProfile::new(Some(true), Some(MorseMode::PermissiveHold), Some(250u16), Some(250u16));
const HRM_NORMAL_PROFILE: MorseProfile =
    MorseProfile::new(Some(true), Some(MorseMode::Normal), Some(250u16), Some(250u16));
const HRM_SETUP: SimKeyboardSetup<1, 5> = SimKeyboardSetup::new()
    .hands([[Hand::Left, Hand::Left, Hand::Right, Hand::Right, Hand::Right]])
    .morse(
        SimMorseSetup::new()
            .patterns(&TEST_MORSE_PATTERNS)
            .profile(HRM_PROFILE)
            .flow_tap(true)
            .prior_idle_ms(120),
    );
const HRM_NORMAL_SETUP: SimKeyboardSetup<1, 5> = SimKeyboardSetup::new()
    .hands([[Hand::Left, Hand::Left, Hand::Right, Hand::Right, Hand::Right]])
    .morse(
        SimMorseSetup::new()
            .patterns(&TEST_MORSE_PATTERNS)
            .profile(HRM_NORMAL_PROFILE),
    );
const RELEASE_REMAP_KEY_OVERRIDES: [KeymapOverride; 6] = [
    KeymapOverride::new(0, 0, 0, mo!(1)),
    KeymapOverride::new(0, 0, 1, a!(No)),
    KeymapOverride::new(0, 0, 2, k!(A)),
    KeymapOverride::new(1, 0, 0, a!(Transparent)),
    KeymapOverride::new(1, 0, 1, k!(B)),
    KeymapOverride::new(1, 0, 2, a!(Transparent)),
];
const RELEASE_REMAP_SETUP: SimKeyboardSetup<5, 14> = SimKeyboardSetup::new().keys(&RELEASE_REMAP_KEY_OVERRIDES);
const RELEASE_REMAP_NORMAL_PROFILE: MorseProfile =
    MorseProfile::new(Some(false), Some(MorseMode::Normal), Some(250u16), Some(250u16));
const RELEASE_REMAP_PERMISSIVE_HOLD_PROFILE: MorseProfile =
    MorseProfile::new(Some(false), Some(MorseMode::PermissiveHold), Some(250u16), Some(250u16));
const RELEASE_REMAP_HOLD_ON_OTHER_PROFILE: MorseProfile = MorseProfile::new(
    Some(false),
    Some(MorseMode::HoldOnOtherPress),
    Some(250u16),
    Some(250u16),
);
const RELEASE_REMAP_NORMAL_SETUP: SimKeyboardSetup<5, 14> =
    RELEASE_REMAP_SETUP.morse_profile(RELEASE_REMAP_NORMAL_PROFILE);
const RELEASE_REMAP_PERMISSIVE_HOLD_SETUP: SimKeyboardSetup<5, 14> =
    RELEASE_REMAP_SETUP.morse_profile(RELEASE_REMAP_PERMISSIVE_HOLD_PROFILE);
const RELEASE_REMAP_HOLD_ON_OTHER_SETUP: SimKeyboardSetup<5, 14> =
    RELEASE_REMAP_SETUP.morse_profile(RELEASE_REMAP_HOLD_ON_OTHER_PROFILE);

#[test]
fn test_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(100)
            .release(0, 1) // Release B
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release B
            .run()
            .await;
    });
}

#[test]
fn test_hold() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(300)
            .release(0, 1) // Release B after hold timeout
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Hold LShift
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // All released
            .run()
            .await;
    });
}

#[test]
fn test_mt_1() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .press(0, 0) // Press A -> unilateral tap
            .delay(10)
            .release(0, 0) // Release A
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Unilateral tap
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), kc_to_u8!(A), 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_mt_1_1() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .press(0, 3) // Press lt!(1, D) -> Flow tap won't be triggered because the previous morse key is not resolved yet.
            .delay(10)
            .release(0, 3) // Release lt!(1, D) -> Permissive hold triggered
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Permissive hold
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(D), 0, 0, 0, 0, 0])) // Press D
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Release D
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_mt_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .press(0, 0) // Press A -> Unilateral tap
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift)
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), kc_to_u8!(A), 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, kc_to_u8!(A), 0, 0, 0, 0])) // Release B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_mt_2_1() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift)
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release B
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), 0, 0, 0, 0, 0])) // Press D
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release D
            .run()
            .await;
    });
}

#[test]
fn test_mt_2_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(10)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .release(0, 2) // Release mt!(C, LGui)
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), 0, 0, 0, 0, 0])) // Press C
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release C
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), 0, 0, 0, 0, 0])) // Press D
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release D
            .run()
            .await;
    });
}

#[test]
fn test_mt_2_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(10)
            .press(0, 3) // Press lt!(1, D) -> Unilateral tap
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .delay(10)
            .release(0, 2) // Release mt!(C, LGui)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), 0, 0, 0, 0, 0])) // Press C
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), kc_to_u8!(D), 0, 0, 0, 0])) // Press D
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), 0, 0, 0, 0, 0])) // Release D
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release C
            .run()
            .await;
    });
}

#[test]
fn test_mt_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift) -> Flow Tap
            .delay(10)
            .release(0, 0) // Release A
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), kc_to_u8!(B), 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [0, kc_to_u8!(B), 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release B
            .run()
            .await;
    });
}

#[test]
fn test_mt_4() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift)
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), kc_to_u8!(B), 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Release B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_mt_5() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release B
            .run()
            .await;
    });
}

#[test]
fn test_mt_6() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift)
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release B
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_1() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .press(0, 0) // Press A
            .delay(260)
            .release(0, 0) // Release A -> Timeout
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Timeout
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_1_1() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(10)
            .press(0, 3) // Press lt!(1, D)
            .delay(260)
            .release(0, 3) // Release lt!(1, D)
            .delay(10)
            .release(0, 2) // Release mt!(C, LGui)
            .expect_keyboard_report(crate::common::report(KC_LGUI, [0, 0, 0, 0, 0, 0])) // Timeout
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_1_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(10)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .release(0, 3) // Release lt!(1, D) -> Unilateral tap
            .delay(260)
            .release(0, 2) // Release mt!(C, LGui)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), 0, 0, 0, 0, 0])) // Press C
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), kc_to_u8!(D), 0, 0, 0, 0])) // Press D
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), 0, 0, 0, 0, 0])) // Release D
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release C
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .press(0, 0) // Press A
            .delay(260)
            .release(0, 1) // Release mt!(B, LShift)
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Timeout
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_2_1() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(10)
            .press(0, 3) // Press lt!(1, D)
            .delay(260)
            .release(0, 2) // Release mt!(C, LGui)
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(KC_LGUI, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift) -> Flow Tap
            .delay(260)
            .release(0, 0) // Release A
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), kc_to_u8!(B), 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [0, kc_to_u8!(B), 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release B
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_4() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift) -> Flow Tap
            .delay(260)
            .release(0, 1) // Release mt!(B, LShift)
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), kc_to_u8!(B), 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Release B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_5() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift) -> Flow Tap
            .delay(260)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Press mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_6() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(260)
            .release(0, 1) // Release mt!(B, LShift)
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Press mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_7() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift) -> Flow Tap
            .delay(10)
            .release(0, 0) // Release A
            .delay(260)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), kc_to_u8!(B), 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [0, kc_to_u8!(B), 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_8() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .press(0, 0) // Press A -> Unilateral tap
            .delay(10)
            .release(0, 0) // Release A
            .delay(260)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Unilateral tap
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), kc_to_u8!(A), 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_8_1() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .delay(260)
            .release(0, 2) // Release mt!(C, LGui)
            .expect_keyboard_report(crate::common::report(KC_LGUI, [0, 0, 0, 0, 0, 0])) // Permissive hold
            .expect_keyboard_report(crate::common::report(KC_LGUI, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(KC_LGUI, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release mt!(C, LGui)
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_9() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(260)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Timeout
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_10() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(260)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift)
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Timeout
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_1() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0])) // Press Kp1
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release Kp1
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), 0, 0, 0, 0, 0])) // Press D
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), kc_to_u8!(A), 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, kc_to_u8!(A), 0, 0, 0, 0])) // Release D
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 3) // Press lt!(1, D) -> Flow Tap
            .delay(10)
            .release(0, 0) // Release A
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), kc_to_u8!(D), 0, 0, 0, 0])) // Press D
            .expect_keyboard_report(crate::common::report(0, [0, kc_to_u8!(D), 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release D
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_4() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 3) // Press lt!(1, D) -> Flow Tap
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), kc_to_u8!(D), 0, 0, 0, 0])) // Press D
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Release D
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_5() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .delay(10)
            .press(0, 3) // Press lt!(1, D) -> Flow Tap
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), 0, 0, 0, 0, 0])) // Press D
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release D
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_6() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), 0, 0, 0, 0, 0])) // Press D
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release D
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_1() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A
            .delay(260)
            .release(0, 0) // Release A -> timeout, trigger Kp1 on layer 1
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0])) // Press Kp1
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release Kp1
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A
            .delay(260)
            .release(0, 3) // Release lt!(1, D) -> timeout, trigger Kp1 on layer 1
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0])) // Press Kp1
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release Kp1
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 3) // Press lt!(1, D) -> Flow Tap
            .delay(260)
            .release(0, 0) // Release A
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), kc_to_u8!(D), 0, 0, 0, 0])) // Press D
            .expect_keyboard_report(crate::common::report(0, [0, kc_to_u8!(D), 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_4() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 3) // Press lt!(1, D) -> Flow Tap
            .delay(260)
            .release(0, 3) // Release lt!(1, D)
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), kc_to_u8!(D), 0, 0, 0, 0])) // Press D
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Release D
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_5() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .delay(10)
            .press(0, 3) // Press lt!(1, D) -> Flow tap
            .delay(260)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), 0, 0, 0, 0, 0])) // Press D
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release D
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_5_1() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(200)
            .release(0, 0) // Release A -> Longer than `prior-idle-time`
            .delay(10)
            .press(0, 3) // Press lt!(1, D)
            .delay(260)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_6() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 3) // Press lt!(1, D)
            .delay(270)
            .release(0, 3) // Release lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_7() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 3) // Press lt!(1, D) -> Flow Tap
            .delay(10)
            .release(0, 0) // Release A
            .delay(260)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), kc_to_u8!(D), 0, 0, 0, 0])) // Press D
            .expect_keyboard_report(crate::common::report(0, [0, kc_to_u8!(D), 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release D
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_8() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A -> permisshive hold: Kp1 on layer 1
            .delay(10)
            .release(0, 0) // Release A
            .delay(260)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_9() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 3) // Press lt!(1, D)
            .delay(260)
            .press(0, 0) // Press A -> Kp1 on layer 1
            .delay(10)
            .release(0, 0) // Release A
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0])) // Press Kp1 on layer 1
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release Kp1
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_10() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 3) // Press lt!(1, D)
            .delay(260)
            .press(0, 0) // Press A -> Kp1 on layer 1
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0])) // Press Kp1 on layer 1
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release Kp1
            .run()
            .await;
    });
}

#[test]
fn test_trigger() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(50)
            .press(0, 0) // Press A -> Unilateral tap
            .delay(10)
            .release(0, 0) // Release A
            .delay(100)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), kc_to_u8!(A), 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // All released
            .run()
            .await;
    });
}

#[test]
fn test_with_combo_1() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP)
            .setup(HRM_SETUP)
            .combos_global(MORSE_2_KEY_COMBOS)
            .combos_global(MORSE_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(200)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(60)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(10)
            .release(0, 2) // Release C
            .delay(300)
            .release(0, 1) // Release B
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(C), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_with_combo_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP)
            .setup(HRM_SETUP)
            .combos_global(MORSE_2_KEY_COMBOS)
            .combos_global(MORSE_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(200)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(20)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(10)
            .release(0, 2) // Release C
            .delay(300)
            .release(0, 1) // Release B
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(X), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_with_combo_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP)
            .setup(HRM_SETUP)
            .combos_global(MORSE_2_KEY_COMBOS)
            .combos_global(MORSE_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(200)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(20)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(20)
            .release(0, 1) // Release B
            .delay(10)
            .release(0, 2) // Release C
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(X), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_with_combo_4() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP)
            .setup(HRM_SETUP)
            .combos_global(MORSE_2_KEY_COMBOS)
            .combos_global(MORSE_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(200)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(60)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(20)
            .release(0, 1) // Release B
            .delay(10)
            .release(0, 2) // Release C
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_with_combo_5() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP)
            .setup(HRM_SETUP)
            .combos_global(MORSE_2_KEY_COMBOS)
            .combos_global(MORSE_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(200)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(20)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(20)
            .release(0, 1) // Release B
            .delay(10)
            .release(0, 2) // Release C
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(X), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_with_combo_6() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP)
            .setup(HRM_SETUP)
            .combos_global(MORSE_2_KEY_COMBOS)
            .combos_global(MORSE_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(200)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(20)
            .press(0, 3) // Press lt!(1, D)
            .delay(60)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(20)
            .release(0, 1) // Release B
            .delay(10)
            .release(0, 3) // Release D
            .delay(10)
            .release(0, 2) // Release C
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_with_combo_7() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP)
            .setup(HRM_SETUP)
            .combos_global(MORSE_2_KEY_COMBOS)
            .combos_global(MORSE_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(200)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(20)
            .press(0, 3) // Press lt!(1, D)
            .delay(20)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(20)
            .release(0, 1) // Release B
            .delay(10)
            .release(0, 2) // Release C
            .delay(10)
            .release(0, 3) // Release D
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Z), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_with_combo_8() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP)
            .setup(HRM_SETUP)
            .combos_global(MORSE_2_KEY_COMBOS)
            .combos_global(MORSE_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(200)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(20)
            .press(0, 3) // Press lt!(1, D)
            .delay(60)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(20)
            .release(0, 1) // Release B
            .delay(10)
            .release(0, 2) // Release C  -> Unilateral tap of lt!(1, D) is triggered, before the mt!(B, LShift) is released and triggered
            .delay(10)
            .release(0, 3) // Release D
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), kc_to_u8!(C), 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_with_combo_8_1() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP)
            .setup(HRM_SETUP)
            .combos_global(MORSE_2_KEY_COMBOS)
            .combos_global(MORSE_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(200)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(200)
            .press(0, 3) // Press lt!(1, D)
            .delay(60)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(20)
            .release(0, 1) // Release B
            .delay(10)
            .release(0, 2) // Release C -> Unilateral tap of lt!(1, D) is triggered, before the mt!(B, LShift) is released and triggered
            .delay(10)
            .release(0, 3) // Release D
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), kc_to_u8!(C), 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_timeout() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(260)
            .press(0, 0) // Press A after hold timeout
            .delay(100)
            .release(0, 0) // Release A
            .delay(100)
            .release(0, 1) // Release B
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Hold LShift
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // All released
            .run()
            .await;
    });
}

#[test]
fn test_quick_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift) -> Flow Tap
            .delay(100)
            .release(0, 1) // Release B
            .delay(100)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), kc_to_u8!(B), 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Release B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_multi_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(120)
            .release(0, 0) // Release A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(60)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(60)
            .release(0, 1) // Release mt!(B, LShift)
            .delay(60)
            .release(0, 2) // Release mt!(C, LGui)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release B
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), 0, 0, 0, 0, 0])) // Release C
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release C
            .run()
            .await;
    });
}

#[test]
fn test_multi_tap_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift) -> Flow Tap
            .delay(200)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(60)
            .release(0, 1) // Release mt!(B, LShift)
            .delay(60)
            .release(0, 2) // Release mt!(C, LGui)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release B
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), 0, 0, 0, 0, 0])) // Release C
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release C
            .run()
            .await;
    });
}

#[test]
fn test_multi_tap_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift) -> Flow Tap
            .delay(40)
            .press(0, 2) // Press mt!(C, LGui) -> Flow Tap
            .delay(60)
            .release(0, 1) // Release mt!(B, LShift)
            .delay(60)
            .release(0, 2) // Release mt!(C, LGui)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), kc_to_u8!(C), 0, 0, 0, 0])) // Press C
            .expect_keyboard_report(crate::common::report(0, [0, kc_to_u8!(C), 0, 0, 0, 0])) // Release B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release C
            .run()
            .await;
    });
}

#[test]
fn test_layer_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(100)
            .release(0, 1) // Release B
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .delay(10)
            .press(0, 3) // Press lt!(1, D) -> Flow Tap after A
            .delay(50)
            .press(0, 1) // Press mt!(B, LShift) -> Flow Tap
            .delay(100)
            .release(0, 1) // Release B
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp2), 0, 0, 0, 0, 0])) // Press Kp2 on layer 1
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release Kp2
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), 0, 0, 0, 0, 0])) // Press D
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), kc_to_u8!(B), 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), 0, 0, 0, 0, 0])) // Release B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release D
            .run()
            .await;
    });
}

#[test]
fn test_rolling_with_layer_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .delay(10)
            .release(0, 0) // Release A
            .delay(250)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .delay(100)
            .release(0, 3) // Release lt!(1, D)
            .delay(250)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A
            .delay(100)
            .release(0, 3) // Release lt!(1, D)
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), 0, 0, 0, 0, 0])) // D
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), kc_to_u8!(A), 0, 0, 0, 0])) // D + A
            .expect_keyboard_report(crate::common::report(0, [0, kc_to_u8!(A), 0, 0, 0, 0])) // Release D
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0])) // Kp1 on layer 1
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release Kp1
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), 0, 0, 0, 0, 0])) // D
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), kc_to_u8!(A), 0, 0, 0, 0])) // D + A
            .expect_keyboard_report(crate::common::report(0, [0, kc_to_u8!(A), 0, 0, 0, 0])) // Release D
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_timeout_rolled_release() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(260)
            .press(0, 0) // Press A after hold timeout
            .delay(100)
            .release(0, 1) // Release B
            .delay(100)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Hold LShift
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // All released
            .run()
            .await;
    });
}

#[test]
fn test_timeout_rolled_release_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .press(0, 0) // Press A
            .delay(300)
            .release(0, 1) // Release B
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Timeout B
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_timeout_and_release() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(20)
            .press(0, 0) // Press A
            .delay(260)
            .release(0, 0) // Release A
            .delay(100)
            .release(0, 1) // Release B
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // All released
            .run()
            .await;
    });
}

#[test]
fn test_timeout_and_release_with_other_morse_key() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(200)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(100)
            .release(0, 2) // Release C  <-- Release C after "permissive hold" interval, but also after the hold-timeout
            .delay(100)
            .release(0, 1) // Release B
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Hold LShift
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(C), 0, 0, 0, 0, 0])) // Press C
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Release C
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // All released
            .run()
            .await;
    });
}

#[test]
fn test_rolling_release_order() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(30)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(30)
            .press(0, 0) // Press A
            .delay(50)
            .release(0, 1) // Release mt!(B, LShift)
            .delay(100)
            .release(0, 2) // Release mt!(C, LGui)
            .delay(100)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // FIXME: Maybe B-C-A is the expected order
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), kc_to_u8!(A), 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, kc_to_u8!(A), 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), kc_to_u8!(A), 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, kc_to_u8!(A), 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_rolling_release_order_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(30)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(30)
            .press(0, 0) // Press A
            .delay(100)
            .release(0, 2) // Release C -> Permissive hold for mt!(B, LShift)
            .delay(50)
            .release(0, 1) // Release B
            .delay(100)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(C), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(
                KC_LSHIFT,
                [kc_to_u8!(C), kc_to_u8!(A), 0, 0, 0, 0],
            ))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, kc_to_u8!(A), 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, kc_to_u8!(A), 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_rolling_release_order_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(30)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(30)
            .press(0, 0) // Press A
            .delay(100)
            .release(0, 2) // Release C -> Permissive hold for mt!(B, LShift)
            .delay(100)
            .release(0, 0) // Release A
            .delay(50)
            .release(0, 1) // Release B
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(C), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(
                KC_LSHIFT,
                [kc_to_u8!(C), kc_to_u8!(A), 0, 0, 0, 0],
            ))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, kc_to_u8!(A), 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_multiple_permissive_hold() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(30)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(30)
            .press(0, 0) // Press A -> Unilateral tap for mt!(B, LShift)
            .delay(100)
            .release(0, 0) // Release A -> Permissive hold triggered of mt!(C, LGui)
            .delay(50)
            .release(0, 1) // Release B
            .delay(100)
            .release(0, 2) // Release C
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LGUI, [kc_to_u8!(B), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LGUI, [kc_to_u8!(B), kc_to_u8!(A), 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LGUI, [kc_to_u8!(B), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LGUI, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_complex_rolling() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(160)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift) -> Flow Tap
            .delay(10)
            .release(0, 0) // Release A
            .delay(30)
            .press(0, 3) // Press lt!(1, D) -> Flow Tap
            .delay(30)
            .press(0, 2) // Press mt!(C, LGui) -> Flow Tap
            .delay(100)
            .release(0, 3) // Release D
            .delay(50)
            .release(0, 1) // Release B
            .delay(10)
            .release(0, 2) // Release C
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), kc_to_u8!(B), 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, kc_to_u8!(B), 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), kc_to_u8!(B), 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(
                0,
                [kc_to_u8!(D), kc_to_u8!(B), kc_to_u8!(C), 0, 0, 0],
            ))
            .expect_keyboard_report(crate::common::report(0, [0, kc_to_u8!(B), kc_to_u8!(C), 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, kc_to_u8!(C), 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_flow_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(30)
            .release(0, 0) // Release A
            .delay(20)
            .press(0, 1) // Press mt!(B, LShift) -> Flow Tap
            .delay(10)
            .press(0, 2) // Press mt!(C, LGui) -> Flow Tap
            .delay(40)
            .release(0, 1) // Release B
            .delay(10)
            .release(0, 2) // Release C
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), kc_to_u8!(C), 0, 0, 0, 0])) // Press C
            .expect_keyboard_report(crate::common::report(0, [0, kc_to_u8!(C), 0, 0, 0, 0])) // Release B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release C
            .run()
            .await;
    });
}

// Ref: https://github.com/HaoboGu/rmk/pull/496
#[test]
fn test_previous_rolling_keypress() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press A
            .delay(150)
            .press(0, 3) // Press lt!(1, D)
            .delay(30)
            .release(0, 0) // Release A
            .delay(150)
            .press(0, 1) // Press Kp2 on layer 1
            .delay(40)
            .release(0, 1) // Release Kp2 on layer 1
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp2), 0, 0, 0, 0, 0])) // Press Kp2
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release Kp2
            .run()
            .await;
    });
}

#[test]
fn test_multi_hold_cross_hand() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(150)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A -> Permisive hold
            .delay(40)
            .release(0, 2) // Release Kp2 on layer 1
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(KC_LGUI, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LGUI, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LGUI, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_flow_tap_misorder() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(120)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .release(0, 2) // Release mt!(C, LGui)
            .delay(10)
            .press(0, 4) // Press td!(0) -> Flow Tap triggered
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .delay(10)
            .release(0, 4) // Release td!(0)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), kc_to_u8!(E), 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, kc_to_u8!(E), 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_mt_lt_combination() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(130)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(130)
            .press(0, 3) // Press lt!(1, D)
            .delay(130)
            .press(0, 0) // Press Kp4 on layer1
            .delay(130)
            .release(0, 0) // Release Kp4 on layer1
            .delay(200)
            .release(0, 3) // Release lt!(1, D)
            .delay(10)
            .release(0, 1) // Release mt!(C, LGui)
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_lt_opposite_hand_roll_permissive_hold() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(130)
            .press(0, 3) // Press lt!(1, D)
            .delay(20)
            .press(0, 0) // Press Kp1 on layer1
            .delay(20)
            .press(0, 1) // Press Kp2 on layer1
            .delay(20)
            .release(0, 0) // Release Kp1 on layer1
            .delay(20)
            .release(0, 1) // Release Kp2 on layer1
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp2), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_lt_opposite_hand_sequence_permissive_hold() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(130)
            .press(0, 3) // Press lt!(1, D)
            .delay(20)
            .press(0, 0) // Press Kp1 on layer1
            .delay(20)
            .release(0, 0) // Release Kp1 on layer1
            .delay(20)
            .press(0, 1) // Press Kp2 on layer1
            .delay(20)
            .release(0, 1) // Release Kp2 on layer1
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp2), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_lt_opposite_hand_roll_permissive_hold_early_modifier_release() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_SETUP).build().await;

        keyboard
            .delay(130)
            .press(0, 3) // Press lt!(1, D)
            .delay(20)
            .press(0, 0) // Press Kp1 on layer1
            .delay(20)
            .press(0, 1) // Press Kp2 on layer1
            .delay(20)
            .release(0, 0) // Release Kp1 on layer1
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .delay(20)
            .release(0, 1) // Release B
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_release_morse_keeps_pressed_layer_no_action_after_layer_off_normal() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(RELEASE_REMAP_NORMAL_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press mo!(1) and activate layer 1 - after timeout
            .delay(10)
            .press(0, 1) // Press a!(No) from layer 0
            .delay(10)
            .release(0, 0) // Release mo!(1), layer 1 is now off (didn't activate)
            .delay(10)
            .release(0, 1) // Release a!(No)
            .run()
            .await;
    });
}

#[test]
fn test_release_morse_keeps_pressed_layer_no_action_after_layer_off_normal_timeout() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(RELEASE_REMAP_NORMAL_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press mo!(1) and activate layer 1 - after timeout
            .delay(10)
            .press(0, 1) // Press k!(B) from layer 1
            .delay(240)
            .release(0, 0) // Release mo!(1), layer 1 is now off
            .delay(10)
            .release(0, 1) // Release k!(B)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Tap B down
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Tap B up
            .run()
            .await;
    });
}

#[test]
fn test_release_morse_keeps_pressed_layer_no_action_after_layer_off_permissive_hold() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(RELEASE_REMAP_PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press mo!(1) and activate layer 1
            .delay(10)
            .press(0, 1) // Press k!(B) from layer 1
            .delay(10)
            .release(0, 0) // Release mo!(1), layer 1 is now off
            .delay(10)
            .release(0, 1) // Release k!(B)
            .run()
            .await;
    });
}

#[test]
fn test_release_morse_keeps_pressed_layer_no_action_after_layer_off_hold_on_other_press() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(RELEASE_REMAP_HOLD_ON_OTHER_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press mo!(1) and activate layer 1
            .delay(10)
            .press(0, 1) // Press k!(B) from layer 1
            .delay(10)
            .release(0, 0) // Release mo!(1), layer 1 is now off
            .delay(10)
            .release(0, 1) // Release k!(B)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Tap B down
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Tap B up
            .run()
            .await;
    });
}

#[test]
fn test_release_morse_keeps_pressed_layer_transparent_action_after_layer_off_normal() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(RELEASE_REMAP_NORMAL_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press mo!(1) and activate layer 1
            .delay(10)
            .press(0, 2) // Press k!(A) from layer 0
            .delay(10)
            .release(0, 0) // Release mo!(1), layer 1 is now off
            .delay(10)
            .release(0, 2) // Release k!(A) from layer 0
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Tap A down
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Tap A up
            .run()
            .await;
    });
}

#[test]
fn test_release_morse_keeps_pressed_layer_transparent_action_after_layer_off_permissive_hold() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(RELEASE_REMAP_PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press mo!(1) and activate layer 1
            .delay(10)
            .press(0, 2) // Press a!(Transparent) from layer 1
            .delay(10)
            .release(0, 0) // Release mo!(1), layer 1 is now off
            .delay(10)
            .release(0, 2) // Release a!(Transparent)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Tap A down
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Tap A up
            .run()
            .await;
    });
}

#[test]
fn test_release_morse_keeps_pressed_layer_transparent_action_after_layer_off_hold_on_other_press() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(RELEASE_REMAP_HOLD_ON_OTHER_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press mo!(1) and activate layer 1
            .delay(10)
            .press(0, 2) // Press a!(Transparent) from layer 1
            .delay(10)
            .release(0, 0) // Release mo!(1), layer 1 is now off
            .delay(10)
            .release(0, 2) // Release a!(Transparent)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Tap A down
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Tap A up
            .run()
            .await;
    });
}

/// Same-hand roll in Normal mode: mt!(B, LShift) (col 1, Left) then A (col 0, Left).
/// The HRM tap must fire BEFORE the plain key so the roll comes out in the pressed order.
/// Previously, Normal mode + unilateral_tap only resolved on key-release, causing the
/// plain key to fire first (wrong order).
#[test]
fn test_normal_mode_same_hand_roll_order() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(MORSE_KEYMAP).setup(HRM_NORMAL_SETUP).build().await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift) — HRM, Left hand
            .delay(10)
            .press(0, 0) // Press A — plain key, Left hand (same-hand roll)
            .delay(10)
            .release(0, 0) // Release A
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // B fires first (unilateral tap on press)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), kc_to_u8!(A), 0, 0, 0, 0])) // A fires after
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // A released
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // B released
            .run()
            .await;
    });
}
