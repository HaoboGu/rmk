/// Test cases for tap-dance like morses
pub mod common;

use embassy_time::Duration;
use heapless::Vec;
use rmk::config::{BehaviorConfig, Hand, MorsesConfig, PositionalConfig};
use rmk::keyboard::Keyboard;
use rmk::sim::{KeymapOverride, SimKeyboard, SimKeyboardSetup, SimMorseSetup};
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};
use rmk::types::modifier::ModifierCombination;
use rmk::types::morse::{HOLD, Morse, MorseMode, MorseProfile};
use rmk::{a, k, td};

use crate::common::{TEST_KEYMAP, wrap_keymap};

const TAP_DANCE_PROFILE: MorseProfile = MorseProfile::new(
    Some(false),
    Some(MorseMode::HoldOnOtherPress),
    Some(250u16),
    Some(250u16),
);
const PERMISSIVE_HOLD_PROFILE: MorseProfile =
    MorseProfile::new(Some(false), Some(MorseMode::PermissiveHold), Some(250u16), Some(250u16));

const TAP_DANCE_KEY_OVERRIDES: [KeymapOverride; 8] = [
    KeymapOverride::new(0, 0, 0, td!(0)),
    KeymapOverride::new(0, 0, 1, td!(1)),
    KeymapOverride::new(0, 0, 2, td!(2)),
    KeymapOverride::new(0, 0, 3, k!(A)),
    KeymapOverride::new(1, 0, 0, k!(Kp1)),
    KeymapOverride::new(1, 0, 1, k!(Kp2)),
    KeymapOverride::new(1, 0, 2, k!(Kp3)),
    KeymapOverride::new(1, 0, 3, k!(Kp4)),
];
const TAP_DANCE_MORSES: [(Action, Action, Action, Action, MorseProfile); 3] = [
    (
        Action::Key(KeyCode::Hid(HidKeyCode::A)),
        Action::Key(KeyCode::Hid(HidKeyCode::B)),
        Action::Key(KeyCode::Hid(HidKeyCode::C)),
        Action::Key(KeyCode::Hid(HidKeyCode::D)),
        MorseProfile::const_default(),
    ),
    (
        Action::Key(KeyCode::Hid(HidKeyCode::X)),
        Action::Key(KeyCode::Hid(HidKeyCode::Y)),
        Action::Key(KeyCode::Hid(HidKeyCode::Z)),
        Action::Key(KeyCode::Hid(HidKeyCode::Space)),
        MorseProfile::const_default(),
    ),
    (
        Action::Key(KeyCode::Hid(HidKeyCode::Kp1)),
        Action::Modifier(ModifierCombination::LSHIFT),
        Action::Key(KeyCode::Hid(HidKeyCode::Kp2)),
        Action::Modifier(ModifierCombination::LGUI),
        MorseProfile::const_default(),
    ),
];
const TAP_DANCE_SETUP: SimKeyboardSetup<5, 14> = SimKeyboardSetup::new().keys(&TAP_DANCE_KEY_OVERRIDES).morse(
    SimMorseSetup::new()
        .vial_morses(&TAP_DANCE_MORSES)
        .profile(TAP_DANCE_PROFILE),
);

/// Create a keyboard with a morse key at (0,4) td!(0) that has:
///   tap = Enter, hold_after_tap = Enter (no double_tap)
/// This triggers the early fire optimization: tap fires immediately on release,
/// hold_after_tap still works on re-press.
/// Uses HoldOnOtherPress mode to reproduce the double-press bug scenario.
const EARLY_FIRE_KEY_OVERRIDES: [KeymapOverride; 12] = [
    KeymapOverride::new(0, 0, 0, k!(A)),
    KeymapOverride::new(0, 0, 1, k!(B)),
    KeymapOverride::new(0, 0, 2, k!(C)),
    KeymapOverride::new(0, 0, 3, k!(D)),
    KeymapOverride::new(0, 0, 4, KeyAction::Morse(0)),
    KeymapOverride::new(0, 0, 5, KeyAction::Morse(1)),
    KeymapOverride::new(1, 0, 0, k!(Kp1)),
    KeymapOverride::new(1, 0, 1, k!(Kp2)),
    KeymapOverride::new(1, 0, 2, k!(Kp3)),
    KeymapOverride::new(1, 0, 3, k!(Kp4)),
    KeymapOverride::new(1, 0, 4, k!(Kp5)),
    KeymapOverride::new(1, 0, 5, k!(Kp6)),
];
const EARLY_FIRE_MORSES: [(Action, Action, Action, Action, MorseProfile); 2] = [
    (
        Action::Key(KeyCode::Hid(HidKeyCode::Enter)),
        Action::Key(KeyCode::Hid(HidKeyCode::B)),
        Action::Key(KeyCode::Hid(HidKeyCode::Enter)),
        Action::No,
        MorseProfile::const_default(),
    ),
    (
        Action::Key(KeyCode::Hid(HidKeyCode::E)),
        Action::Key(KeyCode::Hid(HidKeyCode::LShift)),
        Action::Key(KeyCode::Hid(HidKeyCode::E)),
        Action::No,
        MorseProfile::const_default(),
    ),
];
const EARLY_FIRE_SETUP: SimKeyboardSetup<5, 14> = SimKeyboardSetup::new().keys(&EARLY_FIRE_KEY_OVERRIDES).morse(
    SimMorseSetup::new()
        .vial_morses(&EARLY_FIRE_MORSES)
        .profile(TAP_DANCE_PROFILE),
);

/// Create a keyboard with permissive hold mode for testing key ordering.
///   td!(0): tap=A, hold=B, hold_after_tap=C, double_tap=D
///   Normal keys: k!(E) at (0,1), k!(F) at (0,2)
const PERMISSIVE_HOLD_KEY_OVERRIDES: [KeymapOverride; 8] = [
    KeymapOverride::new(0, 0, 0, td!(0)),
    KeymapOverride::new(0, 0, 1, k!(E)),
    KeymapOverride::new(0, 0, 2, k!(F)),
    KeymapOverride::new(0, 0, 3, k!(A)),
    KeymapOverride::new(1, 0, 0, k!(Kp1)),
    KeymapOverride::new(1, 0, 1, k!(Kp2)),
    KeymapOverride::new(1, 0, 2, k!(Kp3)),
    KeymapOverride::new(1, 0, 3, k!(Kp4)),
];
const PERMISSIVE_HOLD_MORSES: [(Action, Action, Action, Action, MorseProfile); 1] = [(
    Action::Key(KeyCode::Hid(HidKeyCode::A)),
    Action::Key(KeyCode::Hid(HidKeyCode::B)),
    Action::Key(KeyCode::Hid(HidKeyCode::C)),
    Action::Key(KeyCode::Hid(HidKeyCode::D)),
    MorseProfile::const_default(),
)];
const PERMISSIVE_HOLD_SETUP: SimKeyboardSetup<5, 14> =
    SimKeyboardSetup::new().keys(&PERMISSIVE_HOLD_KEY_OVERRIDES).morse(
        SimMorseSetup::new()
            .vial_morses(&PERMISSIVE_HOLD_MORSES)
            .profile(PERMISSIVE_HOLD_PROFILE),
    );

fn key_action(keycode: HidKeyCode) -> Action {
    Action::Key(KeyCode::Hid(keycode))
}

fn create_timeout_blocking_morse_keyboard() -> Keyboard<'static> {
    let keymap = [
        [[td!(0), k!(E), td!(1), td!(2)]],
        [[k!(Kp1), k!(Kp2), k!(Kp3), k!(Kp4)]],
    ];

    let mut hold_continues_morse = Morse::default();
    let _ = hold_continues_morse.actions.insert(HOLD, key_action(HidKeyCode::B));
    let _ = hold_continues_morse
        .actions
        .insert(HOLD.followed_by_hold(), key_action(HidKeyCode::C));

    let behavior_config = BehaviorConfig {
        morse: MorsesConfig {
            enable_flow_tap: false,
            default_profile: MorseProfile::new(
                Some(false),
                Some(MorseMode::PermissiveHold),
                Some(250u16),
                Some(250u16),
            ),
            morses: Vec::from_slice(&[
                Morse::new_from_vial(
                    key_action(HidKeyCode::A),
                    key_action(HidKeyCode::B),
                    key_action(HidKeyCode::C),
                    key_action(HidKeyCode::D),
                    MorseProfile::const_default(),
                ),
                Morse::new_from_vial(
                    key_action(HidKeyCode::X),
                    key_action(HidKeyCode::Y),
                    key_action(HidKeyCode::Z),
                    key_action(HidKeyCode::Space),
                    MorseProfile::const_default(),
                ),
                hold_continues_morse,
            ])
            .unwrap(),
            ..Default::default()
        },
        ..Default::default()
    };

    let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(behavior_config));
    let per_key_config: &'static PositionalConfig<1, 4> = Box::leak(Box::new(PositionalConfig::default()));
    Keyboard::new(wrap_keymap(keymap, per_key_config, behavior_config))
}

fn create_dusk_rollover_keyboard(use_tap_dance_c: bool) -> Keyboard<'static> {
    let hrm_profile = MorseProfile::new(Some(true), Some(MorseMode::PermissiveHold), Some(400u16), None);
    let fh_profile = MorseProfile::new(None, Some(MorseMode::HoldOnOtherPress), None, None);
    let ph_profile = MorseProfile::new(None, Some(MorseMode::PermissiveHold), None, None);
    let th_profile = MorseProfile::new(None, Some(MorseMode::Normal), None, None);

    let c_action = if use_tap_dance_c {
        td!(6)
    } else {
        KeyAction::TapHold(key_action(HidKeyCode::C), Action::LayerOn(3), 0u8)
    };

    #[rustfmt::skip]
    let dusk_layer = [
        [k!(B),     k!(F), k!(L), k!(P), k!(Q),     k!(Quote),     k!(W),     k!(O),     k!(U),   k!(Y)],
        [td!(2),    td!(3), td!(4), td!(5), k!(K),  k!(J),         c_action,  td!(7),    td!(8),  td!(9)],
        [k!(X),     k!(V), k!(M), k!(D), k!(Z),     k!(Minus),     k!(G),     k!(Comma), k!(Dot), k!(Slash)],
        [a!(No),    a!(No), td!(0), KeyAction::TapHold(key_action(HidKeyCode::R), Action::LayerOn(1), 1u8), k!(Enter), k!(Backspace), td!(1), k!(Grave), a!(No), a!(No)],
    ];
    let no_layer = [[a!(No); 10]; 4];
    let keymap = [dusk_layer, no_layer, no_layer, no_layer, no_layer];

    let behavior_config = BehaviorConfig {
        morse: MorsesConfig {
            enable_flow_tap: true,
            prior_idle_time: Duration::from_millis(120),
            default_profile: MorseProfile::new(None, Some(MorseMode::Normal), Some(250u16), Some(250u16)),
            // Index 0 = hrm_profile (C→LayerOn3 tap-hold), index 1 = ph_profile (R→LayerOn1 tap-hold).
            profiles: Vec::from_slice(&[hrm_profile, ph_profile]).unwrap(),
            // The test build's MORSE_MAX_NUM is 8; only TD(0)..TD(6) is needed for this rollover.
            morses: Vec::from_slice(&[
                Morse::new_from_vial(
                    key_action(HidKeyCode::Tab),
                    Action::LayerOn(4),
                    key_action(HidKeyCode::Tab),
                    Action::No,
                    th_profile,
                ),
                Morse::new_from_vial(
                    key_action(HidKeyCode::Space),
                    Action::LayerOn(2),
                    key_action(HidKeyCode::Space),
                    Action::No,
                    fh_profile,
                ),
                Morse::new_from_vial(
                    key_action(HidKeyCode::N),
                    Action::Modifier(ModifierCombination::LSHIFT),
                    key_action(HidKeyCode::N),
                    Action::No,
                    hrm_profile,
                ),
                Morse::new_from_vial(
                    key_action(HidKeyCode::S),
                    Action::Modifier(ModifierCombination::LCTRL),
                    key_action(HidKeyCode::S),
                    Action::No,
                    hrm_profile,
                ),
                Morse::new_from_vial(
                    key_action(HidKeyCode::H),
                    Action::Modifier(ModifierCombination::LGUI),
                    key_action(HidKeyCode::H),
                    Action::No,
                    hrm_profile,
                ),
                Morse::new_from_vial(
                    key_action(HidKeyCode::T),
                    Action::LayerOn(3),
                    key_action(HidKeyCode::T),
                    Action::No,
                    hrm_profile,
                ),
                Morse::new_from_vial(
                    key_action(HidKeyCode::C),
                    Action::LayerOn(3),
                    key_action(HidKeyCode::C),
                    Action::No,
                    hrm_profile,
                ),
            ])
            .unwrap(),
            ..Default::default()
        },
        ..Default::default()
    };

    #[rustfmt::skip]
    let hand = [
        [Hand::Left,    Hand::Left,    Hand::Left,    Hand::Left,    Hand::Left,       Hand::Right,     Hand::Right, Hand::Right,     Hand::Right,     Hand::Right],
        [Hand::Left,    Hand::Left,    Hand::Left,    Hand::Left,    Hand::Left,       Hand::Right,     Hand::Right, Hand::Right,     Hand::Right,     Hand::Right],
        [Hand::Left,    Hand::Left,    Hand::Left,    Hand::Left,    Hand::Left,       Hand::Right,     Hand::Right, Hand::Right,     Hand::Right,     Hand::Right],
        [Hand::Unknown, Hand::Unknown, Hand::Bilateral, Hand::Left, Hand::Bilateral,   Hand::Bilateral, Hand::Right, Hand::Bilateral, Hand::Unknown,   Hand::Unknown],
    ];

    let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(behavior_config));
    let per_key_config: &'static PositionalConfig<4, 10> = Box::leak(Box::new(PositionalConfig::new(hand)));
    Keyboard::new(wrap_keymap(keymap, per_key_config, behavior_config))
}

fn create_saurus_client_roll_keyboard() -> Keyboard<'static> {
    let hrm_profile = MorseProfile::new(Some(true), Some(MorseMode::PermissiveHold), Some(400u16), Some(250u16));
    let hrmsgt_profile = MorseProfile::new(Some(true), Some(MorseMode::PermissiveHold), Some(400u16), Some(100u16));
    // Isolate the profile override path: the held key opts out of flow-tap,
    // while the later key still inherits the global flow-tap setting.
    let hrmsgt_no_flow_profile = hrmsgt_profile.with_enable_flow_tap(Some(false));

    #[rustfmt::skip]
    let dusk_layer = [
        [k!(B),     k!(F), k!(L), k!(D), k!(Q),     k!(Quote), k!(W),     k!(O),  k!(U),     k!(Y)],
        [td!(0),    k!(S), k!(H), td!(1), k!(K),    k!(J),     td!(2),    k!(A), td!(3),    td!(4)],
        [k!(X),     k!(V), k!(M), k!(G), k!(Z),     k!(Minus), k!(P),     k!(Comma), k!(Dot), k!(Slash)],
        [a!(No),    a!(No), k!(Tab), k!(R), k!(Enter), k!(Backspace), k!(Space), k!(Grave), a!(No), a!(No)],
    ];
    let no_layer = [[a!(No); 10]; 4];
    let keymap = [dusk_layer, no_layer, no_layer, no_layer, no_layer];

    let behavior_config = BehaviorConfig {
        morse: MorsesConfig {
            enable_flow_tap: true,
            prior_idle_time: Duration::from_millis(120),
            default_profile: MorseProfile::new(None, Some(MorseMode::Normal), Some(250u16), Some(250u16)),
            morses: Vec::from_slice(&[
                Morse::new_from_vial(
                    key_action(HidKeyCode::N),
                    Action::Modifier(ModifierCombination::LSHIFT),
                    key_action(HidKeyCode::N),
                    Action::No,
                    hrmsgt_no_flow_profile,
                ),
                Morse::new_from_vial(
                    key_action(HidKeyCode::T),
                    Action::LayerOn(3),
                    key_action(HidKeyCode::T),
                    Action::No,
                    hrm_profile,
                ),
                Morse::new_from_vial(
                    key_action(HidKeyCode::C),
                    Action::LayerOn(3),
                    key_action(HidKeyCode::C),
                    Action::No,
                    hrm_profile,
                ),
                Morse::new_from_vial(
                    key_action(HidKeyCode::E),
                    Action::Modifier(ModifierCombination::RCTRL),
                    key_action(HidKeyCode::E),
                    Action::No,
                    hrm_profile,
                ),
                Morse::new_from_vial(
                    key_action(HidKeyCode::I),
                    Action::Modifier(ModifierCombination::RSHIFT),
                    key_action(HidKeyCode::I),
                    Action::No,
                    hrmsgt_profile,
                ),
            ])
            .unwrap(),
            ..Default::default()
        },
        ..Default::default()
    };

    #[rustfmt::skip]
    let hand = [
        [Hand::Left,    Hand::Left,    Hand::Left,    Hand::Left,    Hand::Left,       Hand::Right,     Hand::Right, Hand::Right,     Hand::Right,     Hand::Right],
        [Hand::Left,    Hand::Left,    Hand::Left,    Hand::Left,    Hand::Left,       Hand::Right,     Hand::Right, Hand::Right,     Hand::Right,     Hand::Right],
        [Hand::Left,    Hand::Left,    Hand::Left,    Hand::Left,    Hand::Left,       Hand::Right,     Hand::Right, Hand::Right,     Hand::Right,     Hand::Right],
        [Hand::Unknown, Hand::Unknown, Hand::Bilateral, Hand::Left, Hand::Bilateral,   Hand::Bilateral, Hand::Right, Hand::Bilateral, Hand::Unknown,   Hand::Unknown],
    ];

    let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(behavior_config));
    let per_key_config: &'static PositionalConfig<4, 10> = Box::leak(Box::new(PositionalConfig::new(hand)));
    Keyboard::new(wrap_keymap(keymap, per_key_config, behavior_config))
}

#[test]
fn test_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(TAP_DANCE_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press td!(0)
            .delay(10)
            .release(0, 0) // Release td!(0)
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_hold() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(TAP_DANCE_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press td!(0)
            .delay(300)
            .release(0, 0) // Release td!(0)
            .expect_keys([HidKeyCode::B])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_hold_after_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(TAP_DANCE_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press td!(0)
            .delay(240)
            .release(0, 0) // Release td!(0)
            .delay(240)
            .press(0, 0) // Press td!(0)
            .delay(300)
            .release(0, 0) // Release td!(0)
            .expect_keys([HidKeyCode::C])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_double_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(TAP_DANCE_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press td!(0)
            .delay(200)
            .release(0, 0) // Release td!(0)
            .delay(200)
            .press(0, 0) // Press td!(0)
            .delay(200)
            .release(0, 0) // Release td!(0)
            .expect_keys([HidKeyCode::D])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_tap_on_other_press() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(TAP_DANCE_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press td!(1)
            .delay(10)
            .release(0, 1) // Release td!(1)
            .delay(10)
            .press(0, 3) // Press A
            .delay(10)
            .release(0, 3) // Press A
            .expect_keys([HidKeyCode::X])
            .expect_all_up()
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_hold_on_other_press() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(TAP_DANCE_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press td!(1)
            .delay(10)
            .press(0, 3) // Press A
            .delay(10)
            .release(0, 3) // Press A
            .delay(10)
            .release(0, 1) // Release td!(1)
            .expect_keys([HidKeyCode::Y])
            .expect_keys([HidKeyCode::Y, HidKeyCode::A])
            .expect_keys([HidKeyCode::Y])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_hold_after_tap_on_other_press() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(TAP_DANCE_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 1) // Press td!(1)
            .delay(100)
            .release(0, 1) // Release td!(1)
            .delay(100)
            .press(0, 1) // Press td!(1)
            .delay(10)
            .press(0, 3) // Press A
            .delay(10)
            .release(0, 3) // Press A
            .delay(10)
            .release(0, 1) // Release td!(1)
            .expect_keys([HidKeyCode::Z])
            .expect_keys([HidKeyCode::Z, HidKeyCode::A])
            .expect_keys([HidKeyCode::Z])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_multiple_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(TAP_DANCE_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press td!(0)
            .delay(10)
            .release(0, 0) // Release td!(0)
            .delay(260)
            .press(0, 0) // Press td!(0)
            .delay(10)
            .release(0, 0) // Release td!(0)
            .delay(260)
            .press(0, 1) // Press td!(1)
            .delay(10)
            .release(0, 1) // Release td!(1)
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .expect_keys([HidKeyCode::X])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_tap_after_double_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(TAP_DANCE_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press td!(0)
            .delay(10)
            .release(0, 0) // Release td!(0)
            .delay(150)
            .press(0, 0) // Press td!(0)
            .delay(10)
            .release(0, 0) // Release td!(0)
            .delay(260)
            .press(0, 0) // Press td!(0)
            .delay(10)
            .release(0, 0) // Release td!(0)
            .expect_keys([HidKeyCode::D])
            .expect_all_up()
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_rolling() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(TAP_DANCE_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press td!(0)
            .delay(10)
            .release(0, 0) // Release td!(0)
            .delay(150)
            .press(0, 0) // Press td!(0)
            .delay(10)
            .press(0, 1) // Press td!(1) -> Trigger hold-after-tap of td!(0)
            .delay(100)
            .release(0, 0) // Release td!(0)
            .delay(10)
            .release(0, 1) // Release td!(1)
            .expect_keys([HidKeyCode::C])
            .expect_all_up()
            .expect_keys([HidKeyCode::X])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_rolling_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(TAP_DANCE_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press td!(0)
            .delay(10)
            .release(0, 0) // Release td!(0)
            .delay(150)
            .press(0, 0) // Press td!(0)
            .delay(260)
            .press(0, 1) // Press td!(1) -> td!(0) timeout
            .delay(260)
            .release(0, 0) // Release td!(0) -> td!(1) timeout
            .delay(10)
            .release(0, 1) // Release td!(1)
            .expect_keys([HidKeyCode::C])
            .expect_keys([HidKeyCode::C, HidKeyCode::Y])
            .expect_keys([HidKeyCode::Y])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_rolling_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(TAP_DANCE_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press td!(0)
            .delay(10)
            .release(0, 0) // Release td!(0)
            .delay(150)
            .press(0, 0) // Press td!(0)
            .delay(260)
            .press(0, 1) // Press td!(1),      td!(0) timeout (tap-hold) -> press "C"
            .delay(260)
            .release(0, 1) // Release td!(1) -> td(1) hold, gap -> tap "Y"
            .delay(260)
            .release(0, 0) // Release td!(0) -> release "C"
            .expect_keys([HidKeyCode::C])
            .expect_keys([HidKeyCode::C, HidKeyCode::Y])
            .expect_keys([HidKeyCode::C])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_multiple_tap_dance_keys() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(TAP_DANCE_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press td!(0)
            .delay(10)
            .release(0, 0) // Release td!(0)
            .delay(150)
            .press(0, 0) // Press td!(0)
            .delay(10)
            .press(0, 1) // Press td!(1) -> Trigger hold-after-tap of td!(0)
            .delay(10)
            .release(0, 1) // Release td!(1)
            .delay(100)
            .release(0, 0) // Release td!(0)
            .expect_keys([HidKeyCode::C])
            .expect_all_up()
            .expect_keys([HidKeyCode::X])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_multiple_tap_dance_keys_2() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(TAP_DANCE_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press td!(0)
            .delay(10)
            .release(0, 0) // Release td!(0)
            .delay(150)
            .press(0, 0) // Press td!(0)
            .delay(10)
            .press(0, 1) // Press td!(1) -> Trigger hold-after-tap of td!(0)
            .delay(10)
            .release(0, 1) // Release td!(1)
            .delay(300)
            .release(0, 0) // Release td!(0) -> td!(1) Timeout!
            .expect_keys([HidKeyCode::C])
            .expect_keys([HidKeyCode::C, HidKeyCode::X])
            .expect_keys([HidKeyCode::C])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_multiple_tap_dance_keys_3() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(TAP_DANCE_SETUP).build().await;

        keyboard
            .delay(150)
            .press(0, 0) // Press td!(0)
            .delay(10)
            .release(0, 0) // Release td!(0)
            .delay(150)
            .press(0, 0) // Press td!(0)
            .delay(10)
            .press(0, 1) // Press td!(1) -> Trigger hold-after-tap of td!(0)
            .delay(310)
            .release(0, 1) // Release td!(1) -> td!(1) Timeout!
            .delay(10)
            .release(0, 0) // Release td!(0)
            .expect_keys([HidKeyCode::C])
            .expect_keys([HidKeyCode::C, HidKeyCode::Y])
            .expect_keys([HidKeyCode::C])
            .expect_all_up()
            .run()
            .await;
    });
}

/// Test that early fire does not produce double key press when another key is pressed shortly after.
///
/// Scenario: Press td!(0) (tap=Enter, hold_after_tap=Enter), release quickly (early fire triggers Enter),
/// then press normal key 'A' shortly after.
///
/// Expected: Enter press, Enter release, A press (NOT: Enter press, Enter release, Enter press, Enter release, A press)
#[test]
fn test_early_fire_no_double_press_on_next_key() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(EARLY_FIRE_SETUP).build().await;

        keyboard
            .delay(10)
            .press(0, 4) // Press td!(0) morse key
            .delay(50)
            .release(0, 4) // Release td!(0) quickly — early fire triggers Enter
            .delay(50)
            .press(0, 0) // Press A shortly after
            .delay(10)
            .release(0, 0) // Release A
            .delay(50)
            .press(0, 0) // Press A again before the early-fire gap expires
            .delay(300)
            .release(0, 0) // Release A after the early-fire gap expires
            .expect_keys([HidKeyCode::Enter])
            .expect_all_up()
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .run()
            .await;
    });
}

/// Test that re-pressing an early-fired key and holding it resolves as hold-after-tap.
#[test]
fn test_early_fire_then_hold_after_tap() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(EARLY_FIRE_SETUP).build().await;

        keyboard
            .delay(10)
            .press(0, 4) // Press td!(0) morse key
            .delay(50)
            .release(0, 4) // Release quickly; early fire triggers Enter
            .delay(50)
            .press(0, 4) // Re-press td!(0)
            .delay(300)
            .release(0, 4) // Hold past timeout; hold-after-tap triggers Enter
            .expect_keys([HidKeyCode::Enter])
            .expect_all_up()
            .expect_keys([HidKeyCode::Enter])
            .expect_all_up()
            .run()
            .await;
    });
}

/// Regression test for permissive hold key ordering bug.
///
/// Scenario: Press morse key (td!(0)), press normal key (E), release morse key first, release E.
/// With permissive hold, the normal key is buffered. When the morse key is released first,
/// the morse key should resolve as tap before the normal key fires.
///
/// Expected: A (morse tap) fires first, then E fires — NOT E then A.
#[test]
fn test_permissive_hold_morse_released_first_key_order() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press td!(0) morse key
            .delay(10)
            .press(0, 1) // Press E (buffered due to permissive hold)
            .delay(10)
            .release(0, 0) // Release td!(0) — morse key released first
            .delay(300)
            .release(0, 1) // Release E after gap timeout
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .expect_keys([HidKeyCode::E])
            .expect_all_up()
            .run()
            .await;
    });
}

/// Test permissive hold: normal key released first triggers hold for the morse key.
///
/// Scenario: Press morse key (td!(0)), press normal key (E), release E first (triggers
/// permissive hold → morse resolves as hold=B), then release morse key.
#[test]
fn test_permissive_hold_normal_released_first() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(PERMISSIVE_HOLD_SETUP)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0) // Press td!(0) morse key
            .delay(10)
            .press(0, 1) // Press E (buffered due to permissive hold)
            .delay(10)
            .release(0, 1) // Release E — triggers permissive hold for td!(0)
            .delay(10)
            .release(0, 0) // Release td!(0)
            .expect_keys([HidKeyCode::B])
            .expect_keys([HidKeyCode::B, HidKeyCode::E])
            .expect_keys([HidKeyCode::B])
            .expect_all_up()
            .run()
            .await;
    });
}

/// Regression for timeout cleanup: when one morse key times out, buffered normal
/// keys must still wait behind any other unresolved morse key.
#[test]
fn test_timeout_does_not_flush_normal_keys_before_released_morse() {
    key_sequence_test! {
        keyboard: create_timeout_blocking_morse_keyboard(),
        sequence: [
            [0, 0, true, 10],    // Press TD(0)
            [0, 1, true, 10],    // Press E, buffered by TD(0)
            [0, 2, true, 10],    // Press TD(1), also buffered
            [0, 0, false, 10],   // Release TD(0), now released-but-unresolved
            [0, 2, false, 300],  // After TD(1) hold timeout and TD(0) gap timeout
            [0, 1, false, 10],   // Release E
        ],
        expected_reports: [
            // TD(1) hold timeout fires first, but E must remain buffered because
            // TD(0) is still waiting for its gap timeout.
            [0, [kc_to_u8!(Y), 0, 0, 0, 0, 0]],
            // TD(0) gap timeout resolves as tap=A.
            [0, [kc_to_u8!(Y), kc_to_u8!(A), 0, 0, 0, 0]],
            [0, [kc_to_u8!(Y), 0, 0, 0, 0, 0]],
            // Only after TD(0) resolves may buffered E fire.
            [0, [kc_to_u8!(Y), kc_to_u8!(E), 0, 0, 0, 0]],
            // Release TD(1)'s hold, then release E.
            [0, [0, kc_to_u8!(E), 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

/// Regression for timeout cleanup: a morse key that reached hold timeout can
/// still be unresolved if a longer hold pattern exists.
#[test]
fn test_timeout_does_not_flush_normal_keys_before_holding_morse() {
    key_sequence_test! {
        keyboard: create_timeout_blocking_morse_keyboard(),
        sequence: [
            [0, 3, true, 10],    // Press TD(2)
            [0, 1, true, 10],    // Press E, buffered by TD(2)
            [0, 3, false, 300],  // Release TD(2) after its unresolved hold timeout
            [0, 1, false, 300],  // Release E after TD(2)'s gap timeout
        ],
        expected_reports: [
            // TD(2)'s hold timeout enters Holding(HOLD), but HOLD can still
            // continue to hold-hold, so E must stay buffered.
            [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(E), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

/// Test that after early fire, re-pressing and release again to produce two taps.
///
/// Scenario: Press td!(1), release quickly (early fire triggers E),
/// then re-press td!(1) and release quickly again.
///
/// Expected: E press, E release (early fire), press E again
#[test]
fn test_early_fire_then_fire_on_second_tap_with_no_double_tap_config() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(EARLY_FIRE_SETUP).build().await;

        keyboard
            .delay(10)
            .press(0, 5) // Press td!(1) morse key
            .delay(20)
            .release(0, 5) // Release td!(1) quickly — early fire triggers E
            .delay(20)
            .press(0, 5) // Re-press td!(1)
            .delay(20)
            .release(0, 5) // quick tap — early fire triggers E again
            .delay(20)
            .press(0, 0) // Press A after 300ms (early-fired key timeout fires, cleans buffer)
            .delay(20)
            .release(0, 0) // Release A
            .expect_keys([HidKeyCode::E])
            .expect_all_up()
            .expect_keys([HidKeyCode::E])
            .expect_all_up()
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .run()
            .await;
    });
}

/// Regression test: rapid repeat tapping a FlowTap+EarlyFire key must not jam the key.
///
/// When flow_tap is enabled and a key has early-fire behaviour (tap == hold_after_tap,
/// no double_tap), the first quick tap fires the action immediately and leaves the key
/// in `EarlyFired` state in the held buffer. A second tap that arrives within
/// `prior_idle_time` triggers `FlowTap`, which sends the key-press report and pushes a
/// new `ProcessedButReleaseNotReportedYet` entry; without the fix it would push on
/// top of the stale `EarlyFired` entry. On release `find_pos_mut` would then find the
/// `EarlyFired` entry first and skip the release report, leaving the key held down (jam).
///
/// The fix drops any existing held entry at this position before pushing in the
/// `FlowTap` handler, so the buffer keeps its one-entry-per-position invariant and
/// the release is always reported.
// td!(0): tap=Backspace, hold=RShift, hold_after_tap=Backspace (no double_tap).
// tap == hold_after_tap with no double_tap makes can_fire_early() return true for TAP.
const FLOW_TAP_EARLY_FIRE_KEY_OVERRIDES: [KeymapOverride; 4] = [
    KeymapOverride::new(0, 0, 0, td!(0)),
    KeymapOverride::new(0, 0, 1, k!(A)),
    KeymapOverride::new(1, 0, 0, k!(Kp1)),
    KeymapOverride::new(1, 0, 1, k!(Kp2)),
];
const FLOW_TAP_EARLY_FIRE_MORSES: [(Action, Action, Action, Action, MorseProfile); 1] = [(
    Action::Key(KeyCode::Hid(HidKeyCode::Backspace)),
    Action::Modifier(ModifierCombination::RSHIFT),
    Action::Key(KeyCode::Hid(HidKeyCode::Backspace)),
    Action::No,
    MorseProfile::const_default(),
)];
const FLOW_TAP_EARLY_FIRE_SETUP: SimKeyboardSetup<5, 14> =
    SimKeyboardSetup::new().keys(&FLOW_TAP_EARLY_FIRE_KEY_OVERRIDES).morse(
        SimMorseSetup::new()
            .vial_morses(&FLOW_TAP_EARLY_FIRE_MORSES)
            .profile(TAP_DANCE_PROFILE)
            .flow_tap(true)
            .prior_idle_ms(120),
    );

#[test]
fn test_flow_tap_after_early_fire_does_not_jam() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(FLOW_TAP_EARLY_FIRE_SETUP)
            .build()
            .await;

        keyboard
            .delay(150)
            .press(0, 0)
            .delay(30)
            .release(0, 0)
            .delay(50)
            .press(0, 0)
            .delay(30)
            .release(0, 0)
            .delay(300)
            .press(0, 1)
            .delay(10)
            .release(0, 1)
            .expect_keys([HidKeyCode::Backspace])
            .expect_all_up()
            .expect_keys([HidKeyCode::Backspace])
            .expect_all_up()
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .run()
            .await;
    });
}

/// Regression test: a tap resolved by flow-tap (e.g. right after a burst of typing) must
/// still allow a hold-after-tap continuation, so press-and-hold after that tap repeats the
/// tap action instead of resolving as a fresh hold.
///
/// Before the fix, flow-tap fired the tap and removed the key from the held buffer on
/// release, leaving no trace. A subsequent press-and-hold was therefore a brand-new press
/// and resolved to the hold action (RShift here) instead of hold-after-tap (Backspace). The
/// early-fire path did not have this problem because it leaves an EarlyFired breadcrumb; the
/// fix makes flow-tapped taps leave the same breadcrumb when a hold-after-tap action exists.
#[test]
fn test_flow_tapped_tap_then_hold_after_tap() {
    key_sequence_test! {
        keyboard: create_flow_tap_early_fire_keyboard(),
        sequence: [
            // Type A, then tap td!(0) within prior_idle_time so the tap is resolved by flow-tap.
            [0, 1, true, 200],
            [0, 1, false, 30],
            [0, 0, true, 50],
            [0, 0, false, 30],
            // Re-press td!(0) within the gap timeout and hold past the hold timeout.
            // With the fix this continues into hold-after-tap (Backspace held); before it
            // resolved as a fresh hold (RShift).
            [0, 0, true, 150],
            [0, 0, false, 400],
        ],
        expected_reports: [
            // Type A.
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            // Flow-tapped tap: Backspace press (held) then release on key-up.
            [0, [kc_to_u8!(Backspace), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            // Re-press held: hold-after-tap fires Backspace (held), released on key-up.
            // RShift would mean the continuation breadcrumb was lost.
            [0, [kc_to_u8!(Backspace), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]]
        ]
    };
}

/// Regression test for a Saurus-shaped `dusk` layout while typing "client".
///
/// The final `e`/`n`/`t` roll leaves `N` pressed before `T`, while `E` only
/// resolves after `N` is already held. `T` then flow-taps because `E` just
/// produced a simple keypress. Even when `N` disables flow-tap through a
/// profile override, the later flow-tapped `T` must not overtake it, or the
/// host sees "clietn".
#[test]
fn test_saurus_client_roll_with_flow_tap_override_keeps_n_before_t() {
    key_sequence_test! {
        keyboard: create_saurus_client_roll_keyboard(),
        sequence: [
            [1, 6, true, 150],  // Press C, after prior idle time
            [1, 6, false, 30],  // Release C
            [0, 2, true, 30],   // Press L
            [0, 2, false, 30],  // Release L
            [1, 9, true, 30],   // Press I, flow-tapped after L
            [1, 9, false, 30],  // Release I
            [1, 8, true, 150],  // Press E, after prior idle time
            [1, 0, true, 40],   // Press N while E is unresolved
            [1, 8, false, 30],  // Release E, resolving it before T
            [1, 3, true, 30],   // Press T, flow-tapped after E
            [1, 0, false, 30],  // Release N
            [1, 3, false, 30],  // Release T
            [0, 9, true, 300],  // Let T's hold-after-tap breadcrumb expire, then press Y
            [0, 9, false, 30],  // Release Y
        ],
        expected_reports: [
            [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(L), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(I), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(E), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(N), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(T), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(Y), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

/// Regression test for the user's `dusk` layout rollover while typing "could".
///
/// C is configured as `TD(6)` with tap=C and hold_after_tap=C. The physical
/// rollover is C down, O down, C up, U down, O up, U up. C must not allow U
/// to overtake the already-held O.
#[test]
fn test_dusk_tap_dance_cou_rollover_keeps_o_before_u() {
    key_sequence_test! {
        keyboard: create_dusk_rollover_keyboard(true),
        sequence: [
            [1, 6, true, 150],  // Press C / TD(6), after prior idle time
            [0, 7, true, 30],   // Press O while C is held
            [1, 6, false, 30],  // Release C before U is pressed
            [0, 8, true, 30],   // Press U while O is still held
            [0, 7, false, 30],  // Release O
            [0, 8, false, 30],  // Release U
            [0, 2, true, 30],   // Press L
            [0, 2, false, 30],  // Release L
            [2, 3, true, 30],   // Press D
            [2, 3, false, 30],  // Release D
        ],
        expected_reports: [
            [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(O), 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(O), kc_to_u8!(U), 0, 0, 0, 0]],
            [0, [0, kc_to_u8!(U), 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(L), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

/// Same C/O/U rollover, but U is delayed past the early-fired C gap timeout.
/// This documents that the buffered O is flushed correctly once the gap timer
/// expires before the next key arrives.
#[test]
fn test_dusk_tap_dance_cou_rollover_after_gap_timeout() {
    key_sequence_test! {
        keyboard: create_dusk_rollover_keyboard(true),
        sequence: [
            [1, 6, true, 150],  // Press C / TD(6), after prior idle time
            [0, 7, true, 30],   // Press O while C is held
            [1, 6, false, 30],  // Release C
            [0, 8, true, 300],  // Press U after C's 250ms gap timeout
            [0, 7, false, 30],  // Release O
            [0, 8, false, 30],  // Release U
        ],
        expected_reports: [
            [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(O), 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(O), kc_to_u8!(U), 0, 0, 0, 0]],
            [0, [0, kc_to_u8!(U), 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

/// Control case for the user's observation: replacing `TD(6)` with the
/// equivalent tap-hold action keeps the text order correct for the same
/// physical rollover.
#[test]
fn test_dusk_tap_hold_cou_rollover_keeps_o_before_u() {
    key_sequence_test! {
        keyboard: create_dusk_rollover_keyboard(false),
        sequence: [
            [1, 6, true, 150],  // Press C / LT(3,C,HRM), after prior idle time
            [0, 7, true, 30],   // Press O while C is held
            [1, 6, false, 30],  // Release C before U is pressed
            [0, 8, true, 30],   // Press U while O is still held
            [0, 7, false, 30],  // Release O
            [0, 8, false, 30],  // Release U
        ],
        expected_reports: [
            [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(C), kc_to_u8!(O), 0, 0, 0, 0]],
            [0, [0, kc_to_u8!(O), 0, 0, 0, 0]],
            [0, [kc_to_u8!(U), kc_to_u8!(O), 0, 0, 0, 0]],
            [0, [kc_to_u8!(U), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}
