// Test for lt! (layer-tap) macro with Action variants
// This test verifies that the extended lt! macro can accept:
// 1. Traditional HID keycodes (backward compatibility)
// 2. Bare Action variants like TriggerMacro(0)
// 3. Fully qualified Action variants like Action::TriggerMacro(0)
// 4. Action variants without arguments like Action::TriLayerLower

use rmk::lt;
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};

#[test]
fn test_lt_backward_compatible_hid_keycode() {
    // Traditional usage - HID keycode identifier
    let key_action = lt!(1, Space);

    // Verify it creates a TapHold action
    match key_action {
        KeyAction::TapHold(tap_action, hold_action, _) => {
            // Tap should be Key(Hid(Space))
            assert!(matches!(
                tap_action,
                Action::Key(KeyCode::Hid(HidKeyCode::Space))
            ));
            // Hold should be LayerOn(1)
            assert!(matches!(hold_action, Action::LayerOn(1)));
        }
        _ => panic!("Expected TapHold action"),
    }
}

#[test]
fn test_lt_bare_action_variant() {
    // New usage - Bare Action variant
    let key_action = lt!(2, TriggerMacro(0));

    // Verify it creates a TapHold action
    match key_action {
        KeyAction::TapHold(tap_action, hold_action, _) => {
            // Tap should be TriggerMacro(0)
            assert!(matches!(tap_action, Action::TriggerMacro(0)));
            // Hold should be LayerOn(2)
            assert!(matches!(hold_action, Action::LayerOn(2)));
        }
        _ => panic!("Expected TapHold action"),
    }
}

#[test]
fn test_lt_fully_qualified_action() {
    // New usage - Fully qualified Action variant
    let key_action = lt!(3, Action::TriggerMacro(1));

    // Verify it creates a TapHold action
    match key_action {
        KeyAction::TapHold(tap_action, hold_action, _) => {
            // Tap should be TriggerMacro(1)
            assert!(matches!(tap_action, Action::TriggerMacro(1)));
            // Hold should be LayerOn(3)
            assert!(matches!(hold_action, Action::LayerOn(3)));
        }
        _ => panic!("Expected TapHold action"),
    }
}

#[test]
fn test_lt_action_without_arguments() {
    // New usage - Action variant without arguments
    let key_action = lt!(1, Action::TriLayerLower);

    // Verify it creates a TapHold action
    match key_action {
        KeyAction::TapHold(tap_action, hold_action, _) => {
            // Tap should be TriLayerLower
            assert!(matches!(tap_action, Action::TriLayerLower));
            // Hold should be LayerOn(1)
            assert!(matches!(hold_action, Action::LayerOn(1)));
        }
        _ => panic!("Expected TapHold action"),
    }
}

#[test]
fn test_lt_multiple_layers() {
    // Test different layer numbers with specific literals
    let key0 = lt!(0, TriggerMacro(0));
    let key1 = lt!(1, TriggerMacro(1));
    let key2 = lt!(2, TriggerMacro(2));
    let key3 = lt!(3, TriggerMacro(3));
    let key4 = lt!(4, TriggerMacro(4));

    let test_cases = vec![
        (key0, 0u8, 0u8),
        (key1, 1, 1),
        (key2, 2, 2),
        (key3, 3, 3),
        (key4, 4, 4),
    ];

    for (key_action, expected_macro, expected_layer) in test_cases {
        match key_action {
            KeyAction::TapHold(tap_action, hold_action, _) => {
                match tap_action {
                    Action::TriggerMacro(idx) => {
                        assert_eq!(idx, expected_macro, "Macro index should match");
                    }
                    _ => panic!("Expected TriggerMacro action"),
                }
                match hold_action {
                    Action::LayerOn(l) => {
                        assert_eq!(l, expected_layer, "Layer should match");
                    }
                    _ => panic!("Expected LayerOn action"),
                }
            }
            _ => panic!("Expected TapHold action"),
        }
    }
}

#[test]
fn test_lt_various_actions() {
    // Test LayerToggle
    let key1 = lt!(1, LayerToggle(2));
    match key1 {
        KeyAction::TapHold(Action::LayerToggle(2), Action::LayerOn(1), _) => {}
        _ => panic!("Expected TapHold with LayerToggle"),
    }

    // Test OneShotLayer
    let key2 = lt!(2, OneShotLayer(3));
    match key2 {
        KeyAction::TapHold(Action::OneShotLayer(3), Action::LayerOn(2), _) => {}
        _ => panic!("Expected TapHold with OneShotLayer"),
    }

    // Test User action
    let key3 = lt!(3, Action::User(5));
    match key3 {
        KeyAction::TapHold(Action::User(5), Action::LayerOn(3), _) => {}
        _ => panic!("Expected TapHold with User"),
    }

    // Test TriLayerUpper
    let key4 = lt!(1, Action::TriLayerUpper);
    match key4 {
        KeyAction::TapHold(Action::TriLayerUpper, Action::LayerOn(1), _) => {}
        _ => panic!("Expected TapHold with TriLayerUpper"),
    }
}

#[test]
fn test_lt_mixed_hid_keycodes() {
    // Test various HID keycodes to ensure backward compatibility
    let test_cases = vec![
        (lt!(1, A), HidKeyCode::A, 1),
        (lt!(2, B), HidKeyCode::B, 2),
        (lt!(3, Space), HidKeyCode::Space, 3),
        (lt!(1, Enter), HidKeyCode::Enter, 1),
        (lt!(0, Escape), HidKeyCode::Escape, 0),
    ];

    for (key_action, expected_keycode, expected_layer) in test_cases {
        match key_action {
            KeyAction::TapHold(Action::Key(KeyCode::Hid(kc)), Action::LayerOn(layer), _) => {
                assert_eq!(kc, expected_keycode, "HID keycode should match");
                assert_eq!(layer, expected_layer, "Layer should match");
            }
            _ => panic!("Expected TapHold with HID keycode"),
        }
    }
}
