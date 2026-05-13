/// Test cases for tap-dance like morses
pub mod common;

use embassy_time::Duration;
use heapless::Vec;
use rmk::config::{BehaviorConfig, MorsesConfig, PositionalConfig};
use rmk::keyboard::Keyboard;
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};
use rmk::types::modifier::ModifierCombination;
use rmk::types::morse::{Morse, MorseMode, MorseProfile};
use rmk::{k, td};

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
