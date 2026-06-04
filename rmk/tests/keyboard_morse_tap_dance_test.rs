/// Test cases for tap-dance like morses
pub mod common;

use embassy_time::Duration;
use heapless::Vec;
use rmk::config::{BehaviorConfig, Hand, MorsesConfig, PositionalConfig};
use rmk::keyboard::Keyboard;
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};
use rmk::types::modifier::ModifierCombination;
use rmk::types::morse::{HOLD, Morse, MorseMode, MorseProfile};
use rmk::{a, k, td};

use crate::common::wrap_keymap;

pub fn create_tap_dance_test_keyboard() -> Keyboard<'static> {
    let keymap = [
        [[td!(0), td!(1), td!(2), k!(A)]],
        [[k!(Kp1), k!(Kp2), k!(Kp3), k!(Kp4)]],
    ];

    let behavior_config = BehaviorConfig {
        morse: MorsesConfig {
            enable_flow_tap: false,
            default_profile: MorseProfile::new(
                Some(false),
                Some(MorseMode::HoldOnOtherPress),
                Some(250u16),
                Some(250u16),
            ),
            morses: Vec::from_slice(&[
                Morse::new_from_vial(
                    Action::Key(KeyCode::Hid(HidKeyCode::A)),
                    Action::Key(KeyCode::Hid(HidKeyCode::B)),
                    Action::Key(KeyCode::Hid(HidKeyCode::C)),
                    Action::Key(KeyCode::Hid(HidKeyCode::D)),
                    MorseProfile::const_default(),
                ),
                Morse::new_from_vial(
                    Action::Key(KeyCode::Hid(HidKeyCode::X)),
                    Action::Key(KeyCode::Hid(HidKeyCode::Y)),
                    Action::Key(KeyCode::Hid(HidKeyCode::Z)),
                    Action::Key(KeyCode::Hid(HidKeyCode::Space)),
                    MorseProfile::const_default(),
                ),
                Morse::new_from_vial(
                    Action::Key(KeyCode::Hid(HidKeyCode::Kp1)),
                    Action::Modifier(ModifierCombination::LSHIFT),
                    Action::Key(KeyCode::Hid(HidKeyCode::Kp2)),
                    Action::Modifier(ModifierCombination::LGUI),
                    MorseProfile::const_default(),
                ),
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

/// Create a keyboard with a morse key at (0,4) td!(0) that has:
///   tap = Enter, hold_after_tap = Enter (no double_tap)
/// This triggers the early fire optimization: tap fires immediately on release,
/// hold_after_tap still works on re-press.
/// Uses HoldOnOtherPress mode to reproduce the double-press bug scenario.
fn create_early_fire_keyboard() -> Keyboard<'static> {
    let keymap = [
        [[k!(A), k!(B), k!(C), k!(D), KeyAction::Morse(0), KeyAction::Morse(1)]],
        [[k!(Kp1), k!(Kp2), k!(Kp3), k!(Kp4), k!(Kp5), k!(Kp6)]],
    ];

    let behavior_config = BehaviorConfig {
        morse: MorsesConfig {
            morses: Vec::from_slice(&[
                Morse::new_from_vial(
                    Action::Key(KeyCode::Hid(HidKeyCode::Enter)),
                    Action::Key(KeyCode::Hid(HidKeyCode::B)),
                    Action::Key(KeyCode::Hid(HidKeyCode::Enter)),
                    Action::No,
                    MorseProfile::const_default(),
                ),
                Morse::new_from_vial(
                    Action::Key(KeyCode::Hid(HidKeyCode::E)),
                    Action::Key(KeyCode::Hid(HidKeyCode::LShift)),
                    Action::Key(KeyCode::Hid(HidKeyCode::E)),
                    Action::No,
                    MorseProfile::const_default(),
                ),
            ])
            .unwrap(),
            enable_flow_tap: false,
            default_profile: MorseProfile::new(
                Some(false),
                Some(MorseMode::HoldOnOtherPress),
                Some(250u16),
                Some(250u16),
            ),
            ..MorsesConfig::default()
        },
        ..Default::default()
    };

    let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(behavior_config));
    let per_key_config: &'static PositionalConfig<1, 6> = Box::leak(Box::new(PositionalConfig::default()));
    Keyboard::new(wrap_keymap(keymap, per_key_config, behavior_config))
}

/// Create a keyboard with permissive hold mode for testing key ordering.
///   td!(0): tap=A, hold=B, hold_after_tap=C, double_tap=D
///   Normal keys: k!(E) at (0,1), k!(F) at (0,2)
fn create_permissive_hold_keyboard() -> Keyboard<'static> {
    let keymap = [[[td!(0), k!(E), k!(F), k!(A)]], [[k!(Kp1), k!(Kp2), k!(Kp3), k!(Kp4)]]];

    let behavior_config = BehaviorConfig {
        morse: MorsesConfig {
            enable_flow_tap: false,
            default_profile: MorseProfile::new(
                Some(false),
                Some(MorseMode::PermissiveHold),
                Some(250u16),
                Some(250u16),
            ),
            morses: Vec::from_slice(&[Morse::new_from_vial(
                Action::Key(KeyCode::Hid(HidKeyCode::A)),
                Action::Key(KeyCode::Hid(HidKeyCode::B)),
                Action::Key(KeyCode::Hid(HidKeyCode::C)),
                Action::Key(KeyCode::Hid(HidKeyCode::D)),
                MorseProfile::const_default(),
            )])
            .unwrap(),
            ..Default::default()
        },
        ..Default::default()
    };

    let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(behavior_config));
    let per_key_config: &'static PositionalConfig<1, 4> = Box::leak(Box::new(PositionalConfig::default()));
    Keyboard::new(wrap_keymap(keymap, per_key_config, behavior_config))
}

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
        KeyAction::TapHold(key_action(HidKeyCode::C), Action::LayerOn(3), hrm_profile)
    };

    #[rustfmt::skip]
    let dusk_layer = [
        [k!(B),     k!(F), k!(L), k!(P), k!(Q),     k!(Quote),     k!(W),     k!(O),     k!(U),   k!(Y)],
        [td!(2),    td!(3), td!(4), td!(5), k!(K),  k!(J),         c_action,  td!(7),    td!(8),  td!(9)],
        [k!(X),     k!(V), k!(M), k!(D), k!(Z),     k!(Minus),     k!(G),     k!(Comma), k!(Dot), k!(Slash)],
        [a!(No),    a!(No), td!(0), KeyAction::TapHold(key_action(HidKeyCode::R), Action::LayerOn(1), ph_profile), k!(Enter), k!(Backspace), td!(1), k!(Grave), a!(No), a!(No)],
    ];
    let no_layer = [[a!(No); 10]; 4];
    let keymap = [dusk_layer, no_layer, no_layer, no_layer, no_layer];

    let behavior_config = BehaviorConfig {
        morse: MorsesConfig {
            enable_flow_tap: true,
            prior_idle_time: Duration::from_millis(120),
            default_profile: MorseProfile::new(None, Some(MorseMode::Normal), Some(250u16), Some(250u16)),
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

#[test]
fn test_tap() {
    key_sequence_test! {
        keyboard: create_tap_dance_test_keyboard(),
        sequence: [
            [0, 0, true, 150],  // Press td!(0)
            [0, 0, false, 10], // Release td!(0)
        ],
        expected_reports: [
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn test_hold() {
    key_sequence_test! {
        keyboard: create_tap_dance_test_keyboard(),
        sequence: [
            [0, 0, true, 150],  // Press td!(0)
            [0, 0, false, 300], // Release td!(0)
        ],
        expected_reports: [
            [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn test_hold_after_tap() {
    key_sequence_test! {
        keyboard: create_tap_dance_test_keyboard(),
        sequence: [
            [0, 0, true, 150], // Press td!(0)
            [0, 0, false, 240], // Release td!(0)
            [0, 0, true, 240], // Press td!(0)
            [0, 0, false, 300], // Release td!(0)
        ],
        expected_reports: [
            [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn test_double_tap() {
    key_sequence_test! {
        keyboard: create_tap_dance_test_keyboard(),
        sequence: [
            [0, 0, true, 150],  // Press td!(0)
            [0, 0, false, 200], // Release td!(0)
            [0, 0, true, 200],  // Press td!(0)
            [0, 0, false, 200], // Release td!(0)
        ],
        expected_reports: [
            [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn test_tap_on_other_press() {
    key_sequence_test! {
        keyboard: create_tap_dance_test_keyboard(),
        sequence: [
            [0, 1, true, 150],  // Press td!(1)
            [0, 1, false, 10], // Release td!(1)
            [0, 3, true, 10], // Press A
            [0, 3, false, 10], // Press A
        ],
        expected_reports: [
            [0, [kc_to_u8!(X), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn test_hold_on_other_press() {
    key_sequence_test! {
        keyboard: create_tap_dance_test_keyboard(),
        sequence: [
            [0, 1, true, 150],  // Press td!(1)
            [0, 3, true, 10], // Press A
            [0, 3, false, 10], // Press A
            [0, 1, false, 10], // Release td!(1)
        ],
        expected_reports: [
            [0, [kc_to_u8!(Y), 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(Y), kc_to_u8!(A), 0, 0, 0, 0]],
            [0, [kc_to_u8!(Y), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn test_hold_after_tap_on_other_press() {
    key_sequence_test! {
        keyboard: create_tap_dance_test_keyboard(),
        sequence: [
            [0, 1, true, 150],  // Press td!(1)
            [0, 1, false, 100], // Release td!(1)
            [0, 1, true, 100],  // Press td!(1)
            [0, 3, true, 10], // Press A
            [0, 3, false, 10], // Press A
            [0, 1, false, 10], // Release td!(1)
        ],
        expected_reports: [
            [0, [kc_to_u8!(Z), 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(Z), kc_to_u8!(A), 0, 0, 0, 0]],
            [0, [kc_to_u8!(Z), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn test_multiple_tap() {
    key_sequence_test! {
        keyboard: create_tap_dance_test_keyboard(),
        sequence: [
            [0, 0, true, 150],  // Press td!(0)
            [0, 0, false, 10], // Release td!(0)
            [0, 0, true, 260],  // Press td!(0)
            [0, 0, false, 10], // Release td!(0)
            [0, 1, true, 260],  // Press td!(1)
            [0, 1, false, 10], // Release td!(1)
        ],
        expected_reports: [
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(X), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn test_tap_after_double_tap() {
    key_sequence_test! {
        keyboard: create_tap_dance_test_keyboard(),
        sequence: [
            [0, 0, true, 150],  // Press td!(0)
            [0, 0, false, 10], // Release td!(0)
            [0, 0, true, 150],  // Press td!(0)
            [0, 0, false, 10], // Release td!(0)
            [0, 0, true, 260],  // Press td!(0)
            [0, 0, false, 10], // Release td!(0)
        ],
        expected_reports: [
            [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn test_rolling() {
    key_sequence_test! {
        keyboard: create_tap_dance_test_keyboard(),
        sequence: [
            [0, 0, true, 150], // Press td!(0)
            [0, 0, false, 10], // Release td!(0)
            [0, 0, true, 150], // Press td!(0)
            [0, 1, true, 10], // Press td!(1) -> Trigger hold-after-tap of td!(0)
            [0, 0, false, 100], // Release td!(0)
            [0, 1, false, 10], // Release td!(1)
        ],
        expected_reports: [
            [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(X), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn test_rolling_2() {
    key_sequence_test! {
        keyboard: create_tap_dance_test_keyboard(),
        sequence: [
            [0, 0, true, 150], // Press td!(0)
            [0, 0, false, 10], // Release td!(0)
            [0, 0, true, 150], // Press td!(0)
            [0, 1, true, 260], // Press td!(1) -> td!(0) timeout
            [0, 0, false, 260], // Release td!(0) -> td!(1) timeout
            [0, 1, false, 10], // Release td!(1)
        ],
        expected_reports: [
            [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(C), kc_to_u8!(Y), 0, 0, 0, 0]],
            [0, [0, kc_to_u8!(Y), 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn test_rolling_3() {
    key_sequence_test! {
        keyboard: create_tap_dance_test_keyboard(),
        sequence: [
            [0, 0, true, 150], // Press td!(0)
            [0, 0, false, 10], // Release td!(0)
            [0, 0, true, 150], // Press td!(0)
            [0, 1, true, 260], // Press td!(1),      td!(0) timeout (tap-hold) -> press "C"
            [0, 1, false, 260], // Release td!(1) -> td(1) hold, gap -> tap "Y"
            [0, 0, false, 260], // Release td!(0) -> release "C"
        ],
        expected_reports: [
            [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(C), kc_to_u8!(Y), 0, 0, 0, 0]],
            [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn test_multiple_tap_dance_keys() {
    key_sequence_test! {
        keyboard: create_tap_dance_test_keyboard(),
        sequence: [
            [0, 0, true, 150], // Press td!(0)
            [0, 0, false, 10], // Release td!(0)
            [0, 0, true, 150], // Press td!(0)
            [0, 1, true, 10], // Press td!(1) -> Trigger hold-after-tap of td!(0)
            [0, 1, false, 10], // Release td!(1)
            [0, 0, false, 100], // Release td!(0)
        ],
        expected_reports: [
            [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(X), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn test_multiple_tap_dance_keys_2() {
    key_sequence_test! {
        keyboard: create_tap_dance_test_keyboard(),
        sequence: [
            [0, 0, true, 150], // Press td!(0)
            [0, 0, false, 10], // Release td!(0)
            [0, 0, true, 150], // Press td!(0)
            [0, 1, true, 10], // Press td!(1) -> Trigger hold-after-tap of td!(0)
            [0, 1, false, 10], // Release td!(1)
            [0, 0, false, 300], // Release td!(0) -> td!(1) Timeout!
        ],
        expected_reports: [
            [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(C), kc_to_u8!(X), 0, 0, 0, 0]],
            [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn test_multiple_tap_dance_keys_3() {
    key_sequence_test! {
        keyboard: create_tap_dance_test_keyboard(),
        sequence: [
            [0, 0, true, 150], // Press td!(0)
            [0, 0, false, 10], // Release td!(0)
            [0, 0, true, 150], // Press td!(0)
            [0, 1, true, 10], // Press td!(1) -> Trigger hold-after-tap of td!(0)
            [0, 1, false, 310], // Release td!(1) -> td!(1) Timeout!
            [0, 0, false, 10], // Release td!(0)
        ],
        expected_reports: [
            [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(C), kc_to_u8!(Y), 0, 0, 0, 0]],
            [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

/// Test that early fire does not produce double key press when another key is pressed shortly after.
///
/// Scenario: Press td!(0) (tap=Enter, hold_after_tap=Enter), release quickly (early fire triggers Enter),
/// then press normal key 'A' shortly after.
///
/// Expected: Enter press, Enter release, A press (NOT: Enter press, Enter release, Enter press, Enter release, A press)
#[test]
fn test_early_fire_no_double_press_on_next_key() {
    key_sequence_test! {
        keyboard: create_early_fire_keyboard(),
        sequence: [
            [0, 4, true, 10],   // Press td!(0) morse key
            [0, 4, false, 50],  // Release td!(0) quickly — early fire triggers Enter
            [0, 0, true, 50],   // Press A shortly after
            [0, 0, false, 10],  // Release A
            [0, 0, true, 50],   // Press A shortly after
            [0, 0, false, 300],  // Release A
        ],
        expected_reports: [
            // Early fire: Enter tap (press + release)
            [0, [kc_to_u8!(Enter), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            // Normal key A (press + release)
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            // Normal key A (press + release)
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

/// Test that after early fire, re-pressing and holding the same key triggers hold_after_tap.
///
/// Scenario: Press td!(0), release quickly (early fire triggers Enter),
/// then re-press td!(0) and hold past timeout.
///
/// Expected: Enter press, Enter release (early fire), then hold_after_tap triggers Enter again
#[test]
fn test_early_fire_then_hold_after_tap() {
    key_sequence_test! {
        keyboard: create_early_fire_keyboard(),
        sequence: [
            [0, 4, true, 10],    // Press td!(0) morse key
            [0, 4, false, 50],   // Release td!(0) quickly — early fire triggers Enter
            [0, 4, true, 50],    // Re-press td!(0)
            [0, 4, false, 300],  // Hold past timeout, then release — hold_after_tap fires Enter
        ],
        expected_reports: [
            // Early fire: Enter tap (press + release)
            [0, [kc_to_u8!(Enter), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            // hold_after_tap: Enter (hold, then release)
            [0, [kc_to_u8!(Enter), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
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
    key_sequence_test! {
        keyboard: create_permissive_hold_keyboard(),
        sequence: [
            [0, 0, true, 10],    // Press td!(0) morse key
            [0, 1, true, 10],    // Press E (buffered due to permissive hold)
            [0, 0, false, 10],   // Release td!(0) — morse key released first
            [0, 1, false, 300],  // Release E after gap timeout
        ],
        expected_reports: [
            // Morse tap fires first (A press + release via process_key_action_tap)
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            // Then normal key E fires (press via fire_held_non_morse_keys)
            [0, [kc_to_u8!(E), 0, 0, 0, 0, 0]],
            // E release
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

/// Test permissive hold: normal key released first triggers hold for the morse key.
///
/// Scenario: Press morse key (td!(0)), press normal key (E), release E first (triggers
/// permissive hold → morse resolves as hold=B), then release morse key.
#[test]
fn test_permissive_hold_normal_released_first() {
    key_sequence_test! {
        keyboard: create_permissive_hold_keyboard(),
        sequence: [
            [0, 0, true, 10],    // Press td!(0) morse key
            [0, 1, true, 10],    // Press E (buffered due to permissive hold)
            [0, 1, false, 10],   // Release E — triggers permissive hold for td!(0)
            [0, 0, false, 10],   // Release td!(0)
        ],
        expected_reports: [
            // Permissive hold: morse key resolves as hold (B press)
            [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]],
            // E fires after hold resolves (press + release via process_key_action_tap)
            [0, [kc_to_u8!(B), kc_to_u8!(E), 0, 0, 0, 0]],
            [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]],
            // Release morse key (B release)
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
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
    key_sequence_test! {
        keyboard: create_early_fire_keyboard(),
        sequence: [
            [0, 5, true, 10],    // Press td!(1) morse key
            [0, 5, false, 20],   // Release td!(1) quickly — early fire triggers E
            [0, 5, true, 20],    // Re-press td!(1)
            [0, 5, false, 20],   // quick tap — early fire triggers E again
            [0, 0, true, 20],    // Press A after 300ms (early-fired key timeout fires, cleans buffer)
            [0, 0, false, 20],   // Release A
        ],
        expected_reports: [
            [0, [kc_to_u8!(E), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(E), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
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
fn create_flow_tap_early_fire_keyboard() -> Keyboard<'static> {
    // td!(0): tap=Backspace, hold=RShift, hold_after_tap=Backspace (no double_tap).
    // tap == hold_after_tap with no double_tap makes can_fire_early() return true for TAP.
    let keymap = [[[td!(0), k!(A)]], [[k!(Kp1), k!(Kp2)]]];

    let behavior_config = BehaviorConfig {
        morse: MorsesConfig {
            enable_flow_tap: true,
            prior_idle_time: Duration::from_millis(120),
            default_profile: MorseProfile::new(Some(false), Some(MorseMode::HoldOnOtherPress), Some(250), Some(250)),
            morses: Vec::from_slice(&[Morse::new_from_vial(
                Action::Key(KeyCode::Hid(HidKeyCode::Backspace)),
                Action::Modifier(ModifierCombination::RSHIFT),
                Action::Key(KeyCode::Hid(HidKeyCode::Backspace)),
                Action::No,
                MorseProfile::const_default(),
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

#[test]
fn test_flow_tap_after_early_fire_does_not_jam() {
    key_sequence_test! {
        keyboard: create_flow_tap_early_fire_keyboard(),
        sequence: [
            // First tap arrives after >prior_idle_time of idle, so it takes the normal morse
            // path: a quick release early-fires Backspace and leaves an EarlyFired entry behind.
            [0, 0, true, 150],
            [0, 0, false, 30],
            // Second tap lands within prior_idle_time of the early-fired Backspace press, so it
            // takes the FlowTap path. FlowTap must replace the stale EarlyFired entry, not stack
            // a new one on top of it, or the release report below is dropped and Backspace jams.
            [0, 0, true, 50],
            [0, 0, false, 30],
            // Press an unrelated key well after the morse gap timeout to confirm nothing is stuck.
            [0, 1, true, 300],
            [0, 1, false, 10],
        ],
        expected_reports: [
            // First tap: early-fired Backspace press, then release 10ms later (process_key_action_tap).
            [0, [kc_to_u8!(Backspace), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            // Second tap via FlowTap: Backspace press, then release on key-up.
            // The release report was missing before the fix (Backspace stayed held -> jam).
            [0, [kc_to_u8!(Backspace), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            // Unrelated key A, cleanly pressed and released.
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
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
