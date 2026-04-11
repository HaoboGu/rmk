pub mod common;

use rmk::config::{BehaviorConfig, PositionalConfig};
use rmk::keyboard::Keyboard;
use rmk::types::action::KeyAction;
use rmk::types::modifier::ModifierCombination;
use rmk::{a, k, mo, sm};
use rusty_fork::rusty_fork_test;

use crate::common::{KC_LALT, KC_LCTRL, KC_LSHIFT, wrap_keymap};

// KEYMAP
// Layer 0: A             B             C             MO(1)         LShift        No
// Layer 1: SM(Tab,LAlt)  SM(Tab,LCtrl) SM(Tab,LCtrl|LShift)  Transparent   Transparent   No

const KEYMAP: [[[KeyAction; 6]; 1]; 2] = [
    [[
        // Layer 0
        k!(A),    // col 0: A
        k!(B),    // col 1: B
        k!(C),    // col 2: C
        mo!(1),   // col 3: MO(1) — momentary layer
        k!(LShift), // col 4: LShift
        a!(No),   // col 5: No
    ]],
    [[
        // Layer 1
        sm!(Tab, ModifierCombination::LALT),                              // col 0: SM(Tab, LAlt)
        sm!(Tab, ModifierCombination::LCTRL),                             // col 1: SM(Tab, LCtrl)
        sm!(Tab, ModifierCombination::new_from_vals(true, true, false, false, false, false, false, false)), // col 2: SM(Tab, LCtrl|LShift)
        a!(Transparent),                                                    // col 3: Transparent
        a!(Transparent),                                                    // col 4: Transparent → LShift
        a!(No),                                                             // col 5: No
    ]],
];

fn create_test_keyboard() -> Keyboard<'static> {
    static BEHAVIOR_CONFIG: static_cell::StaticCell<BehaviorConfig> = static_cell::StaticCell::new();
    let behavior_config = BEHAVIOR_CONFIG.init(BehaviorConfig::default());
    static KEY_CONFIG: static_cell::StaticCell<PositionalConfig<1, 6>> = static_cell::StaticCell::new();
    let per_key_config = KEY_CONFIG.init(PositionalConfig::default());
    Keyboard::new(wrap_keymap(KEYMAP, per_key_config, behavior_config))
}

rusty_fork_test! {
    /// StickyMod Test 1: Basic SM flow — press SM twice while MO held
    ///
    /// Sequence:
    /// - Press MO(1) → layer activates, no report
    /// - Press SM(Tab,LAlt) → [KC_LALT, [Tab, ...]]
    /// - Release SM → [KC_LALT, [0, ...]] (modifier held)
    /// - Press SM again → [KC_LALT, [Tab, ...]]
    /// - Release SM → [KC_LALT, [0, ...]]
    /// - Release MO(1) → [0, [0, ...]] (layer deactivation cleans up SM)
    #[test]
    fn test_sm_basic_flow_press_twice() {
        key_sequence_test! {
            keyboard: create_test_keyboard(),
            sequence: [
                [0, 3, true,  10],  // Press MO(1)
                [0, 0, true,  10],  // Press SM(Tab, LAlt)
                [0, 0, false, 10],  // Release SM
                [0, 0, true,  10],  // Press SM again
                [0, 0, false, 10],  // Release SM
                [0, 3, false, 10],  // Release MO(1)
            ],
            expected_reports: [
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SM press: Alt+Tab
                [KC_LALT, [0, 0, 0, 0, 0, 0]],                 // SM release: Alt held
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SM press again: Alt+Tab
                [KC_LALT, [0, 0, 0, 0, 0, 0]],                 // SM release: Alt held
                [0, [0, 0, 0, 0, 0, 0]],                        // MO release: SM cleaned up
            ]
        };
    }

    /// StickyMod Test 2: Layer change cleanup
    ///
    /// Sequence:
    /// - Press MO(1), press SM(Tab,LAlt), release SM, release MO(1)
    ///
    /// Expected:
    /// - SM press: Alt+Tab
    /// - SM release: Alt held
    /// - MO release: cleans up SM, sends [0, [0,...]]
    #[test]
    fn test_sm_layer_change_cleanup() {
        key_sequence_test! {
            keyboard: create_test_keyboard(),
            sequence: [
                [0, 3, true,  10],  // Press MO(1)
                [0, 0, true,  10],  // Press SM(Tab, LAlt)
                [0, 0, false, 10],  // Release SM
                [0, 3, false, 10],  // Release MO(1) → triggers SM cleanup
            ],
            expected_reports: [
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SM press: Alt+Tab
                [KC_LALT, [0, 0, 0, 0, 0, 0]],                 // SM release: Alt held
                [0, [0, 0, 0, 0, 0, 0]],                        // MO release: SM cleaned up
            ]
        };
    }

    /// StickyMod Test 3: Shift integration — Shift does NOT release SM
    ///
    /// Sequence:
    /// - Press MO(1), press SM(Tab,LCtrl), release SM
    /// - Press LShift (col 4, transparent → LShift) — should NOT release SM
    /// - Press SM again, release SM
    /// - Release LShift, release MO(1)
    ///
    /// Expected:
    /// - SM press: Ctrl+Tab
    /// - SM release: Ctrl held
    /// - Shift press: Ctrl+Shift held (SM not released)
    /// - SM press: Ctrl+Shift+Tab
    /// - SM release: Ctrl+Shift held
    /// - Shift release: Ctrl held
    /// - MO release: SM cleaned up
    #[test]
    fn test_sm_shift_does_not_release_sm() {
        key_sequence_test! {
            keyboard: create_test_keyboard(),
            sequence: [
                [0, 3, true,  10],  // Press MO(1)
                [0, 1, true,  10],  // Press SM(Tab, LCtrl)
                [0, 1, false, 10],  // Release SM
                [0, 4, true,  10],  // Press LShift (Transparent → LShift on L0)
                [0, 1, true,  10],  // Press SM again
                [0, 1, false, 10],  // Release SM
                [0, 4, false, 10],  // Release LShift
                [0, 3, false, 10],  // Release MO(1)
            ],
            expected_reports: [
                [KC_LCTRL, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],           // SM press: Ctrl+Tab
                [KC_LCTRL, [0, 0, 0, 0, 0, 0]],                          // SM release: Ctrl held
                [KC_LCTRL | KC_LSHIFT, [0, 0, 0, 0, 0, 0]],             // Shift press: Ctrl+Shift (SM not released)
                [KC_LCTRL | KC_LSHIFT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]], // SM press: Ctrl+Shift+Tab
                [KC_LCTRL | KC_LSHIFT, [0, 0, 0, 0, 0, 0]],             // SM release: Ctrl+Shift held
                [KC_LCTRL, [0, 0, 0, 0, 0, 0]],                          // Shift release: Ctrl held
                [0, [0, 0, 0, 0, 0, 0]],                                  // MO release: SM cleaned up
            ]
        };
    }

    /// StickyMod Test 4: Rapid presses — 3x SM press/release while MO held
    ///
    /// Sequence:
    /// - Press MO(1), then 3x (press SM, release SM), release MO(1)
    ///
    /// Expected: Each SM press sends Alt+Tab; each release holds Alt; MO release cleans up.
    #[test]
    fn test_sm_rapid_three_presses() {
        key_sequence_test! {
            keyboard: create_test_keyboard(),
            sequence: [
                [0, 3, true,  10],  // Press MO(1)
                [0, 0, true,  10],  // Press SM #1
                [0, 0, false, 10],  // Release SM #1
                [0, 0, true,  10],  // Press SM #2
                [0, 0, false, 10],  // Release SM #2
                [0, 0, true,  10],  // Press SM #3
                [0, 0, false, 10],  // Release SM #3
                [0, 3, false, 10],  // Release MO(1)
            ],
            expected_reports: [
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SM #1 press
                [KC_LALT, [0, 0, 0, 0, 0, 0]],                 // SM #1 release
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SM #2 press
                [KC_LALT, [0, 0, 0, 0, 0, 0]],                 // SM #2 release
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SM #3 press
                [KC_LALT, [0, 0, 0, 0, 0, 0]],                 // SM #3 release
                [0, [0, 0, 0, 0, 0, 0]],                        // MO release: SM cleaned up
            ]
        };
    }

    /// StickyMod Test 5: Combined modifiers LCtrl|LShift
    ///
    /// Sequence:
    /// - Press MO(1), press SM(Tab,LCtrl|LShift) at col 2, release SM, release MO(1)
    ///
    /// Expected:
    /// - SM press: Ctrl+Shift+Tab
    /// - SM release: Ctrl+Shift held
    /// - MO release: SM cleaned up
    #[test]
    fn test_sm_combined_modifiers() {
        key_sequence_test! {
            keyboard: create_test_keyboard(),
            sequence: [
                [0, 3, true,  10],  // Press MO(1)
                [0, 2, true,  10],  // Press SM(Tab, LCtrl|LShift)
                [0, 2, false, 10],  // Release SM
                [0, 3, false, 10],  // Release MO(1)
            ],
            expected_reports: [
                [KC_LCTRL | KC_LSHIFT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SM press: Ctrl+Shift+Tab
                [KC_LCTRL | KC_LSHIFT, [0, 0, 0, 0, 0, 0]],                 // SM release: Ctrl+Shift held
                [0, [0, 0, 0, 0, 0, 0]],                                      // MO release: SM cleaned up
            ]
        };
    }
}
