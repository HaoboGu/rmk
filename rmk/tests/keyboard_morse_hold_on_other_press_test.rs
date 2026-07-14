pub mod common;

use rmk::sim::{SimKeyboard, SimKeyboardSetup};
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};
use rmk::types::modifier::ModifierCombination;
use rmk::{a, k};
use rmk_types::morse::{MorseMode, MorseProfile};

use crate::common::morse::SIMPLE_MORSE_SETUP;
use crate::common::{KC_LGUI, KC_LSHIFT, TEST_KEYMAP};

const HOLD_ON_OTHER_PROFILE: MorseProfile = MorseProfile::new(
    Some(false),
    Some(MorseMode::HoldOnOtherPress),
    Some(250u16),
    Some(250u16),
);
const HOLD_ON_OTHER_SETUP: SimKeyboardSetup<5, 14> = SIMPLE_MORSE_SETUP.morse_profile(HOLD_ON_OTHER_PROFILE);
const HOLD_ON_OTHER_COMBO_KEY: KeyAction = KeyAction::TapHold(
    Action::Key(KeyCode::Hid(HidKeyCode::B)),
    Action::Modifier(ModifierCombination::LSHIFT),
    HOLD_ON_OTHER_PROFILE,
);
const HOLD_ON_OTHER_COMBO_KEY_2: KeyAction = KeyAction::TapHold(
    Action::Key(KeyCode::Hid(HidKeyCode::C)),
    Action::Modifier(ModifierCombination::LGUI),
    MorseProfile::new(Some(false), Some(MorseMode::Normal), Some(250u16), Some(250u16)),
);
const HOLD_ON_OTHER_COMBO_KEY_3: KeyAction = KeyAction::TapHold(
    Action::Key(KeyCode::Hid(HidKeyCode::D)),
    Action::LayerOn(1),
    MorseProfile::const_default(),
);
const HOLD_ON_OTHER_2_KEY_COMBOS: [([KeyAction; 2], KeyAction); 2] = [
    ([HOLD_ON_OTHER_COMBO_KEY, HOLD_ON_OTHER_COMBO_KEY_2], k!(X)),
    ([k!(A), HOLD_ON_OTHER_COMBO_KEY], k!(Y)),
];
const HOLD_ON_OTHER_3_KEY_COMBOS: [([KeyAction; 3], KeyAction); 1] = [(
    [
        HOLD_ON_OTHER_COMBO_KEY,
        HOLD_ON_OTHER_COMBO_KEY_2,
        HOLD_ON_OTHER_COMBO_KEY_3,
    ],
    k!(Z),
)];

#[test]
fn test_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
fn test_hold() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
fn test_mt_1() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_only_mods(KC_LSHIFT) // Permissive hold
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // Press A
            .expect_only_mods(KC_LSHIFT) // Release A
            .expect_all_up() // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_mt_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_only_mods(KC_LSHIFT) // Permissive hold
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // Press A
            .expect_keys([HidKeyCode::A]) // Release mt!(B, LShift)
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_mt_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
fn test_mt_4() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
fn test_mt_5() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
fn test_mt_6() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
fn test_mt_timeout_1() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_only_mods(KC_LSHIFT) // Timeout
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // Press A
            .expect_only_mods(KC_LSHIFT) // Release A
            .expect_all_up() // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_only_mods(KC_LSHIFT) // Timeout
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // Press A
            .expect_keys([HidKeyCode::A]) // Release mt!(B, LShift)
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
fn test_mt_timeout_4() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
fn test_mt_timeout_5() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
fn test_mt_timeout_6() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_only_mods(KC_LSHIFT) // Press mt!(B, LShift)
            .expect_all_up() // Release mt!(B, LShift)
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_7() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
fn test_mt_timeout_8() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_only_mods(KC_LSHIFT) // Permissve hold
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // Press A
            .expect_only_mods(KC_LSHIFT) // Release A
            .expect_all_up() // Release mt!(B, LShift)
            .run()
            .await;
    });
}

#[test]
fn test_mt_timeout_9() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
fn test_mt_timeout_10() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_keys([HidKeyCode::Kp1]) // Press Kp1
            .expect_all_up() // Release Kp1
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_keys([HidKeyCode::Kp1]) // Press Kp1
            .expect_all_up() // Release Kp1
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_keys([HidKeyCode::Kp1]) // Press A
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_keys([HidKeyCode::Kp1]) // Press A
            .expect_all_up() // Release A
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A -> Kp1 on layer 1
            .delay(10)
            .release(0, 0) // Release A
            .delay(260)
            .release(0, 3) // Release lt!(1, D)
            .expect_keys([HidKeyCode::Kp1])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_morse_lt_timeout_9() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
fn test_trigger() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_only_mods(KC_LSHIFT) // Hold LShift
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // Press A
            .expect_only_mods(KC_LSHIFT) // Release A
            .expect_all_up() // All released
            .run()
            .await;
    });
}

#[test]
fn test_with_combo_1() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
            .combos_global(HOLD_ON_OTHER_2_KEY_COMBOS)
            .combos_global(HOLD_ON_OTHER_3_KEY_COMBOS)
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
            .expect_only_mods(KC_LSHIFT)
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::C])
            .expect_only_mods(KC_LSHIFT)
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_with_combo_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
            .combos_global(HOLD_ON_OTHER_2_KEY_COMBOS)
            .combos_global(HOLD_ON_OTHER_3_KEY_COMBOS)
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
fn test_with_combo_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
            .combos_global(HOLD_ON_OTHER_2_KEY_COMBOS)
            .combos_global(HOLD_ON_OTHER_3_KEY_COMBOS)
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
            .expect_keys([HidKeyCode::X])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_with_combo_4() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
            .combos_global(HOLD_ON_OTHER_2_KEY_COMBOS)
            .combos_global(HOLD_ON_OTHER_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(20)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(60)
            .press(0, 2) // Press mt!(C, LGui) -> Resolve B, note that mt!(C, LGui) is not resolved yet
            .delay(20)
            .release(0, 1) // Release B
            .delay(10)
            .release(0, 2) // Release C -> mt!(C, LGui) is resolved now
            .expect_only_mods(KC_LSHIFT)
            .expect_all_up()
            .expect_keys([HidKeyCode::C])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_with_combo_5() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
            .combos_global(HOLD_ON_OTHER_2_KEY_COMBOS)
            .combos_global(HOLD_ON_OTHER_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(20)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(20)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(260)
            .release(0, 1) // Release B
            .delay(260)
            .release(0, 2) // Release C
            .expect_keys([HidKeyCode::X])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_with_combo_6() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
            .combos_global(HOLD_ON_OTHER_2_KEY_COMBOS)
            .combos_global(HOLD_ON_OTHER_3_KEY_COMBOS)
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(20)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(20)
            .press(0, 3) // Press lt!(1, D)
            .delay(60)
            .press(0, 2) // Press mt!(C, LGui) -> Kp3 on layer 1
            .delay(20)
            .release(0, 1) // Release B
            .delay(10)
            .release(0, 3) // Release D
            .delay(10)
            .release(0, 2) // Release C
            .expect_only_mods(KC_LSHIFT)
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::Kp3])
            .expect_keys([HidKeyCode::Kp3])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_with_combo_7() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
            .combos_global(HOLD_ON_OTHER_2_KEY_COMBOS)
            .combos_global(HOLD_ON_OTHER_3_KEY_COMBOS)
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
            .expect_keys([HidKeyCode::Z])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_with_combo_8() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
            .combos_global(HOLD_ON_OTHER_2_KEY_COMBOS)
            .combos_global(HOLD_ON_OTHER_3_KEY_COMBOS)
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
            .expect_only_mods(KC_LSHIFT)
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::Kp3])
            .expect_keys([HidKeyCode::Kp3])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_timeout() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_only_mods(KC_LSHIFT) // Hold LShift
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // Press A
            .expect_only_mods(KC_LSHIFT) // Release A
            .expect_all_up() // All released
            .run()
            .await;
    });
}

#[test]
fn test_quick_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press A
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(100)
            .release(0, 1) // Release mt!(B, LShift)
            .delay(100)
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
fn test_multi_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .expect_only_mods(KC_LSHIFT) // mt!(B, LShift)
            .expect_all_up() // Release C
            .expect_keys([HidKeyCode::C]) // mt!(B, LShift)
            .expect_all_up() // Release C
            .run()
            .await;
    });
}

#[test]
fn test_layer_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_keys([HidKeyCode::Kp2]) // Press Kp2 on layer 1
            .expect_all_up() // Release Kp2
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .expect_keys([HidKeyCode::Kp2]) // Press Kp2 on layer 1
            .expect_all_up() // Release Kp2
            .run()
            .await;
    });
}

#[test]
fn test_rolling_with_layer_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A -> Kp1 on layer 1
            .delay(10)
            .release(0, 3) // Release lt!(1, D)
            .delay(10)
            .release(0, 0) // Release A
            .delay(250)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A Press A -> Kp1 on layer 1
            .delay(10)
            .release(0, 0) // Release A
            .delay(100)
            .release(0, 3) // Release lt!(1, D)
            .delay(250)
            .press(0, 3) // Press lt!(1, D)
            .delay(10)
            .press(0, 0) // Press A Press A -> Kp1 on layer 1
            .delay(100)
            .release(0, 3) // Release lt!(1, D)
            .delay(10)
            .release(0, 0) // Release A
            .expect_keys([HidKeyCode::Kp1]) // Kp1 on layer 1
            .expect_all_up() // Release Kp1
            .expect_keys([HidKeyCode::Kp1]) // Kp1 on layer 1
            .expect_all_up() // Release Kp1
            .expect_keys([HidKeyCode::Kp1]) // Kp1 on layer 1
            .expect_all_up() // Release Kp1
            .run()
            .await;
    });
}

#[test]
fn test_timeout_rolled_release() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_only_mods(KC_LSHIFT) // Hold LShift
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // Press A
            .expect_keys([HidKeyCode::A]) // Release A
            .expect_all_up() // All released
            .run()
            .await;
    });
}

#[test]
fn test_timeout_rolled_release_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_only_mods(KC_LSHIFT) // Hold LShift
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // Press A
            .expect_keys([HidKeyCode::A]) // Release A
            .expect_all_up() // All released
            .run()
            .await;
    });
}

#[test]
fn test_timeout_and_release() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(20)
            .press(0, 0) // Press A
            .delay(260)
            .release(0, 0) // Release A
            .delay(100)
            .release(0, 1) // Release B
            .expect_only_mods(KC_LSHIFT) // Hold LShift
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // Press A
            .expect_only_mods(KC_LSHIFT) // Release A
            .expect_all_up() // All released
            .run()
            .await;
    });
}

#[test]
fn test_timeout_and_release_with_other_morse_key() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(200)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(100)
            .release(0, 2) // Release C
            .delay(100)
            .release(0, 1) // Release B
            .expect_only_mods(KC_LSHIFT) // Hold LShift
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::C]) // Press C
            .expect_only_mods(KC_LSHIFT) // Release C
            .expect_all_up() // All released
            .run()
            .await;
    });
}

#[test]
fn test_rolling_release_order() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(30)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(30)
            .press(0, 0) // Press A -> Triggers mt!(B, LShift) and mt!(C, LGui)
            .delay(50)
            .release(0, 1) // Release mt!(B, LShift)
            .delay(100)
            .release(0, 2) // Release mt!(C, LGui)
            .delay(100)
            .release(0, 0) // Release A
            .expect_only_mods(KC_LSHIFT)
            .expect_only_mods(KC_LSHIFT | KC_LGUI)
            .expect_keys_with_mods(KC_LSHIFT | KC_LGUI, [HidKeyCode::A])
            .expect_keys_with_mods(KC_LGUI, [HidKeyCode::A])
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_rolling_release_order_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(30)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(30)
            .press(0, 0) // Press A -> Triggers mt!(B, LShift) and mt!(C, LGui)
            .delay(100)
            .release(0, 2) // Release C
            .delay(50)
            .release(0, 1) // Release B
            .delay(100)
            .release(0, 0) // Release A
            .expect_only_mods(KC_LSHIFT)
            .expect_only_mods(KC_LSHIFT | KC_LGUI)
            .expect_keys_with_mods(KC_LSHIFT | KC_LGUI, [HidKeyCode::A])
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A])
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_rolling_release_order_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(30)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(30)
            .press(0, 0) // Press A -> Triggers mt!(B, LShift) and mt!(C, LGui)
            .delay(100)
            .release(0, 2) // Release C
            .delay(100)
            .release(0, 0) // Release A
            .delay(50)
            .release(0, 1) // Release B
            .expect_only_mods(KC_LSHIFT)
            .expect_only_mods(KC_LSHIFT | KC_LGUI)
            .expect_keys_with_mods(KC_LSHIFT | KC_LGUI, [HidKeyCode::A])
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A])
            .expect_only_mods(KC_LSHIFT)
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_multiple_mt_triggered() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 1) // Press mt!(B, LShift)
            .delay(30)
            .press(0, 2) // Press mt!(C, LGui)
            .delay(30)
            .press(0, 0) // Press A -> Triggers mt!(B, LShift) and mt!(C, LGui)
            .delay(100)
            .release(0, 0) // Release A
            .delay(50)
            .release(0, 1) // Release B
            .delay(100)
            .release(0, 2) // Release C
            .expect_only_mods(KC_LSHIFT)
            .expect_only_mods(KC_LSHIFT | KC_LGUI)
            .expect_keys_with_mods(KC_LSHIFT | KC_LGUI, [HidKeyCode::A])
            .expect_only_mods(KC_LSHIFT | KC_LGUI)
            .expect_only_mods(KC_LGUI)
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_complex_rolling() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .expect_only_mods(KC_LSHIFT)
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::Kp3])
            .expect_keys([HidKeyCode::Kp3])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_flow_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .expect_only_mods(KC_LSHIFT) // Press B
            .expect_all_up() // Release B
            .expect_keys([HidKeyCode::C]) // Press C
            .expect_all_up() // Release C
            .run()
            .await;
    });
}

#[test]
fn profile_flow_tap_true_overrides_global_false() {
    crate::common::test_block_on::test_block_on(async {
        let profile = HOLD_ON_OTHER_PROFILE.with_enable_flow_tap(Some(true));
        let mut keyboard = SimKeyboard::builder([[[
            k!(A),
            KeyAction::TapHold(
                Action::Key(KeyCode::Hid(HidKeyCode::B)),
                Action::Modifier(ModifierCombination::LSHIFT),
                profile,
            ),
        ]]])
        .morse_default_profile(HOLD_ON_OTHER_PROFILE)
        .morse_prior_idle_ms(120)
        .morse_flow_tap(false)
        .build()
        .await;

        keyboard
            .delay(30)
            .tap(0, 0, 30)
            .delay(20)
            .press(0, 1)
            .delay(10)
            .tap(0, 0, 10)
            .delay(10)
            .release(0, 1)
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .expect_keys([HidKeyCode::B])
            .expect_keys([HidKeyCode::B, HidKeyCode::A])
            .expect_keys([HidKeyCode::B])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn profile_flow_tap_false_overrides_global_true() {
    crate::common::test_block_on::test_block_on(async {
        let profile = HOLD_ON_OTHER_PROFILE.with_enable_flow_tap(Some(false));
        let mut keyboard = SimKeyboard::builder([[[
            k!(A),
            KeyAction::TapHold(
                Action::Key(KeyCode::Hid(HidKeyCode::B)),
                Action::Modifier(ModifierCombination::LSHIFT),
                profile,
            ),
        ]]])
        .morse_default_profile(HOLD_ON_OTHER_PROFILE)
        .morse_prior_idle_ms(120)
        .morse_flow_tap(true)
        .build()
        .await;

        keyboard
            .delay(30)
            .tap(0, 0, 30)
            .delay(20)
            .press(0, 1)
            .delay(10)
            .tap(0, 0, 10)
            .delay(10)
            .release(0, 1)
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .expect_only_mods(KC_LSHIFT)
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A])
            .expect_only_mods(KC_LSHIFT)
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn morse_profile_flow_tap_true_overrides_global_false() {
    crate::common::test_block_on::test_block_on(async {
        let profile = HOLD_ON_OTHER_PROFILE.with_enable_flow_tap(Some(true));
        let mut keyboard = SimKeyboard::builder([[[k!(A), KeyAction::Morse(0)]]])
            .morse_default_profile(HOLD_ON_OTHER_PROFILE)
            .morse_prior_idle_ms(120)
            .morse_flow_tap(false)
            .morse_from_vial(
                Action::Key(KeyCode::Hid(HidKeyCode::B)),
                Action::Modifier(ModifierCombination::LSHIFT),
                Action::No,
                Action::No,
                profile,
            )
            .build()
            .await;

        keyboard
            .delay(30)
            .tap(0, 0, 30)
            .delay(20)
            .press(0, 1)
            .delay(10)
            .tap(0, 0, 10)
            .delay(10)
            .release(0, 1)
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .expect_keys([HidKeyCode::B])
            .expect_keys([HidKeyCode::B, HidKeyCode::A])
            .expect_keys([HidKeyCode::B])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn morse_profile_flow_tap_false_overrides_global_true() {
    crate::common::test_block_on::test_block_on(async {
        let profile = HOLD_ON_OTHER_PROFILE.with_enable_flow_tap(Some(false));
        let mut keyboard = SimKeyboard::builder([[[k!(A), KeyAction::Morse(0)]]])
            .morse_default_profile(HOLD_ON_OTHER_PROFILE)
            .morse_prior_idle_ms(120)
            .morse_flow_tap(true)
            .morse_from_vial(
                Action::Key(KeyCode::Hid(HidKeyCode::B)),
                Action::Modifier(ModifierCombination::LSHIFT),
                Action::No,
                Action::No,
                profile,
            )
            .build()
            .await;

        keyboard
            .delay(30)
            .tap(0, 0, 30)
            .delay(20)
            .press(0, 1)
            .delay(10)
            .tap(0, 0, 10)
            .delay(10)
            .release(0, 1)
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .expect_only_mods(KC_LSHIFT)
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A])
            .expect_only_mods(KC_LSHIFT)
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn flow_tap_rechecks_current_key_after_held_key_changes_layer() {
    crate::common::test_block_on::test_block_on(async {
        let disabled_flow_profile = HOLD_ON_OTHER_PROFILE.with_enable_flow_tap(Some(false));
        let enabled_flow_profile = HOLD_ON_OTHER_PROFILE.with_enable_flow_tap(Some(true));
        let mut keyboard = SimKeyboard::builder([
            [[
                k!(A),
                KeyAction::TapHold(
                    Action::Key(KeyCode::Hid(HidKeyCode::D)),
                    Action::LayerOn(1),
                    disabled_flow_profile,
                ),
                KeyAction::TapHold(
                    Action::Key(KeyCode::Hid(HidKeyCode::B)),
                    Action::Modifier(ModifierCombination::LSHIFT),
                    enabled_flow_profile,
                ),
            ]],
            [[a!(Transparent), a!(Transparent), k!(Kp1)]],
        ])
        .morse_default_profile(HOLD_ON_OTHER_PROFILE)
        .morse_prior_idle_ms(120)
        .morse_flow_tap(false)
        .build()
        .await;

        keyboard
            .delay(30)
            .tap(0, 0, 30)
            .delay(20)
            .press(0, 1)
            .delay(10)
            .tap(0, 2, 10)
            .delay(10)
            .release(0, 1)
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .expect_keys([HidKeyCode::Kp1])
            .expect_all_up()
            .run()
            .await;
    });
}

// Ref: https://github.com/HaoboGu/rmk/pull/496
#[test]
fn test_previous_rolling_keypress() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_keys([HidKeyCode::A]) // Press A
            .expect_all_up() // Release A
            .expect_keys([HidKeyCode::Kp2]) // Press Kp2
            .expect_all_up() // Release Kp2
            .run()
            .await;
    });
}
