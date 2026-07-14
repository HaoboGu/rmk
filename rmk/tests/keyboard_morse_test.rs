pub mod common;

use rmk::k;
use rmk::sim::SimKeyboard;
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};
use rmk::types::modifier::ModifierCombination;

use crate::common::morse::SIMPLE_MORSE_SETUP;
use crate::common::{KC_LGUI, KC_LSHIFT, TEST_KEYMAP};

#[test]
fn test_morse_tap() {
    crate::common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(100)
            .release(0, 1) // Release B
            .expect_keys([HidKeyCode::B]) // Press B
            .expect_all_up() // Release B
            .run()
            .await;
    });
}

#[test]
fn test_morse_hold() {
    crate::common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(SIMPLE_MORSE_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(300)
            .release(0, 1) // Release B after hold timeout
            .expect_only_mods(KC_LSHIFT) // Hold LShift
            .expect_all_up() // All released
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_1() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .expect_keys([HidKeyCode::B]) // Press B
            .expect_all_up() // Release B
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_2() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_keys([HidKeyCode::A, HidKeyCode::B]) // Press B
            .expect_keys([HidKeyCode::A]) // Release B
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_3() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .expect_keys([HidKeyCode::B]) // Press B
            .expect_all_up() // Release B
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_4() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_keys([HidKeyCode::A, HidKeyCode::B]) // Press B
            .expect_keys([HidKeyCode::A]) // Release B
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_5() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .expect_keys([HidKeyCode::B]) // Press B
            .expect_all_up() // Release B
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_6() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::B]) // Press B
            .expect_all_up() // Release B
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_timeout_1() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // Timeout
            .expect_only_mods(KC_LSHIFT) // Release A
            .expect_all_up() // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_timeout_2() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // Press mt!(B, LShift)
            .expect_keys([HidKeyCode::A]) // Release mt!(B, LShift)
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_timeout_3() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // Timeout
            .expect_only_mods(KC_LSHIFT) // Release A
            .expect_all_up() // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_timeout_4() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // Timeout
            .expect_keys([HidKeyCode::A]) // Release mt!(B, LShift)
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_timeout_5() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .expect_only_mods(KC_LSHIFT) // Press mt!(B, LShift)
            .expect_all_up() // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_timeout_6() {
    crate::common::test_block_on(async {
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
            .expect_only_mods(KC_LSHIFT) // Press mt!(B, LShift)
            .expect_all_up() // Release mt!(B, LShift)
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_timeout_7() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .expect_only_mods(KC_LSHIFT) // Timeout
            .expect_all_up() // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_timeout_8() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .expect_only_mods(KC_LSHIFT) // Timeout
            .expect_all_up() // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_timeout_9() {
    crate::common::test_block_on(async {
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
            .expect_only_mods(KC_LSHIFT) // Timeout
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // Press A
            .expect_only_mods(KC_LSHIFT) // Release A
            .expect_all_up() // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_morse_mt_timeout_10() {
    crate::common::test_block_on(async {
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
            .expect_only_mods(KC_LSHIFT) // Timeout
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // Press A
            .expect_keys([HidKeyCode::A]) // Release mt!(B, LShift)
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_1() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .expect_keys([HidKeyCode::D]) // Press D
            .expect_all_up() // Release D
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_2() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_keys([HidKeyCode::A, HidKeyCode::D]) // Press D
            .expect_keys([HidKeyCode::A]) // Release D
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_3() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .expect_keys([HidKeyCode::D]) // Press D
            .expect_all_up() // Release D
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_4() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_keys([HidKeyCode::A, HidKeyCode::D]) // Press D
            .expect_keys([HidKeyCode::A]) // Release D
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_5() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .expect_keys([HidKeyCode::D]) // Press D
            .expect_all_up() // Release D
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_6() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::D]) // Press D
            .expect_all_up() // Release D
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_1() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_2() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_3() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_4() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_5() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_6() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_7() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_8() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_9() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::Kp1]) // Press Kp1 on layer 1
            .expect_all_up() // Release Kp1
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_10() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::Kp1]) // Press Kp1 on layer 1
            .expect_all_up() // Release Kp1
            .run()
            .await;
    });
}

#[test]
fn test_morse_multi_hold() {
    crate::common::test_block_on(async {
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
            .expect_only_mods(KC_LSHIFT) // Hold LShift
            .expect_only_mods(KC_LSHIFT | KC_LGUI) // Hold LShift + LGui
            .expect_keys_with_mods(KC_LSHIFT | KC_LGUI, [HidKeyCode::A]) // Press A
            .expect_only_mods(KC_LSHIFT | KC_LGUI) // Release A
            .expect_only_mods(KC_LGUI) // Hold LGui
            .expect_all_up() // All released
            .run()
            .await;
    });
}

#[test]
fn test_morse_hold_after_last_tapping() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::B]) // Press B
            .expect_all_up() // Release B
            .expect_only_mods(KC_LSHIFT) // Press B
            .expect_all_up() // Release B
            .run()
            .await;
    });
}

#[test]
fn test_morse_hold_after_last_tapping_timeout() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::B]) // Press B
            .expect_all_up() // Release B
            .expect_only_mods(KC_LSHIFT) // Press LShift
            .expect_all_up() // Release LShift
            .run()
            .await;
    });
}

#[test]
fn test_morse_rolling() {
    // For normal mode, each morse keys are independently resolved
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .expect_keys([HidKeyCode::D])
            .expect_all_up()
            .expect_keys([HidKeyCode::B])
            .expect_all_up()
            .expect_only_mods(KC_LGUI)
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_morse_with_combo() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::C])
            .expect_all_up()
            .expect_only_mods(KC_LSHIFT)
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_morse_with_combo_2() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::X])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_morse_abc_c() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::C])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_morse_abc_s_o_s() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::S])
            .expect_all_up()
            .expect_keys([HidKeyCode::O])
            .expect_all_up()
            .expect_keys([HidKeyCode::S])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_morse_rmk() {
    crate::common::test_block_on(async {
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
            .expect_keys([HidKeyCode::R])
            .expect_all_up()
            .expect_keys([HidKeyCode::M])
            .expect_all_up()
            .expect_keys([HidKeyCode::K])
            .expect_all_up()
            .run()
            .await;
    });
}
