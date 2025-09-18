/// Test cases for AutoShift feature
///
/// AutoShift automatically applies shift to keys when they are held down for a specified duration.
/// This eliminates the need to manually hold the shift key for capitals and symbols.
pub mod common;

use rmk::config::{AutoShiftConfig, AutoShiftKeySet, BehaviorConfig, PerKeyConfig};
use rmk::keyboard::Keyboard;
use rmk::types::action::Action;
use rmk::types::keycode::KeyCode;
use rmk::{k, mt};
use rmk_types::modifier::ModifierCombination;
use rusty_fork::rusty_fork_test;

use crate::common::KC_LSHIFT;
use crate::common::wrap_keymap;

/// Create AutoShift keyboard with proper keymap for testing
fn create_autoshift_test_keyboard(behavior_config: BehaviorConfig) -> Keyboard<'static, 1, 5, 2> {
    let keymap = [
        [[
            k!(A),                             // Position (0,0): Letter A
            k!(Kc1),                           // Position (0,1): Number 1
            k!(Semicolon),                     // Position (0,2): Symbol ;
            mt!(D, ModifierCombination::LGUI), // Position (0,3): D with GUI (regular tap-hold)
            k!(E),                             // Position (0,4): Letter E
        ]],
        [[k!(Kp1), k!(Kp2), k!(Kp3), k!(Kp4), k!(Kp5)]],
    ];

    use static_cell::StaticCell;
    static BEHAVIOR_CONFIG: StaticCell<BehaviorConfig> = StaticCell::new();
    let behavior_config = BEHAVIOR_CONFIG.init(behavior_config);
    static KEY_CONFIG: static_cell::StaticCell<PerKeyConfig<1, 5>> = static_cell::StaticCell::new();
    let per_key_config = KEY_CONFIG.init(PerKeyConfig::default());
    Keyboard::new(wrap_keymap(keymap, per_key_config, behavior_config))
}

fn create_autoshift_keyboard() -> Keyboard<'static, 1, 5, 2> {
    create_autoshift_test_keyboard(BehaviorConfig {
        autoshift: AutoShiftConfig {
            enable: true,
            enabled_keys: AutoShiftKeySet {
                letters: true,
                numbers: true,
                symbols: true,
            },
        },
        ..BehaviorConfig::default()
    })
}

fn create_autoshift_keyboard_disabled() -> Keyboard<'static, 1, 5, 2> {
    create_autoshift_test_keyboard(BehaviorConfig {
        autoshift: AutoShiftConfig {
            enable: false,
            enabled_keys: AutoShiftKeySet::default(),
        },
        ..BehaviorConfig::default()
    })
}

rusty_fork_test! {
    #[test]
    fn test_autoshift_key_classification() {
        // Test letters
        assert!(KeyCode::A.supports_autoshift(true, true, true));
        assert!(KeyCode::Z.supports_autoshift(true, true, true));

        // Test numbers
        assert!(KeyCode::Kc1.supports_autoshift(true, true, true));
        assert!(KeyCode::Kc0.supports_autoshift(true, true, true));

        // Test symbols
        assert!(KeyCode::Semicolon.supports_autoshift(true, true, true));
        assert!(KeyCode::Quote.supports_autoshift(true, true, true));
        assert!(KeyCode::Comma.supports_autoshift(true, true, true));
        assert!(KeyCode::Dot.supports_autoshift(true, true, true));
        assert!(KeyCode::Slash.supports_autoshift(true, true, true));

        // Test non-supported keys
        assert!(!KeyCode::F1.supports_autoshift(true, true, true));
        assert!(!KeyCode::Enter.supports_autoshift(true, true, true));
        assert!(!KeyCode::Space.supports_autoshift(true, true, true));
    }

    #[test]
    fn test_autoshift_shifted_actions() {
        // Test letter shifting
        if let Some(Action::KeyWithModifier(key, modifier)) = KeyCode::A.get_shifted_action() {
            assert_eq!(key, KeyCode::A);
            assert_eq!(modifier, ModifierCombination::LSHIFT);
        } else {
            panic!("Expected shifted action for letter A");
        }

        // Test number shifting
        if let Some(Action::KeyWithModifier(key, modifier)) = KeyCode::Kc1.get_shifted_action() {
            assert_eq!(key, KeyCode::Kc1);
            assert_eq!(modifier, ModifierCombination::LSHIFT);
        } else {
            panic!("Expected shifted action for number 1");
        }

        // Test symbol shifting
        if let Some(Action::KeyWithModifier(key, modifier)) = KeyCode::Semicolon.get_shifted_action() {
            assert_eq!(key, KeyCode::Semicolon);
            assert_eq!(modifier, ModifierCombination::LSHIFT);
        } else {
            panic!("Expected shifted action for semicolon");
        }
    }

    #[test]
    fn test_autoshift_tap_behavior_letter() {
        // Test that quick tap generates normal (lowercase) letter
        key_sequence_test! {
            keyboard: create_autoshift_keyboard(),
            sequence: [
                [0, 0, true, 50],    // Press 'A' briefly
                [0, 0, false, 50],   // Release 'A'
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Normal A
                [0, [0, 0, 0, 0, 0, 0]], // Release
            ]
        };
    }

    #[test]
    fn test_autoshift_hold_behavior_letter() {
        // Test that holding generates shifted (uppercase) letter
        key_sequence_test! {
            keyboard: create_autoshift_keyboard(),
            sequence: [
                [0, 0, true, 10],   // Press 'A'
                [0, 0, false, 270], // Hold past timeout
            ],
            expected_reports: [
                [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Shift+A (uppercase)
                [0, [0, 0, 0, 0, 0, 0]], // Release all
            ]
        };
    }

    #[test]
    fn test_autoshift_tap_behavior_number() {
        // Test that quick tap generates normal number
        key_sequence_test! {
            keyboard: create_autoshift_keyboard(),
            sequence: [
                [0, 1, true, 50],    // Press '1' briefly
                [0, 1, false, 50],   // Release '1'
            ],
            expected_reports: [
                [0, [kc_to_u8!(Kc1), 0, 0, 0, 0, 0]], // Normal 1
                [0, [0, 0, 0, 0, 0, 0]], // Release
            ]
        };
    }

    #[test]
    fn test_autoshift_hold_behavior_number() {
        // Test that holding number generates symbol (1 -> !)
        key_sequence_test! {
            keyboard: create_autoshift_keyboard(),
            sequence: [
                [0, 1, true, 10],   // Press '1'
                [0, 1, false, 270], // Hold past timeout
            ],
            expected_reports: [
                [KC_LSHIFT, [kc_to_u8!(Kc1), 0, 0, 0, 0, 0]], // Shift+1 (!)
                [0, [0, 0, 0, 0, 0, 0]], // Release all
            ]
        };
    }

    #[test]
    fn test_autoshift_tap_behavior_symbol() {
        // Test that quick tap generates normal symbol
        key_sequence_test! {
            keyboard: create_autoshift_keyboard(),
            sequence: [
                [0, 2, true, 50],    // Press ';' briefly
                [0, 2, false, 50],   // Release ';'
            ],
            expected_reports: [
                [0, [kc_to_u8!(Semicolon), 0, 0, 0, 0, 0]], // Normal ;
                [0, [0, 0, 0, 0, 0, 0]], // Release
            ]
        };
    }

    #[test]
    fn test_autoshift_hold_behavior_symbol() {
        // Test that holding symbol generates shifted symbol (; -> :)
        key_sequence_test! {
            keyboard: create_autoshift_keyboard(),
            sequence: [
                [0, 2, true, 10],   // Press ';'
                [0, 2, false, 270], // Hold past timeout
            ],
            expected_reports: [
                [KC_LSHIFT, [kc_to_u8!(Semicolon), 0, 0, 0, 0, 0]], // Shift+; (:)
                [0, [0, 0, 0, 0, 0, 0]], // Release all
            ]
        };
    }

    #[test]
    fn test_autoshift_disabled() {
        // Test that AutoShift doesn't activate when disabled
        key_sequence_test! {
            keyboard: create_autoshift_keyboard_disabled(),
            sequence: [
                [0, 0, true, 50],   // Press 'A' and hold past timeout
                [0, 0, false, 220],   // Release 'A'
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Normal A (AutoShift disabled)
                [0, [0, 0, 0, 0, 0, 0]], // Release
            ]
        };
    }

    #[test]
    fn test_autoshift_multiple_keys() {
        // Test AutoShift with multiple keys pressed - each key times out independently
        key_sequence_test! {
            keyboard: create_autoshift_keyboard(),
            sequence: [
                [0, 0, true, 10],    // Press 'A'
                [0, 1, true, 10],    // Press '1' shortly after
                [0, 0, false, 50],   // Release A quickly (tap)
                [0, 1, false, 220],  // Release '1' after timeout (hold - shifted)
            ],
            expected_reports: [
                [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Normal A (tap)
                [0, [0, 0, 0, 0, 0, 0]], // Release A
                [KC_LSHIFT, [kc_to_u8!(Kc1), 0, 0, 0, 0, 0]], // Shift+1 (1 timed out)
                [0, [0, 0, 0, 0, 0, 0]], // Release all
            ]
        };
    }

    #[test]
    fn test_autoshift_with_regular_tap_hold() {
        // Test AutoShift interaction with regular tap-hold keys - they should work independently
        key_sequence_test! {
            keyboard: create_autoshift_keyboard(),
            sequence: [
                [0, 3, true, 10],    // Press mt!(D, LGui)
                [0, 0, true, 10],    // Press 'A' shortly after
                [0, 3, false, 50],   // Release D quickly (tap)
                [0, 0, false, 220],  // Release 'A' after timeout (hold - AutoShift)
            ],
            expected_reports: [
                [0, [kc_to_u8!(D), 0, 0, 0, 0, 0]], // Normal D (tap)
                [0, [0, 0, 0, 0, 0, 0]], // Release D
                [KC_LSHIFT, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // Shift+A (A timed out)
                [0, [0, 0, 0, 0, 0, 0]], // Release all
            ]
        };
    }
}
