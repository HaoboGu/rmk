pub mod common;

use embassy_time::Duration;
use rmk::config::{BehaviorConfig, PositionalConfig, StickyKeyConfig};
use rmk::keyboard::Keyboard;
use rmk::types::action::KeyAction;
use rmk::types::modifier::ModifierCombination;
use rmk::{a, k, mo, sk};
use rusty_fork::rusty_fork_test;

use crate::common::{KC_LALT, KC_LCTRL, KC_LSHIFT, wrap_keymap};

// KEYMAP (release_on_layer_change=true is set in the helper config, not per-key)
// Layer 0: A             B             C             MO(1)         LShift        No
// Layer 1: SK(Tab,LAlt)  SK(Tab,LCtrl)  SK(Tab,LCtrl|LShift)  Transparent   Transparent   No

const KEYMAP: [[[KeyAction; 6]; 1]; 2] = [
    [[
        // Layer 0
        k!(A),      // col 0: A
        k!(B),      // col 1: B
        k!(C),      // col 2: C
        mo!(1),     // col 3: MO(1) — momentary layer
        k!(LShift), // col 4: LShift
        a!(No),     // col 5: No
    ]],
    [[
        // Layer 1
        sk!(Tab, ModifierCombination::LALT), // col 0: SK(Tab, LAlt)
        sk!(Tab, ModifierCombination::LCTRL), // col 1: SK(Tab, LCtrl)
        sk!(
            Tab,
            ModifierCombination::new_from_vals(true, true, false, false, false, false, false, false)
        ), // col 2: SK(Tab, LCtrl|LShift)
        a!(Transparent),                                 // col 3: Transparent
        a!(Transparent),                                 // col 4: Transparent → LShift
        a!(No),                                          // col 5: No
    ]],
];

// KEYMAP_MAX_REPEAT: used with the max_repeat=2 helper config (max_repeat is global, not per-key)
const KEYMAP_MAX_REPEAT: [[[KeyAction; 6]; 1]; 2] = [
    [[k!(A), k!(B), k!(C), mo!(1), k!(LShift), a!(No)]],
    [[
        sk!(Tab, ModifierCombination::LALT),  // col 0
        sk!(Tab, ModifierCombination::LCTRL), // col 1
        sk!(Tab, ModifierCombination::LCTRL), // col 2
        a!(Transparent),
        a!(Transparent),
        a!(No),
    ]],
];

// KEYMAP_NO_EXIT: used with the default helper config (release_on_layer_change=false → SK survives MO release)
const KEYMAP_NO_EXIT: [[[KeyAction; 6]; 1]; 2] = [
    [[k!(A), k!(B), k!(C), mo!(1), k!(LShift), a!(No)]],
    [[
        sk!(Tab, ModifierCombination::LALT),  // col 0
        sk!(Tab, ModifierCombination::LCTRL), // col 1
        sk!(Tab, ModifierCombination::LCTRL), // col 2
        a!(Transparent),
        a!(Transparent),
        a!(No),
    ]],
];

fn create_test_keyboard() -> Keyboard<'static> {
    static BEHAVIOR_CONFIG: static_cell::StaticCell<BehaviorConfig> = static_cell::StaticCell::new();
    let behavior_config = BEHAVIOR_CONFIG.init(BehaviorConfig {
        sticky_key: StickyKeyConfig {
            release_on_layer_change: true,
            ..StickyKeyConfig::default()
        },
        ..BehaviorConfig::default()
    });
    static KEY_CONFIG: static_cell::StaticCell<PositionalConfig<1, 6>> = static_cell::StaticCell::new();
    let per_key_config = KEY_CONFIG.init(PositionalConfig::default());
    Keyboard::new(wrap_keymap(KEYMAP, per_key_config, behavior_config))
}

fn create_test_keyboard_max_repeat() -> Keyboard<'static> {
    let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(BehaviorConfig {
        sticky_key: StickyKeyConfig {
            max_repeat: 2,
            ..StickyKeyConfig::default()
        },
        ..BehaviorConfig::default()
    }));
    let per_key_config: &'static PositionalConfig<1, 6> = Box::leak(Box::new(PositionalConfig::default()));
    Keyboard::new(wrap_keymap(KEYMAP_MAX_REPEAT, per_key_config, behavior_config))
}

fn create_test_keyboard_no_exit() -> Keyboard<'static> {
    let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(BehaviorConfig::default()));
    let per_key_config: &'static PositionalConfig<1, 6> = Box::leak(Box::new(PositionalConfig::default()));
    Keyboard::new(wrap_keymap(KEYMAP_NO_EXIT, per_key_config, behavior_config))
}

fn create_test_keyboard_with_behavior_config(config: BehaviorConfig) -> Keyboard<'static> {
    let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(config));
    let per_key_config: &'static PositionalConfig<1, 6> = Box::leak(Box::new(PositionalConfig::default()));
    Keyboard::new(wrap_keymap(KEYMAP, per_key_config, behavior_config))
}

rusty_fork_test! {
    /// StickyKey Test 1: Basic SK flow — press SK twice while MO held
    ///
    /// Sequence:
    /// - Press MO(1) → layer activates, no report
    /// - Press SK(Tab,LAlt) → [KC_LALT, [Tab, ...]]
    /// - Release SK → [KC_LALT, [0, ...]] (modifier held)
    /// - Press SK again → [KC_LALT, [Tab, ...]]
    /// - Release SK → [KC_LALT, [0, ...]]
    /// - Release MO(1) → [0, [0, ...]] (layer deactivation cleans up SK)
    #[test]
    fn test_sk_basic_flow_press_twice() {
        key_sequence_test! {
            keyboard: create_test_keyboard(),
            sequence: [
                [0, 3, true,  10],  // Press MO(1)
                [0, 0, true,  10],  // Press SK(Tab, LAlt)
                [0, 0, false, 10],  // Release SK
                [0, 0, true,  10],  // Press SK again
                [0, 0, false, 10],  // Release SK
                [0, 3, false, 10],  // Release MO(1)
            ],
            expected_reports: [
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SK press: Alt+Tab
                [KC_LALT, [0, 0, 0, 0, 0, 0]],                 // SK release: Alt held
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SK press again: Alt+Tab
                [KC_LALT, [0, 0, 0, 0, 0, 0]],                 // SK release: Alt held
                [0, [0, 0, 0, 0, 0, 0]],                        // MO release: SK cleaned up
            ]
        };
    }

    /// StickyKey Test 2: Layer change cleanup (exit_on_layer_change=true)
    ///
    /// Sequence:
    /// - Press MO(1), press SK(Tab,LAlt), release SK, release MO(1)
    ///
    /// Expected:
    /// - SK press: Alt+Tab
    /// - SK release: Alt held
    /// - MO release: cleans up SK (exit_on_layer_change=true), sends [0, [0,...]]
    #[test]
    fn test_sk_layer_change_cleanup() {
        key_sequence_test! {
            keyboard: create_test_keyboard(),
            sequence: [
                [0, 3, true,  10],  // Press MO(1)
                [0, 0, true,  10],  // Press SK(Tab, LAlt)
                [0, 0, false, 10],  // Release SK
                [0, 3, false, 10],  // Release MO(1) → triggers SK cleanup (exit_on_layer_change=true)
            ],
            expected_reports: [
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SK press: Alt+Tab
                [KC_LALT, [0, 0, 0, 0, 0, 0]],                 // SK release: Alt held
                [0, [0, 0, 0, 0, 0, 0]],                        // MO release: SK cleaned up
            ]
        };
    }

    /// StickyKey Test 3: Shift does NOT release SK
    ///
    /// Sequence:
    /// - Press MO(1), press SK(Tab,LCtrl), release SK
    /// - Press LShift (col 4, transparent → LShift) — should NOT release SK
    /// - Press SK again, release SK
    /// - Release LShift, release MO(1)
    ///
    /// Expected:
    /// - SK press: Ctrl+Tab
    /// - SK release: Ctrl held
    /// - Shift press: Ctrl+Shift held (SK not released)
    /// - SK press: Ctrl+Shift+Tab
    /// - SK release: Ctrl+Shift held
    /// - Shift release: Ctrl held
    /// - MO release: SK cleaned up
    #[test]
    fn test_sk_shift_does_not_release_sk() {
        key_sequence_test! {
            keyboard: create_test_keyboard(),
            sequence: [
                [0, 3, true,  10],  // Press MO(1)
                [0, 1, true,  10],  // Press SK(Tab, LCtrl)
                [0, 1, false, 10],  // Release SK
                [0, 4, true,  10],  // Press LShift (Transparent → LShift on L0)
                [0, 1, true,  10],  // Press SK again
                [0, 1, false, 10],  // Release SK
                [0, 4, false, 10],  // Release LShift
                [0, 3, false, 10],  // Release MO(1)
            ],
            expected_reports: [
                [KC_LCTRL, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],           // SK press: Ctrl+Tab
                [KC_LCTRL, [0, 0, 0, 0, 0, 0]],                          // SK release: Ctrl held
                [KC_LCTRL | KC_LSHIFT, [0, 0, 0, 0, 0, 0]],             // Shift press: Ctrl+Shift (SK not released)
                [KC_LCTRL | KC_LSHIFT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]], // SK press: Ctrl+Shift+Tab
                [KC_LCTRL | KC_LSHIFT, [0, 0, 0, 0, 0, 0]],             // SK release: Ctrl+Shift held
                [KC_LCTRL, [0, 0, 0, 0, 0, 0]],                          // Shift release: Ctrl held
                [0, [0, 0, 0, 0, 0, 0]],                                  // MO release: SK cleaned up
            ]
        };
    }

    /// StickyKey Test 4: Rapid presses — 3x SK press/release while MO held
    ///
    /// Sequence:
    /// - Press MO(1), then 3x (press SK, release SK), release MO(1)
    ///
    /// Expected: Each SK press sends Alt+Tab; each release holds Alt; MO release cleans up.
    #[test]
    fn test_sk_rapid_three_presses() {
        key_sequence_test! {
            keyboard: create_test_keyboard(),
            sequence: [
                [0, 3, true,  10],  // Press MO(1)
                [0, 0, true,  10],  // Press SK #1
                [0, 0, false, 10],  // Release SK #1
                [0, 0, true,  10],  // Press SK #2
                [0, 0, false, 10],  // Release SK #2
                [0, 0, true,  10],  // Press SK #3
                [0, 0, false, 10],  // Release SK #3
                [0, 3, false, 10],  // Release MO(1)
            ],
            expected_reports: [
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SK #1 press
                [KC_LALT, [0, 0, 0, 0, 0, 0]],                 // SK #1 release
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SK #2 press
                [KC_LALT, [0, 0, 0, 0, 0, 0]],                 // SK #2 release
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SK #3 press
                [KC_LALT, [0, 0, 0, 0, 0, 0]],                 // SK #3 release
                [0, [0, 0, 0, 0, 0, 0]],                        // MO release: SK cleaned up
            ]
        };
    }

    /// StickyKey Test 5: Combined modifiers LCtrl|LShift
    ///
    /// Sequence:
    /// - Press MO(1), press SK(Tab,LCtrl|LShift) at col 2, release SK, release MO(1)
    ///
    /// Expected:
    /// - SK press: Ctrl+Shift+Tab
    /// - SK release: Ctrl+Shift held
    /// - MO release: SK cleaned up
    #[test]
    fn test_sk_combined_modifiers() {
        key_sequence_test! {
            keyboard: create_test_keyboard(),
            sequence: [
                [0, 3, true,  10],  // Press MO(1)
                [0, 2, true,  10],  // Press SK(Tab, LCtrl|LShift)
                [0, 2, false, 10],  // Release SK
                [0, 3, false, 10],  // Release MO(1)
            ],
            expected_reports: [
                [KC_LCTRL | KC_LSHIFT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SK press: Ctrl+Shift+Tab
                [KC_LCTRL | KC_LSHIFT, [0, 0, 0, 0, 0, 0]],                 // SK release: Ctrl+Shift held
                [0, [0, 0, 0, 0, 0, 0]],                                      // MO release: SK cleaned up
            ]
        };
    }

    /// StickyKey Test 6: Timeout — modifier auto-releases after inactivity
    ///
    /// Config: global timeout = 100ms
    ///
    /// Sequence:
    /// - Press MO(1), press SK(Tab,LAlt), release SK → timer starts (100ms)
    /// - Wait 150ms → timer fires, Alt auto-released
    /// - Release MO(1) (SK already inactive — no cleanup report)
    /// - Press C on layer 0 (no modifier), release C
    ///
    /// Note: MO(1) must be released before pressing the verification key so that
    /// col 2 resolves to k!(C) on layer 0 rather than SK(Tab,LCtrl|LShift) on layer 1.
    #[test]
    fn test_sk_timeout() {
        key_sequence_test! {
            keyboard: create_test_keyboard_with_behavior_config(BehaviorConfig {
                sticky_key: StickyKeyConfig {
                    timeout: Duration::from_millis(100),
                    release_on_layer_change: true,
                    ..StickyKeyConfig::default()
                },
                ..BehaviorConfig::default()
            }),
            sequence: [
                [0, 3, true,  10],   // Press MO(1)
                [0, 0, true,  10],   // Press SK(Tab, LAlt)
                [0, 0, false, 10],   // Release SK → timer starts (100ms)
                [0, 3, false, 150],  // Wait 150ms (timer fires!), then release MO(1)
                [0, 2, true,  10],   // Press C on layer 0 (no modifier)
                [0, 2, false, 10],   // Release C
            ],
            expected_reports: [
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SK press: Alt+Tab
                [KC_LALT, [0, 0, 0, 0, 0, 0]],                 // SK release: Alt held, timer starts
                [0, [0, 0, 0, 0, 0, 0]],                        // Timeout: Alt auto-released
                // MO(1) release: SK already inactive, no report
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],           // C press: no modifier
                [0, [0, 0, 0, 0, 0, 0]],                        // C release
            ]
        };
    }

    /// StickyKey Test 7: Timeout resets on each SK press
    ///
    /// Config: global timeout = 100ms
    ///
    /// Sequence:
    /// - Press MO(1), press SK #1, release SK #1 → T1 starts (100ms)
    /// - At 50ms: press SK #2 → T1 cancelled, SK #2 processed from unprocessed queue
    /// - Release SK #2 → T2 starts (100ms reset)
    /// - Wait 150ms → T2 fires, Alt auto-released
    /// - Release MO(1) (SK already inactive — no cleanup report)
    /// - Press C on layer 0 (no modifier), release C
    #[test]
    fn test_sk_timeout_resets_on_press() {
        key_sequence_test! {
            keyboard: create_test_keyboard_with_behavior_config(BehaviorConfig {
                sticky_key: StickyKeyConfig {
                    timeout: Duration::from_millis(100),
                    release_on_layer_change: true,
                    ..StickyKeyConfig::default()
                },
                ..BehaviorConfig::default()
            }),
            sequence: [
                [0, 3, true,  10],   // Press MO(1)
                [0, 0, true,  10],   // Press SK #1
                [0, 0, false, 10],   // Release SK #1 → T1 starts (100ms)
                [0, 0, true,  50],   // At 50ms: press SK #2 → T1 cancelled
                [0, 0, false, 10],   // Release SK #2 → T2 starts (100ms reset)
                [0, 3, false, 150],  // Wait 150ms (T2 fires!), then release MO(1)
                [0, 2, true,  10],   // Press C on layer 0 (no modifier)
                [0, 2, false, 10],   // Release C
            ],
            expected_reports: [
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SK #1 press: Alt+Tab
                [KC_LALT, [0, 0, 0, 0, 0, 0]],                 // SK #1 release: Alt held (T1 starts)
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SK #2 press: Alt+Tab (T1 cancelled)
                [KC_LALT, [0, 0, 0, 0, 0, 0]],                 // SK #2 release: Alt held (T2 starts)
                [0, [0, 0, 0, 0, 0, 0]],                        // T2 fires: Alt auto-released
                // MO(1) release: SK already inactive, no report
                [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],           // C press: no modifier
                [0, [0, 0, 0, 0, 0, 0]],                        // C release
            ]
        };
    }

    /// StickyKey Test 8: max_repeat — SK releases after N presses
    ///
    /// Config: KEYMAP_MAX_REPEAT, SK at col 0 has max_repeat=2
    ///
    /// Sequence:
    /// - Press MO(1), press SK ×3, release MO(1)
    ///
    /// Expected:
    /// - Press 1: fire (Alt+Tab, Alt held)
    /// - Press 2: fire (Alt+Tab, Alt held) — this is the max_repeat=2 press
    /// - Press 3: max_repeat reached, SK deactivates silently (no new report beyond empty)
    #[test]
    fn test_sk_max_repeat() {
        key_sequence_test! {
            keyboard: create_test_keyboard_max_repeat(),
            sequence: [
                [0, 3, true,  10],  // Press MO(1)
                [0, 0, true,  10],  // Press SK #1
                [0, 0, false, 10],  // Release SK #1
                [0, 0, true,  10],  // Press SK #2
                [0, 0, false, 10],  // Release SK #2
                [0, 0, true,  10],  // Press SK #3 → max_repeat reached, deactivate
                [0, 0, false, 10],  // Release SK #3
                [0, 3, false, 10],  // Release MO(1)
                [0, 0, true,  10],  // Press A on layer 0 — SK deactivated, no modifier
                [0, 0, false, 10],  // Release A
            ],
            expected_reports: [
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SK #1 press: Alt+Tab
                [KC_LALT, [0, 0, 0, 0, 0, 0]],                 // SK #1 release: Alt held
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SK #2 press: Alt+Tab
                [KC_LALT, [0, 0, 0, 0, 0, 0]],                 // SK #2 release: Alt held
                [0, [0, 0, 0, 0, 0, 0]],                        // SK #3: max_repeat reached, SK deactivated
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],           // A press: no modifier (SK deactivated cleanly)
                [0, [0, 0, 0, 0, 0, 0]],                        // A release
            ]
        };
    }

    // per-key timeout removed this round (deferred, spec Section 4); see parity catalogue

    /// StickyKey Test 10: exit_on_layer_change=true — SK exits on MO release
    ///
    /// This is the same as Test 2 — verifying the explicit exit_on_layer_change=true
    /// setting (the default KEYMAP uses exit=true).
    ///
    /// Sequence: MO↓ SK(exit=true)↓ SK↑ MO↑
    /// Expected: Alt+Tab, Alt, empty.
    #[test]
    fn test_sk_exits_on_layer_change() {
        key_sequence_test! {
            keyboard: create_test_keyboard(),
            sequence: [
                [0, 3, true,  10],  // Press MO(1)
                [0, 0, true,  10],  // Press SK(Tab, LAlt, exit=true)
                [0, 0, false, 10],  // Release SK
                [0, 3, false, 10],  // Release MO(1) → SK exits (exit_on_layer_change=true)
            ],
            expected_reports: [
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SK press: Alt+Tab
                [KC_LALT, [0, 0, 0, 0, 0, 0]],                 // SK release: Alt held
                [0, [0, 0, 0, 0, 0, 0]],                        // MO release: SK exits
            ]
        };
    }

    /// StickyKey Test 11: exit_on_layer_change=false — SK survives layer change
    ///
    /// Config: KEYMAP_NO_EXIT (exit_on_layer_change=false)
    ///
    /// Sequence:
    /// - Press MO(1), press SK(exit=false), release SK
    /// - Release MO(1) — SK does NOT exit (exit_on_layer_change=false)
    /// - Press A on layer 0 — A press releases SK first, then sends A
    /// - Release A
    ///
    /// Expected:
    /// - SK press: Alt+Tab
    /// - SK release: Alt held
    /// - (MO release: no report — SK still active)
    /// - A press: SK released first → [0, [0, ...]], then A registered → [0, [A, ...]]
    /// - A release: [0, [0, ...]]
    #[test]
    fn test_sk_survives_layer_change() {
        key_sequence_test! {
            keyboard: create_test_keyboard_no_exit(),
            sequence: [
                [0, 3, true,  10],  // Press MO(1)
                [0, 0, true,  10],  // Press SK(Tab, LAlt, exit=false)
                [0, 0, false, 10],  // Release SK
                [0, 3, false, 10],  // Release MO(1) — SK does NOT exit
                [0, 0, true,  10],  // Press A on layer 0 — releases SK, sends A
                [0, 0, false, 10],  // Release A
            ],
            expected_reports: [
                [KC_LALT, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],  // SK press: Alt+Tab
                [KC_LALT, [0, 0, 0, 0, 0, 0]],                 // SK release: Alt held (SK still active after MO release)
                [0, [0, 0, 0, 0, 0, 0]],                        // A press: SK release report (Alt released)
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],           // A press: A registered
                [0, [0, 0, 0, 0, 0, 0]],                        // A release
            ]
        };
    }
}
