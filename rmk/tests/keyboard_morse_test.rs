pub mod common;

use rmk::sim::SimKeyboard;

use rmk::k;
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};
use rmk::types::modifier::ModifierCombination;

use crate::common::morse::SIMPLE_MORSE_SETUP;
use crate::common::{KC_LGUI, KC_LSHIFT, TEST_KEYMAP};

#[test]
fn test_morse_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
fn test_morse_hold() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
fn test_morse_mt_1() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release B
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), kc_to_u8!(B), 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Release B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
fn test_morse_mt_4() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
fn test_morse_mt_5() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
fn test_morse_mt_6() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
fn test_morse_mt_timeout_1() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Timeout
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_timeout_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_timeout_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
fn test_morse_mt_timeout_4() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
fn test_morse_mt_timeout_5() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
fn test_morse_mt_timeout_6() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(270)
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
fn test_morse_mt_timeout_7() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
fn test_morse_mt_timeout_8() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Timeout
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_timeout_9() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
fn test_morse_mt_timeout_10() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
            .setup(SIMPLE_MORSE_SETUP)
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
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), 0, 0, 0, 0, 0])) // Press D
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release D
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), kc_to_u8!(D), 0, 0, 0, 0])) // Press D
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Release D
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
            .setup(SIMPLE_MORSE_SETUP)
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
            .setup(SIMPLE_MORSE_SETUP)
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
            .setup(SIMPLE_MORSE_SETUP)
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
            .setup(SIMPLE_MORSE_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A
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
fn test_morse_lt_timeout_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A
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
fn test_morse_lt_timeout_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
            .setup(SIMPLE_MORSE_SETUP)
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
            .setup(SIMPLE_MORSE_SETUP)
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
            .setup(SIMPLE_MORSE_SETUP)
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
            .setup(SIMPLE_MORSE_SETUP)
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
            .setup(SIMPLE_MORSE_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A
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
fn test_morse_lt_timeout_9() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
            .setup(SIMPLE_MORSE_SETUP)
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
fn test_morse_multi_hold() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, lshift)
            .delay(10)
            .press(0, 2) // Press mt!(C, lgui)
            .delay(270)
            .press(0, 0) // Press A (after hold timeout)
            .delay(290)
            .release(0, 0) // Release A
            .delay(380)
            .release(0, 1) // Release B
            .delay(400)
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
fn test_morse_hold_after_last_tapping() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(100)
            .release(0, 1) // Release B
            .delay(100)
            .press(0, 1) // Hold mt!(B, LShift) after tapping
            .delay(400)
            .release(0, 1)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release B
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release B
            .run()
            .await;
    });
}

#[test]
fn test_morse_hold_after_last_tapping_timeout() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(100)
            .release(0, 1) // Release B
            .delay(300)
            .press(0, 1) // Hold mt!(B, LShift) after tapping timeout
            .delay(400)
            .release(0, 1)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release B
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Press LShift
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release LShift
            .run()
            .await;
    });
}

#[test]
fn test_morse_rolling() {
    // For normal mode, each morse keys are independently resolved
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
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
            .delay(10)
            .release(0, 1) // Release B
            .delay(150)
            .release(0, 2) // Release C (timeout)
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(D), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(B), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LGUI, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_morse_with_combo() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
            .combo_global(
                [
                    KeyAction::TapHold(
                        Action::Key(KeyCode::Hid(HidKeyCode::B)),
                        Action::Modifier(ModifierCombination::LSHIFT),
                        Default::default(),
                    ),
                    KeyAction::TapHold(
                        Action::Key(KeyCode::Hid(HidKeyCode::C)),
                        Action::Modifier(ModifierCombination::LGUI),
                        Default::default(),
                    ),
                ],
                k!(X),
            )
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
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_morse_with_combo_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
            .combo_global(
                [
                    KeyAction::TapHold(
                        Action::Key(KeyCode::Hid(HidKeyCode::B)),
                        Action::Modifier(ModifierCombination::LSHIFT),
                        Default::default(),
                    ),
                    KeyAction::TapHold(
                        Action::Key(KeyCode::Hid(HidKeyCode::C)),
                        Action::Modifier(ModifierCombination::LGUI),
                        Default::default(),
                    ),
                ],
                k!(X),
            )
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
fn test_morse_abc_c() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
            .build()
            .await;

        keyboard
            .delay(300)
            .press(0, 4)
            .delay(300)
            .release(0, 4) //-
            .delay(80)
            .press(0, 4)
            .delay(80)
            .release(0, 4) //.
            .delay(80)
            .press(0, 4)
            .delay(300)
            .release(0, 4) //-
            .delay(80)
            .press(0, 4)
            .delay(80)
            .release(0, 4) //.
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_morse_abc_s_o_s() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
            .build()
            .await;

        keyboard
            .delay(300)
            .press(0, 4)
            .delay(10)
            .release(0, 4) //.
            .delay(10)
            .press(0, 4)
            .delay(10)
            .release(0, 4) //.
            .delay(10)
            .press(0, 4)
            .delay(10)
            .release(0, 4) //.
            .delay(300)
            .press(0, 4)
            .delay(300)
            .release(0, 4) //-
            .delay(10)
            .press(0, 4)
            .delay(300)
            .release(0, 4) //-
            .delay(10)
            .press(0, 4)
            .delay(300)
            .release(0, 4) //-
            .delay(300)
            .press(0, 4)
            .delay(10)
            .release(0, 4) //.
            .delay(10)
            .press(0, 4)
            .delay(10)
            .release(0, 4) //.
            .delay(10)
            .press(0, 4)
            .delay(10)
            .release(0, 4) //.
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(S), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(O), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(S), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}

#[test]
fn test_morse_rmk() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
            .build()
            .await;

        keyboard
            .delay(300)
            .press(0, 4)
            .delay(10)
            .release(0, 4) //.
            .delay(10)
            .press(0, 4)
            .delay(300)
            .release(0, 4) //-
            .delay(10)
            .press(0, 4)
            .delay(10)
            .release(0, 4) //.
            .delay(300)
            .press(0, 4)
            .delay(300)
            .release(0, 4) //-
            .delay(10)
            .press(0, 4)
            .delay(300)
            .release(0, 4) //-
            .delay(300)
            .press(0, 4)
            .delay(300)
            .release(0, 4) //-
            .delay(10)
            .press(0, 4)
            .delay(10)
            .release(0, 4) //.
            .delay(10)
            .press(0, 4)
            .delay(300)
            .release(0, 4) //-
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(R), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(M), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(K), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .run()
            .await;
    });
}
