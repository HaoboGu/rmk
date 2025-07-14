//! Tests for the hold-on-other-key-press tap-hold feature
//!
//! This feature immediately triggers the hold action when any other non-tap-hold key
//! is pressed while a tap-hold key is being held.

pub mod common;

use rmk::config::{BehaviorConfig, TapHoldConfig};
use rmk::{k, lt, th};
use rusty_fork::rusty_fork_test;

/// Helper to create tap-hold config with hold-on-other-key-press enabled
fn tap_hold_config_with_hold_on_other_press() -> TapHoldConfig {
    TapHoldConfig {
        hold_on_other_press: true,
        enable_hrm: false,
        permissive_hold: false,
        chordal_hold: false,
        hold_timeout: embassy_time::Duration::from_millis(200),
        prior_idle_time: embassy_time::Duration::from_millis(120),
        post_wait_time: embassy_time::Duration::from_millis(80),
    }
}

/// Helper to create tap-hold config with both hold-on-other-press and permissive hold
fn tap_hold_config_with_hold_on_other_press_and_permissive() -> TapHoldConfig {
    TapHoldConfig {
        hold_on_other_press: true,
        permissive_hold: true,
        enable_hrm: false,
        chordal_hold: false,
        hold_timeout: embassy_time::Duration::from_millis(200),
        prior_idle_time: embassy_time::Duration::from_millis(120),
        post_wait_time: embassy_time::Duration::from_millis(80),
    }
}

mod hold_on_other_press_tests {
    use super::*;
    use crate::common::{create_test_keyboard_with_config, wrap_keymap, KC_LALT, KC_LCTRL, KC_LGUI, KC_LSHIFT};
    use embassy_futures::block_on;
    use rmk::keyboard::Keyboard;

    rusty_fork_test! {
        #[test]
        fn test_hold_on_other_press_basic() {
            // Basic test: tap-hold key becomes hold when another key is pressed
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hold_on_other_press(),
                    ..BehaviorConfig::default()
                }),
                sequence: [
                    [2, 1, true, 10],   // Press th!(A, LShift)
                    [2, 3, true, 50],   // Press D (regular key) - should trigger hold immediately
                    [2, 3, false, 10],  // Release D
                    [2, 1, false, 10],  // Release A
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],  // Hold triggered immediately
                    [KC_LSHIFT, [kc_to_u8!(D), 0, 0, 0, 0, 0]],  // D pressed with shift
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],  // D released
                    [0, [0, 0, 0, 0, 0, 0]],  // All released
                ]
            };
        }

        #[test]
        fn test_hold_on_other_press_disabled() {
            // When disabled, should wait for timeout
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: TapHoldConfig {
                        hold_on_other_press: false,  // Disabled
                        ..tap_hold_config_with_hold_on_other_press()
                    },
                    ..BehaviorConfig::default()
                }),
                sequence: [
                    [2, 1, true, 10],   // Press th!(A, LShift)
                    [2, 3, true, 50],   // Press D - should NOT trigger hold
                    [2, 1, false, 10],  // Release A - should tap
                    [2, 3, false, 10],  // Release D
                ],
                expected_reports: [
                    [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]],  // Tap A
                    [0, [kc_to_u8!(D), kc_to_u8!(A), 0, 0, 0, 0]],  // A + D
                    [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]],  // Release A
                    [0, [0, 0, 0, 0, 0, 0]],  // All released
                ]
            };
        }

        #[test]
        fn test_hold_on_other_press_with_another_taphold() {
            // Critical test: pressing another tap-hold key should NOT trigger hold
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hold_on_other_press(),
                    ..BehaviorConfig::default()
                }),
                sequence: [
                    [2, 1, true, 10],   // Press th!(A, LShift)
                    [2, 2, true, 30],   // Press th!(S, LGui) - should NOT trigger hold
                    [2, 3, true, 30],   // Press D (regular key) - NOW should trigger hold
                    [2, 1, false, 10],  // Release A
                    [2, 2, false, 10],  // Release S
                    [2, 3, false, 10],  // Release D
                ],
                expected_reports: [
                    // Both tap-hold keys become hold when D is pressed
                    [KC_LSHIFT , [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT | KC_LGUI, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT | KC_LGUI, [kc_to_u8!(D), 0, 0, 0, 0, 0]],
                    [KC_LGUI, [kc_to_u8!(D), 0, 0, 0, 0, 0]],  // Release A
                    [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]],  // Release S
                    [0, [0, 0, 0, 0, 0, 0]],  // All released
                ]
            };
        }

        #[test]
        fn test_hold_on_other_press_tap_hold_combinations() {


            let keymap = wrap_keymap(
                [[[
                    th!(Tab, LCtrl),  // Layer tap-hold
                    th!(CapsLock, LAlt),
                    k!(B),
                    k!(Delete),
                ]], [[
                    k!(No),
                    k!(Kc1),
                    k!(Kc2),
                    k!(Kc3),
                ]]],
                BehaviorConfig {
                    tap_hold: tap_hold_config_with_hold_on_other_press(),
                    ..BehaviorConfig::default()
                }
            );
            // Test that we can still do Ctrl+Alt+Delete style combinations

            key_sequence_test! {
                keyboard: Keyboard::new(keymap),
                sequence: [
                    [0, 0, true, 10],   // Press th!(Tab, LCtrl)
                    [0, 1, true, 10],   // Press th!(CapsLock, LAlt) - should NOT trigger
                    [0, 3, true, 10],   // Press Delete - triggers both holds
                    [0, 3, false, 10],  // Release Delete
                    [0, 0, false, 10],  // Release Tab
                    [0, 1, false, 10],  // Release CapsLock
                ],
                expected_reports: [
                    [KC_LCTRL , [0, 0, 0, 0, 0, 0]],  // Both TAP-HOLD
                    [KC_LCTRL | KC_LALT, [0, 0, 0, 0, 0, 0]],  // Both TAP-HOLD
                    [KC_LCTRL | KC_LALT, [kc_to_u8!(Delete), 0, 0, 0, 0, 0]],  // Ctrl+Alt+Delete
                    [KC_LCTRL | KC_LALT, [0, 0, 0, 0, 0, 0]],  // Release Delete
                    [KC_LALT, [0, 0, 0, 0, 0, 0]],  // Release Ctrl
                    [0, [0, 0, 0, 0, 0, 0]],  // All released
                ]
            };
        }

        #[test]
        fn test_hold_on_other_press_quick_tap() {
            // Quick tap should still work as tap
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hold_on_other_press(),
                    ..BehaviorConfig::default()
                }),
                sequence: [
                    [2, 1, true, 10],   // Press th!(A, LShift)
                    [2, 1, false, 30],  // Release A quickly - should tap
                ],
                expected_reports: [
                    [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],  // Tap A
                    [0, [0, 0, 0, 0, 0, 0]],  // Released
                ]
            };
        }

        #[test]
        fn test_hold_on_other_press_with_permissive_hold() {
            // Both features enabled - hold-on-other-press should take precedence
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hold_on_other_press_and_permissive(),
                    ..BehaviorConfig::default()
                }),
                sequence: [
                    [2, 1, true, 10],   // Press th!(A, LShift)
                    [2, 3, true, 50],   // Press D - should trigger hold immediately
                    [2, 3, false, 10],  // Release D
                    [2, 1, false, 10],  // Release A
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],  // Hold triggered on press
                    [KC_LSHIFT, [kc_to_u8!(D), 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }

        #[test]
        fn test_hold_on_other_press_layer_tap() {
            // Test with layer tap-hold keys
            let keymap = wrap_keymap(
                [[[
                    lt!(1, D),  // Layer tap-hold
                    k!(A),
                    k!(B),
                    k!(C),
                ]], [[
                    k!(No),
                    k!(Kc1),
                    k!(Kc2),
                    k!(Kc3),
                ]]],
                BehaviorConfig {
                    tap_hold: tap_hold_config_with_hold_on_other_press(),
                    ..BehaviorConfig::default()
                }
            );

            key_sequence_test! {
                keyboard: Keyboard::new(keymap),
                sequence: [
                    [0, 0, true, 10],   // Press LT!(1, D)
                    [0, 1, true, 50],   // Press A (which is 1 on layer 1) - should trigger layer
                    [0, 1, false, 10],  // Release A
                    [0, 0, false, 10],  // Release mo!(1)
                ],
                expected_reports: [
                    [0, [kc_to_u8!(Kc1), 0, 0, 0, 0, 0]],  // 1 from layer 1
                    [0, [0, 0, 0, 0, 0, 0]],  // Released
                ]
            };
        }

        #[test]
        fn test_hold_on_other_press_rolling() {
            // Test rolling keys with hold-on-other-press
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hold_on_other_press(),
                    ..BehaviorConfig::default()
                }),
                sequence: [
                    [2, 1, true, 10],   // Press th!(A, LShift)
                    [2, 3, true, 20],   // Press D - triggers hold
                    [2, 4, true, 20],   // Press F
                    [2, 1, false, 10],  // Release A
                    [2, 5, true, 10],   // Press G
                    [2, 3, false, 10],  // Release D
                    [2, 4, false, 10],  // Release F
                    [2, 5, false, 10],  // Release G
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],  // Hold triggered
                    [KC_LSHIFT, [kc_to_u8!(D), 0, 0, 0, 0, 0]],  // D with shift
                    [KC_LSHIFT, [kc_to_u8!(D), kc_to_u8!(F), 0, 0, 0, 0]],  // D+F with shift
                    [0, [kc_to_u8!(D), kc_to_u8!(F), 0, 0, 0, 0]],  // Release shift
                    [0, [kc_to_u8!(D), kc_to_u8!(F), kc_to_u8!(G), 0, 0, 0]],  // Add G
                    [0, [0, kc_to_u8!(F), kc_to_u8!(G), 0, 0, 0]],  // Release D
                    [0, [0, 0, kc_to_u8!(G), 0, 0, 0]],  // Release F
                    [0, [0, 0, 0, 0, 0, 0]],  // All released
                ]
            };
        }

        #[test]
        fn test_hold_on_other_press_multiple_taphold_then_regular() {
            // Multiple tap-hold keys pressed, then a regular key
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hold_on_other_press(),
                    ..BehaviorConfig::default()
                }),
                sequence: [
                    [2, 1, true, 10],   // Press th!(A, LShift)
                    [2, 2, true, 10],   // Press th!(S, LGui)
                    [3, 1, true, 10],   // Press th!(Z, LAlt)
                    [2, 3, true, 30],   // Press D - triggers all holds
                    [2, 1, false, 10],  // Release A
                    [2, 2, false, 10],  // Release S
                    [3, 1, false, 10],  // Release Z
                    [2, 3, false, 10],  // Release D
                ],
                expected_reports: [
                    // All modifiers activated when D is pressed
                    [KC_LSHIFT , [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT | KC_LGUI , [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT | KC_LGUI | KC_LALT, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT | KC_LGUI | KC_LALT, [kc_to_u8!(D), 0, 0, 0, 0, 0]],
                    [KC_LGUI | KC_LALT, [kc_to_u8!(D), 0, 0, 0, 0, 0]],  // Release Shift
                    [KC_LALT, [kc_to_u8!(D), 0, 0, 0, 0, 0]],  // Release Gui
                    [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]],  // Release Alt
                    [0, [0, 0, 0, 0, 0, 0]],  // All released
                ]
            };
        }

        #[test]
        fn test_hold_on_other_press_timeout_still_works() {
            // Verify timeout still works when no other key is pressed
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: tap_hold_config_with_hold_on_other_press(),
                    ..BehaviorConfig::default()
                }),
                sequence: [
                    [2, 1, true, 0],   // Press th!(A, LShift) and hold past timeout
                    [2, 1, false, 250],  // Release A
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],  // Hold triggered by timeout
                    [0, [0, 0, 0, 0, 0, 0]],  // Released
                ]
            };
        }

        #[test]
        fn test_hold_on_other_press_priority_with_hrm_disabled() {
            // When HRM is disabled and permissive hold is enabled,
            // hold-on-other-press should be disabled (permissive hold takes precedence)
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: TapHoldConfig {
                        hold_on_other_press: true,
                        permissive_hold: true,
                        enable_hrm: false,  // HRM disabled
                        chordal_hold: false,
                        hold_timeout: embassy_time::Duration::from_millis(200),
                        prior_idle_time: embassy_time::Duration::from_millis(120),
                        post_wait_time: embassy_time::Duration::from_millis(80),
                    },
                    ..BehaviorConfig::default()
                }),
                sequence: [
                    [2, 1, true, 10],   // Press th!(A, LShift)
                    [2, 3, true, 50],   // Press D - should NOT trigger hold immediately
                    [2, 3, false, 10],  // Release D - should trigger permissive hold
                    [2, 1, false, 10],  // Release A
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],  // LShift became HOLD
                    [KC_LSHIFT, [kc_to_u8!(D), 0, 0, 0, 0, 0]],  // Permissive hold triggered on D release
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],  // D released
                    [0, [0, 0, 0, 0, 0, 0]],  // All released
                ]
            };
        }

        #[test]
        fn test_hold_on_other_press_priority_with_hrm_enabled() {
            // When HRM is enabled, hold-on-other-press should work normally
            key_sequence_test! {
                keyboard: create_test_keyboard_with_config(BehaviorConfig {
                    tap_hold: TapHoldConfig {
                        hold_on_other_press: true,
                        permissive_hold: true,
                        enable_hrm: true,  // HRM enabled
                        chordal_hold: false,
                        hold_timeout: embassy_time::Duration::from_millis(200),
                        prior_idle_time: embassy_time::Duration::from_millis(120),
                        post_wait_time: embassy_time::Duration::from_millis(80),
                    },
                    ..BehaviorConfig::default()
                }),
                sequence: [
                    [2, 1, true, 10],   // Press th!(A, LShift)
                    [2, 3, true, 50],   // Press D - should trigger hold immediately
                    [2, 3, false, 10],  // Release D
                    [2, 1, false, 10],  // Release A
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],  // Hold triggered on press
                    [KC_LSHIFT, [kc_to_u8!(D), 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }

        #[test]
        fn test_hold_on_other_press_simple_tap_hold() {
            // th! key should become hold when another key is pressed
            key_sequence_test! {
                keyboard: Keyboard::new(wrap_keymap(
                    [[[
                        th!(A, LShift), k!(B), k!(C)
                    ]]],
                    BehaviorConfig {
                        tap_hold: tap_hold_config_with_hold_on_other_press(),
                        ..BehaviorConfig::default()
                    }
                )),
                sequence: [
                    [0, 0, true, 10],   // Press th!(A, LShift)
                    [0, 1, true, 20],  // Press B - should trigger hold
                    [0, 1, false, 10], // Release B
                    [0, 0, false, 10], // Release A
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [kc_to_u8!(B), 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }

        #[test]
        fn test_hold_on_other_press_simple_layer_tap() {
            // lt! key should become hold (activate layer) when another key is pressed
            key_sequence_test! {
                keyboard: Keyboard::new(wrap_keymap(
                    [[[
                        lt!(1, D), k!(A), k!(B)
                    ]], [[
                        k!(No), k!(Kc1), k!(Kc2)
                    ]]],
                    BehaviorConfig {
                        tap_hold: tap_hold_config_with_hold_on_other_press(),
                        ..BehaviorConfig::default()
                    }
                )),
                sequence: [
                    [0, 0, true, 10],   // Press LT!(1, D)
                    [0, 1, true, 20],  // Press A - should trigger layer 1
                    [0, 1, false, 10], // Release A
                    [0, 0, false, 10], // Release LT
                ],
                expected_reports: [
                    [0, [kc_to_u8!(Kc1), 0, 0, 0, 0, 0]],  // Layer 1 active, Kc1
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }

        #[test]
        fn test_hold_on_other_press_simple_layer_tap_hrm() {
            // lt! key should become hold (activate layer) when another key is pressed
            key_sequence_test! {
                keyboard: Keyboard::new(wrap_keymap(
                    [[[
                        lt!(1, D), k!(A), k!(B)
                    ]], [[
                        k!(No), k!(Kc1), k!(Kc2)
                    ]]],
                    BehaviorConfig {
                        tap_hold: TapHoldConfig {
                            hold_on_other_press: true,
                            permissive_hold: true,
                            enable_hrm: true,  // HRM enabled
                            chordal_hold: false,
                            hold_timeout: embassy_time::Duration::from_millis(200),
                            prior_idle_time: embassy_time::Duration::from_millis(120),
                            post_wait_time: embassy_time::Duration::from_millis(80),
                        },
                        ..BehaviorConfig::default()
                    }
                )),
                sequence: [
                    [0, 0, true, 10],   // Press LT!(1, D)
                    [0, 1, true, 20],  // Press A - should trigger layer 1
                    [0, 1, false, 10], // Release A
                    [0, 0, false, 10], // Release LT
                ],
                expected_reports: [
                    [0, [kc_to_u8!(Kc1), 0, 0, 0, 0, 0]],  // Layer 1 active, Kc1
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }
        #[test]
        fn test_hold_on_other_press_both_th_and_lt_no_hrm() {
            let keymap = wrap_keymap(
                    [[[
                        th!(A, LShift), lt!(1, D), k!(B)
                    ]], [[
                        k!(No), k!(Kc1), k!(Kc2)
                    ]]],
                    BehaviorConfig {
                        tap_hold: tap_hold_config_with_hold_on_other_press(),
                        ..BehaviorConfig::default()
                    }
                );
            // Both th! and lt! held, then another key triggers both holds

            key_sequence_test! {
                keyboard: Keyboard::new(keymap),
                sequence: [
                    [0, 0, true, 10],   // Press th!(A, LShift)
                    [0, 1, true, 10],   // Press LT!(1, D)
                    [0, 2, true, 20],   // Press B - should trigger both holds
                    [0, 2, false, 10],  // Release B
                    [0, 0, false, 10],  // Release th!
                    [0, 1, false, 10],  // Release LT!
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [kc_to_u8!(Kc2), 0, 0, 0, 0, 0]],  // Both hold: shift + layer 1
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }

        #[test]
        fn test_hold_on_other_press_both_th_and_lt_hrm() {
            let keymap = wrap_keymap(
                    [[[
                        th!(A, LShift), lt!(1, D), k!(B)
                    ]], [[
                        k!(No), k!(Kc1), k!(Kc2)
                    ]]],
                    BehaviorConfig {
                        tap_hold: TapHoldConfig {
                            hold_on_other_press: true,
                            permissive_hold: true,
                            enable_hrm: true,  // HRM enabled
                            chordal_hold: false,
                            hold_timeout: embassy_time::Duration::from_millis(200),
                            prior_idle_time: embassy_time::Duration::from_millis(120),
                            post_wait_time: embassy_time::Duration::from_millis(80),
                        },
                        ..BehaviorConfig::default()
                    }
                );
            // Both th! and lt! held, then another key triggers both holds

            key_sequence_test! {
                keyboard: Keyboard::new(keymap),
                sequence: [
                    [0, 0, true, 10],   // Press th!(A, LShift)
                    [0, 1, true, 10],   // Press LT!(1, D)
                    [0, 2, true, 20],   // Press B - should trigger both holds, since previous key is LT(hold on other key press)
                    [0, 2, false, 10],  // Release B
                    [0, 0, false, 10],  // Release th!
                    [0, 1, false, 10],  // Release LT!
                ],
                expected_reports: [
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [KC_LSHIFT, [kc_to_u8!(Kc2), 0, 0, 0, 0, 0]],  // Both hold: shift + layer 1
                    [KC_LSHIFT, [0, 0, 0, 0, 0, 0]],
                    [0, [0, 0, 0, 0, 0, 0]],
                ]
            };
        }
    }
}
