//! Tests for `LatchTap` action.
//!
//! `LatchTap(modifier, key)` latches `modifier` for the lifetime of the current
//! layer and taps `key` under it on each press, giving Alt/Ctrl/Gui-Tab style
//! window switching. This is the generalization of the old `Tabber` action,
//! extended with an arbitrary `KeyCode` (not just `Tab`) and made to cooperate
//! with other modifiers (held keys and one-shot modifiers).

pub mod common;

use rmk::config::{BehaviorConfig, OneShotModifiersConfig};
use rmk::types::modifier::ModifierCombination;

mod latchtap_test {
    use rmk::config::PositionalConfig;
    use rmk::keyboard::Keyboard;
    use rmk::types::action::KeyAction;
    use rmk::{a, k, latchtap, mo, osm};

    use super::*;
    use crate::common::{KC_LALT, KC_LCTRL, KC_LGUI, KC_LSHIFT, wrap_keymap};

    // KEYMAP
    // Layer 0: A             B              C             MO(1)        LShift       RShift
    // Layer 1: LatchTap(LGui) LatchTap(LCtrl) LatchTap(LAlt) OSM(LShift) Transparent  Transparent

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
            latchtap!(ModifierCombination::LGUI, Tab),
            latchtap!(ModifierCombination::LCTRL, Tab),
            latchtap!(ModifierCombination::LALT, Tab),
            osm!(ModifierCombination::new_from(false, false, false, true, false)), // OSM(LShift)
            a!(Transparent),
            a!(Transparent),
        ]],
    ];

    fn create_test_keyboard() -> Keyboard<'static> {
        let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(BehaviorConfig::default()));
        let per_key_config: &'static PositionalConfig<1, 6> = Box::leak(Box::new(PositionalConfig::default()));
        Keyboard::new(wrap_keymap(KEYMAP, per_key_config, behavior_config))
    }

    fn create_test_keyboard_with_one_shot_modifiers_config(config: OneShotModifiersConfig) -> Keyboard<'static> {
        let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(BehaviorConfig {
            one_shot_modifiers: config,
            ..BehaviorConfig::default()
        }));
        let per_key_config: &'static PositionalConfig<1, 6> = Box::leak(Box::new(PositionalConfig::default()));
        Keyboard::new(wrap_keymap(KEYMAP, per_key_config, behavior_config))
    }

    /// LatchTap Test Case 1: Basic Flow
    ///
    /// Sequence:
    /// - Press MO(1) to activate layer 1
    /// - Press LatchTap(LGui) → Should send LGui+Tab
    /// - Release LatchTap → Should release Tab, keep LGui held
    /// - Press LatchTap again → Should send Tab only
    /// - Release LatchTap → Should release Tab only
    /// - Release MO(1) → Should release LGui
    ///
    /// Expected:
    /// - LGui+Tab on first press
    /// - Only LGui held after first release
    /// - LGui+Tab on second press
    /// - Only LGui held after second release
    /// - All released after MO(1) release
    #[test]
    fn test_latchtap_basic_flow() {
        key_sequence_test! {
            keyboard: create_test_keyboard(),
            sequence: [
                [0, 3, true, 10],   // Press MO(1)
                [0, 0, true, 10],   // Press LatchTap(LGui)
                [0, 0, false, 10],  // Release LatchTap
                [0, 0, true, 10],   // Press LatchTap again
                [0, 0, false, 10],  // Release LatchTap again
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

    /// LatchTap Test Case 2: Shift Integration
    ///
    /// Sequence:
    /// - Press MO(1) to activate layer 1
    /// - Press LatchTap(LCtrl) → Should send LCtrl+Tab
    /// - Release LatchTap → Should release Tab, keep LCtrl held
    /// - Press LShift
    /// - Press LatchTap → Should send LCtrl+LShift+Tab
    /// - Release LatchTap → Should release Tab, keep LCtrl held
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
    fn test_latchtap_with_shift() {
        key_sequence_test! {
            keyboard: create_test_keyboard(),
            sequence: [
                [0, 3, true, 10],   // Press MO(1)
                [0, 1, true, 10],   // Press LatchTap(LCtrl)
                [0, 1, false, 10],  // Release LatchTap(LCtrl)
                [0, 4, true, 10],   // Press LShift
                [0, 1, true, 10],   // Press LatchTap(LCtrl)
                [0, 1, false, 10],  // Release LatchTap(LCtrl)
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

    /// LatchTap Test Case 3: Rapid Presses
    ///
    /// Sequence:
    /// - Press MO(1) to activate layer 1
    /// - Rapidly press and release LatchTap 3 times
    /// - Release MO(1)
    ///
    /// Expected:
    /// - LGui+Tab on each press
    /// - Only LGui held after each release
    /// - All released after MO(1) release
    #[test]
    fn test_latchtap_rapid_presses() {
        key_sequence_test! {
            keyboard: create_test_keyboard(),
            sequence: [
                [0, 3, true, 10],   // Press MO(1)
                [0, 0, true, 10],   // Press LatchTap
                [0, 0, false, 10],  // Release LatchTap
                [0, 0, true, 10],   // Press LatchTap
                [0, 0, false, 10],  // Release LatchTap
                [0, 0, true, 10],   // Press LatchTap
                [0, 0, false, 10],  // Release LatchTap
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

    /// LatchTap Test Case 4: Different Modifiers
    ///
    /// Sequence:
    /// - Press MO(1) to activate layer 1
    /// - Press LatchTap(LAlt)
    /// - Release LatchTap
    /// - Release MO(1)
    ///
    /// Expected:
    /// - LAlt+Tab on press
    /// - Only LAlt held after release
    /// - All released after MO(1) release
    #[test]
    fn test_latchtap_with_alt() {
        key_sequence_test! {
            keyboard: create_test_keyboard(),
            sequence: [
                [0, 3, true, 10],   // Press MO(1)
                [0, 2, true, 10],   // Press LatchTap(LAlt)
                [0, 2, false, 10],  // Release LatchTap
                [0, 3, false, 10],  // Release MO(1)
            ],
            expected_reports: [
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]], // LAlt+Tab
                [KC_LALT, [0, 0, 0, 0, 0, 0]],              // Only LAlt held
                [0, [0, 0, 0, 0, 0, 0]],                    // All released
            ]
        };
    }

    /// LatchTap Test Case 5: Layer Change Cleanup
    ///
    /// Sequence:
    /// - Press MO(1) to activate layer 1
    /// - Press LatchTap(LGui)
    /// - Release LatchTap
    /// - Release MO(1) immediately (should clean up)
    ///
    /// Expected:
    /// - LGui+Tab on press
    /// - Only LGui held after release
    /// - All released after MO(1) release (cleanup)
    #[test]
    fn test_latchtap_layer_change_cleanup() {
        key_sequence_test! {
            keyboard: create_test_keyboard(),
            sequence: [
                [0, 3, true, 10],   // Press MO(1)
                [0, 0, true, 10],   // Press LatchTap(LGui)
                [0, 0, false, 10],  // Release LatchTap
                [0, 3, false, 10],  // Release MO(1) - should clean up LatchTap
            ],
            expected_reports: [
                [KC_LGUI, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]], // LGui+Tab
                [KC_LGUI, [0, 0, 0, 0, 0, 0]],              // Only LGui held
                [0, [0, 0, 0, 0, 0, 0]],                    // All released (cleanup)
            ]
        };
    }

    /// LatchTap Test Case 6: One-Shot Modifier Cooperation
    ///
    /// With `one_shot_modifiers.activate_on_keypress = true`, an OSM modifier
    /// engaged while a LatchTap is latched must persist across LatchTap taps
    /// and combine into the same report (LCtrl + LShift + Tab).
    ///
    /// Sequence:
    /// - Press MO(1) to activate layer 1
    /// - Press LatchTap(LCtrl) → LCtrl+Tab
    /// - Release LatchTap → LCtrl held
    /// - Press OSM(LShift) → LCtrl+LShift (activate_on_keypress)
    /// - Press LatchTap(LCtrl) → LCtrl+LShift+Tab
    /// - Release LatchTap → LCtrl+LShift held
    /// - Release OSM(LShift) → LCtrl held (OSM released)
    /// - Release MO(1) → All released (cleanup)
    #[test]
    fn test_latchtap_with_osm() {
        key_sequence_test! {
            keyboard: create_test_keyboard_with_one_shot_modifiers_config(OneShotModifiersConfig {
                activate_on_keypress: true,
                ..OneShotModifiersConfig::default()
            }),
            sequence: [
                [0, 3, true, 10],   // Press MO(1)
                [0, 1, true, 10],   // Press LatchTap(LCtrl)
                [0, 1, false, 10],  // Release LatchTap
                [0, 3, true, 10],   // Press OSM(LShift) on layer 1
                [0, 1, true, 10],   // Press LatchTap(LCtrl)
                [0, 1, false, 10],  // Release LatchTap
                [0, 3, false, 10],  // Release OSM(LShift)
                [0, 3, false, 10],  // Release MO(1)
            ],
            expected_reports: [
                [KC_LCTRL, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],                  // LCtrl+Tab
                [KC_LCTRL, [0, 0, 0, 0, 0, 0]],                              // Only LCtrl held
                [KC_LCTRL | KC_LSHIFT, [0, 0, 0, 0, 0, 0]],                  // LCtrl+LShift (OSM on)
                [KC_LCTRL | KC_LSHIFT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],      // LCtrl+LShift+Tab
                [KC_LCTRL | KC_LSHIFT, [0, 0, 0, 0, 0, 0]],                  // LCtrl+LShift held
                [KC_LCTRL, [0, 0, 0, 0, 0, 0]],                              // Only LCtrl held (OSM off)
                [0, [0, 0, 0, 0, 0, 0]],                                      // All released (cleanup)
            ]
        };
    }
}
