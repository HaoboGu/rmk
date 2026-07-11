pub mod common;

use embassy_time::Duration;
use heapless::Vec;
use rmk::config::{BehaviorConfig, CombosConfig, MorsesConfig, PositionalConfig};
use rmk::keyboard::Keyboard;
use rmk::keyboard::combo::{Combo, ComboConfig};
use rmk::sim::{SimKeyboard, SimKeyboardSetup};
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};
use rmk::types::modifier::ModifierCombination;
use rmk::{a, k};
use rmk_types::morse::{Morse, MorseMode, MorseProfile};

use crate::common::{KC_LGUI, KC_LSHIFT, wrap_keymap};
use crate::common::morse::SIMPLE_MORSE_SETUP;
use crate::common::TEST_KEYMAP;

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

fn create_profile_flow_tap_keyboard(
    global_enable_flow_tap: bool,
    profile_enable_flow_tap: Option<bool>,
) -> Keyboard<'static> {
    let profile = MorseProfile::new(
        Some(false),
        Some(MorseMode::HoldOnOtherPress),
        Some(250u16),
        Some(250u16),
    )
    .with_enable_flow_tap(profile_enable_flow_tap);
    let keymap = [[[
        k!(A),
        KeyAction::TapHold(
            Action::Key(KeyCode::Hid(HidKeyCode::B)),
            Action::Modifier(ModifierCombination::LSHIFT),
            0u8,
        ),
    ]]];
    let behavior_config = BehaviorConfig {
        morse: MorsesConfig {
            enable_flow_tap: global_enable_flow_tap,
            prior_idle_time: Duration::from_millis(120),
            default_profile: MorseProfile::new(
                Some(false),
                Some(MorseMode::HoldOnOtherPress),
                Some(250u16),
                Some(250u16),
            ),
            profiles: Vec::from_slice(&[profile]).unwrap(),
            ..Default::default()
        },
        ..Default::default()
    };

    let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(behavior_config));
    let per_key_config: &'static PositionalConfig<1, 2> = Box::leak(Box::new(PositionalConfig::default()));
    Keyboard::new(wrap_keymap(keymap, per_key_config, behavior_config))
}

fn create_profile_flow_tap_morse_keyboard(
    global_enable_flow_tap: bool,
    profile_enable_flow_tap: Option<bool>,
) -> Keyboard<'static> {
    let profile = MorseProfile::new(
        Some(false),
        Some(MorseMode::HoldOnOtherPress),
        Some(250u16),
        Some(250u16),
    )
    .with_enable_flow_tap(profile_enable_flow_tap);
    let keymap = [[[k!(A), KeyAction::Morse(0)]]];
    let behavior_config = BehaviorConfig {
        morse: MorsesConfig {
            enable_flow_tap: global_enable_flow_tap,
            prior_idle_time: Duration::from_millis(120),
            default_profile: MorseProfile::new(
                Some(false),
                Some(MorseMode::HoldOnOtherPress),
                Some(250u16),
                Some(250u16),
            ),
            morses: Vec::from_slice(&[Morse::new_from_vial(
                Action::Key(KeyCode::Hid(HidKeyCode::B)),
                Action::Modifier(ModifierCombination::LSHIFT),
                Action::No,
                Action::No,
                profile,
            )])
            .unwrap(),
            ..Default::default()
        },
        ..Default::default()
    };

    let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(behavior_config));
    let per_key_config: &'static PositionalConfig<1, 2> = Box::leak(Box::new(PositionalConfig::default()));
    Keyboard::new(wrap_keymap(keymap, per_key_config, behavior_config))
}

fn create_flow_tap_layer_cache_keyboard() -> Keyboard<'static> {
    let disabled_flow_profile = MorseProfile::new(
        Some(false),
        Some(MorseMode::HoldOnOtherPress),
        Some(250u16),
        Some(250u16),
    )
    .with_enable_flow_tap(Some(false));
    let enabled_flow_profile = MorseProfile::new(
        Some(false),
        Some(MorseMode::HoldOnOtherPress),
        Some(250u16),
        Some(250u16),
    )
    .with_enable_flow_tap(Some(true));
    let keymap = [
        [[
            k!(A),
            KeyAction::TapHold(Action::Key(KeyCode::Hid(HidKeyCode::D)), Action::LayerOn(1), 0u8),
            KeyAction::TapHold(
                Action::Key(KeyCode::Hid(HidKeyCode::B)),
                Action::Modifier(ModifierCombination::LSHIFT),
                1u8,
            ),
        ]],
        [[a!(Transparent), a!(Transparent), k!(Kp1)]],
    ];
    let behavior_config = BehaviorConfig {
        morse: MorsesConfig {
            enable_flow_tap: false,
            prior_idle_time: Duration::from_millis(120),
            default_profile: MorseProfile::new(
                Some(false),
                Some(MorseMode::HoldOnOtherPress),
                Some(250u16),
                Some(250u16),
            ),
            profiles: Vec::from_slice(&[disabled_flow_profile, enabled_flow_profile]).unwrap(),
            ..Default::default()
        },
        ..Default::default()
    };

    let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(behavior_config));
    let per_key_config: &'static PositionalConfig<1, 3> = Box::leak(Box::new(PositionalConfig::default()));
    Keyboard::new(wrap_keymap(keymap, per_key_config, behavior_config))
}

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
            .setup(HOLD_ON_OTHER_SETUP)
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
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Permissive hold
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Release mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
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
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0])) // Press Kp1
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release Kp1
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
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
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
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(Kp3), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp3), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
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
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(Kp3), 0, 0, 0, 0, 0]))
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
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release C
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), 0, 0, 0, 0, 0])) // mt!(B, LShift)
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release C
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
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0])) // Kp1 on layer 1
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release Kp1
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0])) // Kp1 on layer 1
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release Kp1
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0])) // Kp1 on layer 1
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release Kp1
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
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(
                KC_LSHIFT | KC_LGUI,
                [kc_to_u8!(A), 0, 0, 0, 0, 0],
            ))
            .expect_keyboard_report(crate::common::report(KC_LGUI, [kc_to_u8!(A), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
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
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(
                KC_LSHIFT | KC_LGUI,
                [kc_to_u8!(A), 0, 0, 0, 0, 0],
            ))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
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
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(
                KC_LSHIFT | KC_LGUI,
                [kc_to_u8!(A), 0, 0, 0, 0, 0],
            ))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
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
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(
                KC_LSHIFT | KC_LGUI,
                [kc_to_u8!(A), 0, 0, 0, 0, 0],
            ))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LGUI, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
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
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [kc_to_u8!(Kp3), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp3), 0, 0, 0, 0, 0]))
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0]))
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
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(KC_LSHIFT, [0, 0, 0, 0, 0, 0])) // Press B
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release B
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(C), 0, 0, 0, 0, 0])) // Press C
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release C
            .run()
            .await;
    });
}

#[test]
fn profile_flow_tap_true_overrides_global_false() {
    key_sequence_test! {
        keyboard: create_profile_flow_tap_keyboard(false, Some(true)),
        sequence: [
            [0, 0, true, 30],  // Press A
            [0, 0, false, 30], // Release A
            [0, 1, true, 20],  // Press mt!(B, LShift) -> profile Flow Tap
            [0, 0, true, 10],  // Press A while B is flow-tapped
            [0, 0, false, 10], // Release A
            [0, 1, false, 10], // Release B
        ],
        expected_reports: [
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(B), kc_to_u8!(A), 0, 0, 0, 0]],
            [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn profile_flow_tap_false_overrides_global_true() {
    key_sequence_test! {
        keyboard: create_profile_flow_tap_keyboard(true, Some(false)),
        sequence: [
            [0, 0, true, 30],  // Press A
            [0, 0, false, 30], // Release A
            [0, 1, true, 20],  // Press mt!(B, LShift), profile disables Flow Tap
            [0, 0, true, 10],  // Press A, causing hold-on-other-press
            [0, 0, false, 10], // Release A
            [0, 1, false, 10], // Release B
        ],
        expected_reports: [
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
            [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn morse_profile_flow_tap_true_overrides_global_false() {
    key_sequence_test! {
        keyboard: create_profile_flow_tap_morse_keyboard(false, Some(true)),
        sequence: [
            [0, 0, true, 30],  // Press A
            [0, 0, false, 30], // Release A
            [0, 1, true, 20],  // Press TD(0) -> profile Flow Tap
            [0, 0, true, 10],  // Press A while B is flow-tapped
            [0, 0, false, 10], // Release A
            [0, 1, false, 10], // Release TD(0)
        ],
        expected_reports: [
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(B), kc_to_u8!(A), 0, 0, 0, 0]],
            [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn morse_profile_flow_tap_false_overrides_global_true() {
    key_sequence_test! {
        keyboard: create_profile_flow_tap_morse_keyboard(true, Some(false)),
        sequence: [
            [0, 0, true, 30],  // Press A
            [0, 0, false, 30], // Release A
            [0, 1, true, 20],  // Press TD(0), profile disables Flow Tap
            [0, 0, true, 10],  // Press A, causing hold-on-other-press
            [0, 0, false, 10], // Release A
            [0, 1, false, 10], // Release TD(0)
        ],
        expected_reports: [
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
            [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn flow_tap_rechecks_current_key_after_held_key_changes_layer() {
    key_sequence_test! {
        keyboard: create_flow_tap_layer_cache_keyboard(),
        sequence: [
            [0, 0, true, 30],  // Press A
            [0, 0, false, 30], // Release A
            [0, 1, true, 20],  // Press LT(1, D), profile disables Flow Tap
            [0, 2, true, 10],  // Press flow-tap key, but held LT activates layer 1 first
            [0, 2, false, 10], // Release Kp1 from layer 1
            [0, 1, false, 10], // Release LT
        ],
        expected_reports: [
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(Kp1), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
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
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(A), 0, 0, 0, 0, 0])) // Press A
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release A
            .expect_keyboard_report(crate::common::report(0, [kc_to_u8!(Kp2), 0, 0, 0, 0, 0])) // Press Kp2
            .expect_keyboard_report(crate::common::report(0, [0, 0, 0, 0, 0, 0])) // Release Kp2
            .run()
            .await;
    });
}
