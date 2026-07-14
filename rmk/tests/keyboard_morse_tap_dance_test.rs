/// Test cases for tap-dance like morses
pub mod common;

use rmk::config::Hand;
use rmk::sim::{HandOverride, KeymapOverride, SimKeyboard, SimKeyboardSetup, SimMorseSetup};
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};
use rmk::types::modifier::ModifierCombination;
use rmk::types::morse::{HOLD, Morse, MorseMode, MorseProfile};
use rmk::{k, td};

use crate::common::TEST_KEYMAP;

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

const TIMEOUT_BLOCKING_KEYS: [KeymapOverride; 8] = [
    KeymapOverride::new(0, 0, 0, td!(0)),
    KeymapOverride::new(0, 0, 1, k!(E)),
    KeymapOverride::new(0, 0, 2, td!(1)),
    KeymapOverride::new(0, 0, 3, td!(2)),
    KeymapOverride::new(1, 0, 0, k!(Kp1)),
    KeymapOverride::new(1, 0, 1, k!(Kp2)),
    KeymapOverride::new(1, 0, 2, k!(Kp3)),
    KeymapOverride::new(1, 0, 3, k!(Kp4)),
];
const TIMEOUT_BLOCKING_MORSES: [(Action, Action, Action, Action, MorseProfile); 2] = [
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
];
const TIMEOUT_BLOCKING_SETUP: SimKeyboardSetup<5, 14> = SimKeyboardSetup::new().keys(&TIMEOUT_BLOCKING_KEYS).morse(
    SimMorseSetup::new()
        .vial_morses(&TIMEOUT_BLOCKING_MORSES)
        .profile(PERMISSIVE_HOLD_PROFILE)
        .flow_tap(false),
);

const DUSK_HRM_PROFILE: MorseProfile =
    MorseProfile::new(Some(true), Some(MorseMode::PermissiveHold), Some(400u16), None);
const DUSK_KEYS: [KeymapOverride; 5] = [
    KeymapOverride::new(0, 0, 2, k!(L)),
    KeymapOverride::new(0, 0, 7, k!(O)),
    KeymapOverride::new(0, 0, 8, k!(U)),
    KeymapOverride::new(0, 1, 6, td!(6)),
    KeymapOverride::new(0, 2, 3, k!(D)),
];
const DUSK_HANDS: [HandOverride; 5] = [
    HandOverride::new(0, 2, Hand::Left),
    HandOverride::new(0, 7, Hand::Right),
    HandOverride::new(0, 8, Hand::Right),
    HandOverride::new(1, 6, Hand::Right),
    HandOverride::new(2, 3, Hand::Left),
];
const DUSK_MORSES: [(Action, Action, Action, Action, MorseProfile); 7] = [
    (
        Action::Key(KeyCode::Hid(HidKeyCode::Tab)),
        Action::LayerOn(1),
        Action::Key(KeyCode::Hid(HidKeyCode::Tab)),
        Action::No,
        MorseProfile::new(None, Some(MorseMode::Normal), None, None),
    ),
    (
        Action::Key(KeyCode::Hid(HidKeyCode::Space)),
        Action::LayerOn(1),
        Action::Key(KeyCode::Hid(HidKeyCode::Space)),
        Action::No,
        MorseProfile::new(None, Some(MorseMode::HoldOnOtherPress), None, None),
    ),
    (
        Action::Key(KeyCode::Hid(HidKeyCode::N)),
        Action::Modifier(ModifierCombination::LSHIFT),
        Action::Key(KeyCode::Hid(HidKeyCode::N)),
        Action::No,
        DUSK_HRM_PROFILE,
    ),
    (
        Action::Key(KeyCode::Hid(HidKeyCode::S)),
        Action::Modifier(ModifierCombination::LCTRL),
        Action::Key(KeyCode::Hid(HidKeyCode::S)),
        Action::No,
        DUSK_HRM_PROFILE,
    ),
    (
        Action::Key(KeyCode::Hid(HidKeyCode::H)),
        Action::Modifier(ModifierCombination::LGUI),
        Action::Key(KeyCode::Hid(HidKeyCode::H)),
        Action::No,
        DUSK_HRM_PROFILE,
    ),
    (
        Action::Key(KeyCode::Hid(HidKeyCode::T)),
        Action::LayerOn(1),
        Action::Key(KeyCode::Hid(HidKeyCode::T)),
        Action::No,
        DUSK_HRM_PROFILE,
    ),
    (
        Action::Key(KeyCode::Hid(HidKeyCode::C)),
        Action::LayerOn(1),
        Action::Key(KeyCode::Hid(HidKeyCode::C)),
        Action::No,
        DUSK_HRM_PROFILE,
    ),
];
const DUSK_SETUP: SimKeyboardSetup<5, 14> = SimKeyboardSetup::new()
    .keys(&DUSK_KEYS)
    .hand_overrides(&DUSK_HANDS)
    .morse(
        SimMorseSetup::new()
            .vial_morses(&DUSK_MORSES)
            .profile(MorseProfile::new(
                None,
                Some(MorseMode::Normal),
                Some(250u16),
                Some(250u16),
            ))
            .flow_tap(true)
            .prior_idle_ms(120),
    );

const SAURUS_HRM_PROFILE: MorseProfile =
    MorseProfile::new(Some(true), Some(MorseMode::PermissiveHold), Some(400u16), Some(250u16));
const SAURUS_SHORT_GAP_PROFILE: MorseProfile =
    MorseProfile::new(Some(true), Some(MorseMode::PermissiveHold), Some(400u16), Some(100u16));
const SAURUS_KEYS: [KeymapOverride; 7] = [
    KeymapOverride::new(0, 0, 2, k!(L)),
    KeymapOverride::new(0, 0, 9, k!(Y)),
    KeymapOverride::new(0, 1, 0, td!(0)),
    KeymapOverride::new(0, 1, 3, td!(1)),
    KeymapOverride::new(0, 1, 6, td!(2)),
    KeymapOverride::new(0, 1, 8, td!(3)),
    KeymapOverride::new(0, 1, 9, td!(4)),
];
const SAURUS_HANDS: [HandOverride; 7] = [
    HandOverride::new(0, 2, Hand::Left),
    HandOverride::new(0, 9, Hand::Right),
    HandOverride::new(1, 0, Hand::Left),
    HandOverride::new(1, 3, Hand::Left),
    HandOverride::new(1, 6, Hand::Right),
    HandOverride::new(1, 8, Hand::Right),
    HandOverride::new(1, 9, Hand::Right),
];
const SAURUS_MORSES: [(Action, Action, Action, Action, MorseProfile); 5] = [
    (
        Action::Key(KeyCode::Hid(HidKeyCode::N)),
        Action::Modifier(ModifierCombination::LSHIFT),
        Action::Key(KeyCode::Hid(HidKeyCode::N)),
        Action::No,
        SAURUS_SHORT_GAP_PROFILE.with_enable_flow_tap(Some(false)),
    ),
    (
        Action::Key(KeyCode::Hid(HidKeyCode::T)),
        Action::LayerOn(1),
        Action::Key(KeyCode::Hid(HidKeyCode::T)),
        Action::No,
        SAURUS_HRM_PROFILE,
    ),
    (
        Action::Key(KeyCode::Hid(HidKeyCode::C)),
        Action::LayerOn(1),
        Action::Key(KeyCode::Hid(HidKeyCode::C)),
        Action::No,
        SAURUS_HRM_PROFILE,
    ),
    (
        Action::Key(KeyCode::Hid(HidKeyCode::E)),
        Action::Modifier(ModifierCombination::RCTRL),
        Action::Key(KeyCode::Hid(HidKeyCode::E)),
        Action::No,
        SAURUS_HRM_PROFILE,
    ),
    (
        Action::Key(KeyCode::Hid(HidKeyCode::I)),
        Action::Modifier(ModifierCombination::RSHIFT),
        Action::Key(KeyCode::Hid(HidKeyCode::I)),
        Action::No,
        SAURUS_SHORT_GAP_PROFILE,
    ),
];
const SAURUS_SETUP: SimKeyboardSetup<5, 14> = SimKeyboardSetup::new()
    .keys(&SAURUS_KEYS)
    .hand_overrides(&SAURUS_HANDS)
    .morse(
        SimMorseSetup::new()
            .vial_morses(&SAURUS_MORSES)
            .profile(MorseProfile::new(
                None,
                Some(MorseMode::Normal),
                Some(250u16),
                Some(250u16),
            ))
            .flow_tap(true)
            .prior_idle_ms(120),
    );

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
    crate::common::test_block_on::test_block_on(async {
        let mut hold_continues = Morse::default();
        hold_continues
            .actions
            .insert(HOLD, Action::Key(KeyCode::Hid(HidKeyCode::B)))
            .unwrap();
        hold_continues
            .actions
            .insert(HOLD.followed_by_hold(), Action::Key(KeyCode::Hid(HidKeyCode::C)))
            .unwrap();
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(TIMEOUT_BLOCKING_SETUP)
            .morse(hold_continues)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 0)
            .delay(10)
            .press(0, 1)
            .delay(10)
            .press(0, 2)
            .delay(10)
            .release(0, 0)
            .delay(300)
            .release(0, 2)
            .delay(10)
            .release(0, 1)
            .expect_keys([HidKeyCode::Y])
            .expect_keys([HidKeyCode::Y, HidKeyCode::A])
            .expect_keys([HidKeyCode::Y])
            .expect_keys([HidKeyCode::Y, HidKeyCode::E])
            .expect_keys([HidKeyCode::E])
            .expect_all_up()
            .run()
            .await;
    });
}

/// Regression for timeout cleanup: a morse key that reached hold timeout can
/// still be unresolved if a longer hold pattern exists.
#[test]
fn test_timeout_does_not_flush_normal_keys_before_holding_morse() {
    crate::common::test_block_on::test_block_on(async {
        let mut hold_continues = Morse::default();
        hold_continues
            .actions
            .insert(HOLD, Action::Key(KeyCode::Hid(HidKeyCode::B)))
            .unwrap();
        hold_continues
            .actions
            .insert(HOLD.followed_by_hold(), Action::Key(KeyCode::Hid(HidKeyCode::C)))
            .unwrap();
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(TIMEOUT_BLOCKING_SETUP)
            .morse(hold_continues)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(0, 3)
            .delay(10)
            .press(0, 1)
            .delay(300)
            .release(0, 3)
            .delay(300)
            .release(0, 1)
            .expect_keys([HidKeyCode::B])
            .expect_all_up()
            .expect_keys([HidKeyCode::E])
            .expect_all_up()
            .run()
            .await;
    });
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
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(FLOW_TAP_EARLY_FIRE_SETUP)
            .build()
            .await;

        keyboard
            .delay(200)
            .tap(0, 1, 30)
            .delay(50)
            .tap(0, 0, 30)
            .delay(150)
            .press(0, 0)
            .delay(400)
            .release(0, 0)
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .expect_keys([HidKeyCode::Backspace])
            .expect_all_up()
            .expect_keys([HidKeyCode::Backspace])
            .expect_all_up()
            .run()
            .await;
    });
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
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(SAURUS_SETUP).build().await;

        keyboard
            .delay(150)
            .tap(1, 6, 30)
            .delay(30)
            .tap(0, 2, 30)
            .delay(30)
            .tap(1, 9, 30)
            .delay(150)
            .press(1, 8)
            .delay(40)
            .press(1, 0)
            .delay(30)
            .release(1, 8)
            .delay(30)
            .press(1, 3)
            .delay(30)
            .release(1, 0)
            .delay(30)
            .release(1, 3)
            .delay(300)
            .tap(0, 9, 30)
            .expect_keys([HidKeyCode::C])
            .expect_all_up()
            .expect_keys([HidKeyCode::L])
            .expect_all_up()
            .expect_keys([HidKeyCode::I])
            .expect_all_up()
            .expect_keys([HidKeyCode::E])
            .expect_all_up()
            .expect_keys([HidKeyCode::N])
            .expect_all_up()
            .expect_keys([HidKeyCode::T])
            .expect_all_up()
            .expect_keys([HidKeyCode::Y])
            .expect_all_up()
            .run()
            .await;
    });
}

/// Regression test for the user's `dusk` layout rollover while typing "could".
///
/// C is configured as `TD(6)` with tap=C and hold_after_tap=C. The physical
/// rollover is C down, O down, C up, U down, O up, U up. C must not allow U
/// to overtake the already-held O.
#[test]
fn test_dusk_tap_dance_cou_rollover_keeps_o_before_u() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(DUSK_SETUP).build().await;

        keyboard
            .delay(150)
            .press(1, 6)
            .delay(30)
            .press(0, 7)
            .delay(30)
            .release(1, 6)
            .delay(30)
            .press(0, 8)
            .delay(30)
            .release(0, 7)
            .delay(30)
            .release(0, 8)
            .delay(30)
            .tap(0, 2, 30)
            .delay(30)
            .tap(2, 3, 30)
            .expect_keys([HidKeyCode::C])
            .expect_all_up()
            .expect_keys([HidKeyCode::O])
            .expect_keys([HidKeyCode::O, HidKeyCode::U])
            .expect_keys([HidKeyCode::U])
            .expect_all_up()
            .expect_keys([HidKeyCode::L])
            .expect_all_up()
            .expect_keys([HidKeyCode::D])
            .expect_all_up()
            .run()
            .await;
    });
}

/// Same C/O/U rollover, but U is delayed past the early-fired C gap timeout.
/// This documents that the buffered O is flushed correctly once the gap timer
/// expires before the next key arrives.
#[test]
fn test_dusk_tap_dance_cou_rollover_after_gap_timeout() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(DUSK_SETUP).build().await;

        keyboard
            .delay(150)
            .press(1, 6)
            .delay(30)
            .press(0, 7)
            .delay(30)
            .release(1, 6)
            .delay(300)
            .press(0, 8)
            .delay(30)
            .release(0, 7)
            .delay(30)
            .release(0, 8)
            .expect_keys([HidKeyCode::C])
            .expect_all_up()
            .expect_keys([HidKeyCode::O])
            .expect_keys([HidKeyCode::O, HidKeyCode::U])
            .expect_keys([HidKeyCode::U])
            .expect_all_up()
            .run()
            .await;
    });
}

/// Control case for the user's observation: replacing `TD(6)` with the
/// equivalent tap-hold action keeps the text order correct for the same
/// physical rollover.
#[test]
fn test_dusk_tap_hold_cou_rollover_keeps_o_before_u() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .setup(DUSK_SETUP)
            .key(
                0,
                1,
                6,
                KeyAction::TapHold(
                    Action::Key(KeyCode::Hid(HidKeyCode::C)),
                    Action::LayerOn(1),
                    DUSK_HRM_PROFILE,
                ),
            )
            .build()
            .await;

        keyboard
            .delay(150)
            .press(1, 6)
            .delay(30)
            .press(0, 7)
            .delay(30)
            .release(1, 6)
            .delay(30)
            .press(0, 8)
            .delay(30)
            .release(0, 7)
            .delay(30)
            .release(0, 8)
            .expect_keys([HidKeyCode::C])
            .expect_keys([HidKeyCode::C, HidKeyCode::O])
            .expect_keys([HidKeyCode::O])
            .expect_keys([HidKeyCode::U, HidKeyCode::O])
            .expect_keys([HidKeyCode::U])
            .expect_all_up()
            .run()
            .await;
    });
}
