//! Tests for Tabber action
//!
//! Tabber provides Alt+Tab-like window switching behavior

pub mod common;

use rmk::config::BehaviorConfig;
use rmk::types::modifier::ModifierCombination;

mod tabber_test {
    use std::cell::RefCell;

    use rmk::config::PositionalConfig;
    use rmk::keyboard::Keyboard;
    use rmk::keymap::KeyMap;
    use rmk::types::action::{Action, KeyAction};
    use rmk::{a, k, mo};
    use rusty_fork::rusty_fork_test;

    use super::*;
    use crate::common::{KC_LALT, KC_LCTRL, KC_LGUI, KC_LSHIFT, wrap_keymap};

    // KEYMAP
    // Layer 0: A             B              C             MO(1)        LShift       RShift
    // Layer 1: Tabber(LGui)  Tabber(LCtrl)  Tabber(LAlt)  Transparent  Transparent  Transparent

    const KEYMAP: [[[KeyAction; 6]; 1]; 2] = [
        [[
            // Layer 0
            k!(A),
            k!(B),
            k!(C),
            mo!(1), // MO(1) to activate layer 1
            k!(LShift),
            k!(RShift),
        ]],
        [[
            // Layer 1
            KeyAction::Single(Action::Tabber(ModifierCombination::LGUI)),
            KeyAction::Single(Action::Tabber(ModifierCombination::LCTRL)),
            KeyAction::Single(Action::Tabber(ModifierCombination::LALT)),
            a!(Transparent), // MO(1) is transparent
            a!(Transparent), // LShift is transparent
            a!(Transparent), // RShift is transparent
        ]],
    ];

    fn create_test_keyboard() -> Keyboard<'static, 1, 6, 2> {
        static BEHAVIOR_CONFIG: static_cell::StaticCell<BehaviorConfig> = static_cell::StaticCell::new();
        let behavior_config = BEHAVIOR_CONFIG.init(BehaviorConfig::default());
        static KEY_CONFIG: static_cell::StaticCell<PositionalConfig<1, 6>> = static_cell::StaticCell::new();
        let per_key_config = KEY_CONFIG.init(PositionalConfig::default());
        let keymap: &RefCell<KeyMap<1, 6, 2>> = wrap_keymap(KEYMAP, per_key_config, behavior_config);
        Keyboard::new(keymap)
    }

    rusty_fork_test! {
        /// Tabber Test Case 1: Basic Flow
        ///
        /// Sequence:
        /// - Press MO(1) to activate layer 1
        /// - Press Tabber(LGui) → Should send LGui+Tab
        /// - Release Tabber → Should release Tab, keep LGui held
        /// - Press Tabber again → Should send Tab only
        /// - Release Tabber → Should release Tab only
        /// - Release MO(1) → Should release LGui
        ///
        /// Expected:
        /// - LGui+Tab on first press
        /// - Only LGui held after first release
        /// - LGui+Tab on second press
        /// - Only LGui held after second release
        /// - All released after MO(1) release
        #[test]
        fn test_tabber_basic_flow() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [0, 3, true, 10],   // Press MO(1)
                    [0, 0, true, 10],   // Press Tabber(LGui)
                    [0, 0, false, 10],  // Release Tabber
                    [0, 0, true, 10],   // Press Tabber again
                    [0, 0, false, 10],  // Release Tabber again
                    [0, 3, false, 10],  // Release MO(1)
                ],
                expected_reports: [
                    [KC_LGUI, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]], // LGui+Tab on first press
                    [KC_LGUI, [0, 0, 0, 0, 0, 0]],              // Only LGui held after first release
                    [KC_LGUI, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]], // LGui+Tab on second press
                    [KC_LGUI, [0, 0, 0, 0, 0, 0]],              // Only LGui held after second release
                    [0, [0, 0, 0, 0, 0, 0]],                    // All released after MO(1) release
                ]
            };
        }

        /// Tabber Test Case 2: Shift Integration
        ///
        /// Sequence:
        /// - Press MO(1) to activate layer 1
        /// - Press Tabber(LCtrl) → Should send LCtrl+Tab
        /// - Release Tabber → Should release Tab, keep LCtrl held
        /// - Press LShift
        /// - Press Tabber → Should send LCtrl+LShift+Tab
        /// - Release Tabber → Should release Tab, keep LCtrl held
        /// - Release LShift
        /// - Release MO(1) → Should release LCtrl
        ///
        /// Expected:
        /// - LCtrl+Tab on first press
        /// - Only LCtrl held after first release
        /// - LShift is added
        /// - LCtrl+LShift+Tab on second press
        /// - LCtrl+LShift held after second release
        /// - Only LCtrl held after LShift release
        /// - All released after MO(1) release
        #[test]
        fn test_tabber_with_shift() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [0, 3, true, 10],   // Press MO(1)
                    [0, 1, true, 10],   // Press Tabber(LCtrl)
                    [0, 1, false, 10],  // Release Tabber(LCtrl)
                    [0, 4, true, 10],   // Press LShift
                    [0, 1, true, 10],   // Press Tabber(LCtrl)
                    [0, 1, false, 10],  // Release Tabber(LCtrl)
                    [0, 4, false, 10],  // Release LShift
                    [0, 3, false, 10],  // Release MO(1)
                ],
                expected_reports: [
                    [KC_LCTRL, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],              // LCtrl+Tab on first press
                    [KC_LCTRL, [0, 0, 0, 0, 0, 0]],                           // Only LCtrl held
                    [KC_LCTRL | KC_LSHIFT, [0, 0, 0, 0, 0, 0]],               // LShift pressed
                    [KC_LCTRL | KC_LSHIFT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // LCtrl+LShift+Tab
                    [KC_LCTRL | KC_LSHIFT, [0, 0, 0, 0, 0, 0]],               // LCtrl+LShift held
                    [KC_LCTRL, [0, 0, 0, 0, 0, 0]],                           // Only LCtrl held
                    [0, [0, 0, 0, 0, 0, 0]],                                  // All released
                ]
            };
        }

        /// Tabber Test Case 3: Rapid Presses
        ///
        /// Sequence:
        /// - Press MO(1) to activate layer 1
        /// - Rapidly press and release Tabber 3 times
        /// - Release MO(1)
        ///
        /// Expected:
        /// - LGui+Tab on each press
        /// - Only LGui held after each release
        /// - All released after MO(1) release
        #[test]
        fn test_tabber_rapid_presses() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [0, 3, true, 10],   // Press MO(1)
                    [0, 0, true, 10],   // Press Tabber
                    [0, 0, false, 10],  // Release Tabber
                    [0, 0, true, 10],   // Press Tabber
                    [0, 0, false, 10],  // Release Tabber
                    [0, 0, true, 10],   // Press Tabber
                    [0, 0, false, 10],  // Release Tabber
                    [0, 3, false, 10],  // Release MO(1)
                ],
                expected_reports: [
                    [KC_LGUI, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]], // LGui+Tab
                    [KC_LGUI, [0, 0, 0, 0, 0, 0]],              // Only LGui held
                    [KC_LGUI, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]], // LGui+Tab
                    [KC_LGUI, [0, 0, 0, 0, 0, 0]],              // Only LGui held
                    [KC_LGUI, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]], // LGui+Tab
                    [KC_LGUI, [0, 0, 0, 0, 0, 0]],              // Only LGui held
                    [0, [0, 0, 0, 0, 0, 0]],                    // All released
                ]
            };
        }

        /// Tabber Test Case 4: Different Modifiers
        ///
        /// Sequence:
        /// - Press MO(1) to activate layer 1
        /// - Press Tabber(LAlt)
        /// - Release Tabber
        /// - Release MO(1)
        ///
        /// Expected:
        /// - LAlt+Tab on press
        /// - Only LAlt held after release
        /// - All released after MO(1) release
        #[test]
        fn test_tabber_with_alt() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [0, 3, true, 10],   // Press MO(1)
                    [0, 2, true, 10],   // Press Tabber(LAlt)
                    [0, 2, false, 10],  // Release Tabber
                    [0, 3, false, 10],  // Release MO(1)
                ],
                expected_reports: [
                    [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]], // LAlt+Tab
                    [KC_LALT, [0, 0, 0, 0, 0, 0]],              // Only LAlt held
                    [0, [0, 0, 0, 0, 0, 0]],                    // All released
                ]
            };
        }

        /// Tabber Test Case 5: Layer Change Cleanup
        ///
        /// Sequence:
        /// - Press MO(1) to activate layer 1
        /// - Press Tabber(LGui)
        /// - Release Tabber
        /// - Release MO(1) immediately (should clean up)
        ///
        /// Expected:
        /// - LGui+Tab on press
        /// - Only LGui held after release
        /// - All released after MO(1) release (cleanup)
        #[test]
        fn test_tabber_layer_change_cleanup() {
            key_sequence_test! {
                keyboard: create_test_keyboard(),
                sequence: [
                    [0, 3, true, 10],   // Press MO(1)
                    [0, 0, true, 10],   // Press Tabber(LGui)
                    [0, 0, false, 10],  // Release Tabber
                    [0, 3, false, 10],  // Release MO(1) - should clean up Tabber
                ],
                expected_reports: [
                    [KC_LGUI, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]], // LGui+Tab
                    [KC_LGUI, [0, 0, 0, 0, 0, 0]],              // Only LGui held
                    [0, [0, 0, 0, 0, 0, 0]],                    // All released (cleanup)
                ]
            };
        }
    }
}
