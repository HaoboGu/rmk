pub mod common;

use rmk::sim::{SimKeyboard, SimKeyboardSetup};

use rmk_types::morse::{MorseMode, MorseProfile};

use crate::common::morse::{MORSE_2_KEY_COMBOS, MORSE_3_KEY_COMBOS, SIMPLE_MORSE_SETUP};
use crate::common::{KC_LGUI, KC_LSHIFT, TEST_KEYMAP};

const PERMISSIVE_HOLD_PROFILE: MorseProfile =
    MorseProfile::new(Some(false), Some(MorseMode::PermissiveHold), Some(250u16), Some(250u16));
const PERMISSIVE_HOLD_SETUP: SimKeyboardSetup<5, 14> = SIMPLE_MORSE_SETUP.morse_profile(PERMISSIVE_HOLD_PROFILE);

#[test]
fn test_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Permissive hold
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_mt_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .press(0, 0) // Press A
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
fn test_mt_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .release(0, 0) // Release A
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
fn test_mt_4() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .press(0, 0) // Press A
            .delay(260)
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
fn test_mt_timeout_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
fn test_mt_timeout_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(260)
            .release(0, 0) // Release A
            .delay(10)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Timeout
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_4() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(260)
            .release(0, 1) // Release mt!(B, LShift)
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Timeout
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_5() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(260)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Press mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_6() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .release(0, 0) // Release A
            .delay(260)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Timeout
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_8() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .delay(260)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Permissve hold
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_9() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .release(0, 0) // Release A
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
fn test_morse_lt_4() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 3) // Press lt!(1, D)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .delay(10)
            .press(0, 3) // Press lt!(1, D)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A -> timeout: Kp1 on layer 1
            .delay(260)
            .release(0, 0) // Release A
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A -> timeout: Kp1 on layer 1
            .delay(260)
            .release(0, 3) // Release lt!(1, D)
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 3) // Press lt!(1, D)
            .delay(260)
            .release(0, 0) // Release A
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_4() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 3) // Press lt!(1, D)
            .delay(260)
            .release(0, 3) // Release lt!(1, D)
            .delay(10)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_5() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .release(0, 0) // Release A
            .delay(260)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_8() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(50)
            .press(0, 0) // Press A
            .delay(10)
            .release(0, 0) // Release A
            .delay(100)
            .release(0, 1) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Hold LShift
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // All released
            .run()
            .await;
    });
}

#[test]
fn test_with_combo_1() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .combos_global(MORSE_2_KEY_COMBOS)
            .combos_global(MORSE_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(20)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .combos_global(MORSE_2_KEY_COMBOS)
            .combos_global(MORSE_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(20)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .combos_global(MORSE_2_KEY_COMBOS)
            .combos_global(MORSE_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(20)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .combos_global(MORSE_2_KEY_COMBOS)
            .combos_global(MORSE_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(20)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .combos_global(MORSE_2_KEY_COMBOS)
            .combos_global(MORSE_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(20)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .combos_global(MORSE_2_KEY_COMBOS)
            .combos_global(MORSE_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(20)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .combos_global(MORSE_2_KEY_COMBOS)
            .combos_global(MORSE_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(20)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .combos_global(MORSE_2_KEY_COMBOS)
            .combos_global(MORSE_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(20)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(20)
            .press(0, 3) // Press lt!(1, D)
            .delay(60)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(20)
            .release(0, 1) // Release B
            .delay(10)
            .release(0, 2) // Release C
            .delay(10)
            .release(0, 3) // Release D
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp3), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_timeout() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press A
            .delay(100)
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
fn test_layer_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(100)
            .release(0, 1) // Release B
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp2), 0, 0, 0, 0, 0])) // Press Kp2 on layer 1
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release Kp2
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp2), 0, 0, 0, 0, 0])) // Press Kp2 on layer 1
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release Kp2
            .run()
            .await;
    });
}

#[test]
fn test_rolling_with_layer_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .press(0, 0) // Press A
            .delay(300)
            .release(0, 1) // Release B after timeout
            .delay(10)
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
fn test_timeout_and_release() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(20)
            .press(0, 0) // Press A
            .delay(260)
            .release(0, 0) // Release A  <-- Release A after "permissive hold" interval, but also after the hold-timeout
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
fn test_timeout_and_release_with_other_morse_key() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(30)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(30)
            .press(0, 0) // Press A
            .delay(50)
            .release(0, 1) // Release mt!(B, LShift) -> In permissive hold mode, this operation resolves `B` and `A`, but not `C`
            .delay(100)
            .release(0, 2) // Release mt!(C, LGui)
            .delay(100)
            .release(0, 0) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0]))
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(30)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(30)
            .press(0, 0) // Press A
            .delay(100)
            .release(0, 2) // Release C -> Triggers permissve hold of mt!(B, LShift), `A` should also be resolved because it's a normal key press.
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(30)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(30)
            .press(0, 0) // Press A
            .delay(100)
            .release(0, 2) // Release C
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
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(30)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(30)
            .press(0, 0) // Press A
            .delay(100)
            .release(0, 0) // Release A -> Triggers permissve hold of mt!(B, LShift) and mt!(C, LGui)
            .delay(50)
            .release(0, 1) // Release B
            .delay(100)
            .release(0, 2) // Release C
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Hold LShift
            .expect_keyboard_report(crate::common::report(KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0])) // Hold LShift + LGui
            .expect_keyboard_report(crate::common::report(
                KC_LSHIFT | KC_LGUI,
                [kc_to_u8!(A), 0, 0, 0, 0, 0],
            )) // Press A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(KC_LGUI, [0, 0, 0, 0, 0, 0])) // Hold LGui
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // All released
            .run()
            .await;
    });
}

#[test]
fn test_complex_rolling() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(30)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .release(0, 0) // Release A
            .delay(30)
            .press(0, 3) // Press lt!(1, D)
            .delay(30)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(100)
            .release(0, 3) // Release D
            .delay(50)
            .release(0, 1) // Release B
            .delay(10)
            .release(0, 2) // Release C
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(D), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_flow_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(30)
            .press(0, 0) // Press A
            .delay(30)
            .release(0, 0) // Release A
            .delay(20)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(10)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(40)
            .release(0, 1) // Release B
            .delay(10)
            .release(0, 2) // Release C
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release B
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), 0, 0, 0, 0, 0])) // Press C
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release C
            .run()
            .await;
    });
}

// Ref: https://github.com/HaoboGu/rmk/pull/496
#[test]
fn test_previous_rolling_keypress() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(30)
            .press(0, 0) // Press A
            .delay(20)
            .press(0, 3) // Press lt!(1, D)
            .delay(30)
            .release(0, 0) // Release A
            .delay(20)
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
