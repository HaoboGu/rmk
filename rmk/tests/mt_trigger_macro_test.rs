// Test for new mt! macro functionality with Action variants
// This test verifies that the extended mt! macro can accept:
// 1. Traditional HID keycodes (backward compatibility)
// 2. Bare Action variants like TriggerMacro(0)
// 3. Fully qualified Action variants like Action::TriggerMacro(0)

use rmk::mt;
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};
use rmk::types::modifier::ModifierCombination;

#[test]
fn test_mt_backward_compatible_hid_keycode() {
    // Traditional usage - HID keycode identifier
    let key_action = mt!(A, ModifierCombination::LSHIFT);

    // Verify it creates a TapHold action
    match key_action {
        KeyAction::TapHold(tap_action, hold_action, _) => {
            // Tap should be Key(Hid(A))
            assert!(matches!(tap_action, Action::Key(KeyCode::Hid(HidKeyCode::A))));
            // Hold should be Modifier(LSHIFT)
            assert!(matches!(hold_action, Action::Modifier(_)));
        }
        _ => panic!("Expected TapHold action"),
    }
}

#[test]
fn test_mt_bare_action_variant() {
    // New usage - Bare Action variant
    let key_action = mt!(TriggerMacro(0), ModifierCombination::LSHIFT);

    // Verify it creates a TapHold action
    match key_action {
        KeyAction::TapHold(tap_action, hold_action, _) => {
            // Tap should be TriggerMacro(0)
            assert!(matches!(tap_action, Action::TriggerMacro(0)));
            // Hold should be Modifier(LSHIFT)
            assert!(matches!(hold_action, Action::Modifier(_)));
        }
        _ => panic!("Expected TapHold action"),
    }
}

#[test]
fn test_mt_bare_action_variant_2() {
    // New usage - Bare Action variant
    let key_action = mt!(Action::TriLayerLower, ModifierCombination::LSHIFT);

    // Verify it creates a TapHold action
    match key_action {
        KeyAction::TapHold(tap_action, hold_action, _) => {
            // Tap should be TriggerMacro(0)
            assert!(matches!(tap_action, Action::TriLayerLower));
            // Hold should be Modifier(LSHIFT)
            assert!(matches!(hold_action, Action::Modifier(_)));
        }
        _ => panic!("Expected TapHold action"),
    }
}

#[test]
fn test_mt_fully_qualified_action() {
    // New usage - Fully qualified Action variant
    let key_action = mt!(Action::TriggerMacro(0), ModifierCombination::LSHIFT);

    // Verify it creates a TapHold action
    match key_action {
        KeyAction::TapHold(tap_action, hold_action, _) => {
            // Tap should be TriggerMacro(0)
            assert!(matches!(tap_action, Action::TriggerMacro(0)));
            // Hold should be Modifier(LSHIFT)
            assert!(matches!(hold_action, Action::Modifier(_)));
        }
        _ => panic!("Expected TapHold action"),
    }
}

#[test]
fn test_mt_other_action_variants() {
    // Test with LayerToggle
    let key_action = mt!(LayerToggle(1), ModifierCombination::LCTRL);

    match key_action {
        KeyAction::TapHold(tap_action, hold_action, _) => {
            assert!(matches!(tap_action, Action::LayerToggle(1)));
            assert!(matches!(hold_action, Action::Modifier(_)));
        }
        _ => panic!("Expected TapHold action"),
    }

    // Test with OneShotLayer
    let key_action2 = mt!(OneShotLayer(2), ModifierCombination::LALT);

    match key_action2 {
        KeyAction::TapHold(tap_action, hold_action, _) => {
            assert!(matches!(tap_action, Action::OneShotLayer(2)));
            assert!(matches!(hold_action, Action::Modifier(_)));
        }
        _ => panic!("Expected TapHold action"),
    }
}

#[test]
fn test_mt_different_modifier_combinations() {
    // Test with different modifiers
    let key1 = mt!(TriggerMacro(0), ModifierCombination::LSHIFT);
    let key2 = mt!(TriggerMacro(1), ModifierCombination::LCTRL);
    let key3 = mt!(TriggerMacro(2), ModifierCombination::LALT);
    let key4 = mt!(TriggerMacro(3), ModifierCombination::LGUI);

    // All should compile and create valid TapHold actions
    assert!(matches!(key1, KeyAction::TapHold(_, _, _)));
    assert!(matches!(key2, KeyAction::TapHold(_, _, _)));
    assert!(matches!(key3, KeyAction::TapHold(_, _, _)));
    assert!(matches!(key4, KeyAction::TapHold(_, _, _)));
}

#[test]
fn test_multiple_macro_indices() {
    // Test that different macro indices work correctly
    for i in 0..10 {
        let key_action = mt!(TriggerMacro(i), ModifierCombination::LSHIFT);

        match key_action {
            KeyAction::TapHold(tap_action, _, _) => match tap_action {
                Action::TriggerMacro(index) => {
                    assert_eq!(index, i, "Macro index should match");
                }
                _ => panic!("Expected TriggerMacro action"),
            },
            _ => panic!("Expected TapHold action"),
        }
    }
}

#[test]
fn test_mt_layer_actions() {
    // Test LayerOn
    let key1 = mt!(Action::LayerOn(1), ModifierCombination::LSHIFT);
    match key1 {
        KeyAction::TapHold(Action::LayerOn(1), Action::Modifier(_), _) => {}
        _ => panic!("Expected TapHold with LayerOn(1)"),
    }

    // Test LayerOff
    let key2 = mt!(Action::LayerOff(2), ModifierCombination::LCTRL);
    match key2 {
        KeyAction::TapHold(Action::LayerOff(2), Action::Modifier(_), _) => {}
        _ => panic!("Expected TapHold with LayerOff(2)"),
    }

    // Test DefaultLayer
    let key3 = mt!(Action::DefaultLayer(0), ModifierCombination::LALT);
    match key3 {
        KeyAction::TapHold(Action::DefaultLayer(0), Action::Modifier(_), _) => {}
        _ => panic!("Expected TapHold with DefaultLayer(0)"),
    }

    // Test LayerToggleOnly
    let key4 = mt!(Action::LayerToggleOnly(3), ModifierCombination::LGUI);
    match key4 {
        KeyAction::TapHold(Action::LayerToggleOnly(3), Action::Modifier(_), _) => {}
        _ => panic!("Expected TapHold with LayerToggleOnly(3)"),
    }
}

#[test]
fn test_mt_trilayer_both() {
    // Test both TriLayer variants
    let key1 = mt!(Action::TriLayerLower, ModifierCombination::LSHIFT);
    let key2 = mt!(Action::TriLayerUpper, ModifierCombination::LCTRL);

    match key1 {
        KeyAction::TapHold(Action::TriLayerLower, Action::Modifier(_), _) => {}
        _ => panic!("Expected TapHold with TriLayerLower"),
    }

    match key2 {
        KeyAction::TapHold(Action::TriLayerUpper, Action::Modifier(_), _) => {}
        _ => panic!("Expected TapHold with TriLayerUpper"),
    }
}

#[test]
fn test_mt_oneshot_actions() {
    // Test OneShotModifier
    let mod_combo = ModifierCombination::LALT;
    let key1 = mt!(Action::OneShotModifier(mod_combo), ModifierCombination::LSHIFT);
    match key1 {
        KeyAction::TapHold(Action::OneShotModifier(_), Action::Modifier(_), _) => {}
        _ => panic!("Expected TapHold with OneShotModifier"),
    }

    // Test OneShotKey
    let keycode = KeyCode::Hid(HidKeyCode::A);
    let key2 = mt!(Action::OneShotKey(keycode), ModifierCombination::LCTRL);
    match key2 {
        KeyAction::TapHold(Action::OneShotKey(_), Action::Modifier(_), _) => {}
        _ => panic!("Expected TapHold with OneShotKey"),
    }
}

#[test]
fn test_mt_user_action() {
    // Test User action with different indices
    for i in 0..5 {
        let key = mt!(Action::User(i), ModifierCombination::LSHIFT);
        match key {
            KeyAction::TapHold(Action::User(idx), Action::Modifier(_), _) => {
                assert_eq!(idx, i, "User index should match");
            }
            _ => panic!("Expected TapHold with User action"),
        }
    }
}

#[test]
fn test_mt_mixed_hid_keycodes() {
    // Test various HID keycodes to ensure backward compatibility
    let test_cases = vec![
        (mt!(A, ModifierCombination::LSHIFT), HidKeyCode::A),
        (mt!(B, ModifierCombination::LCTRL), HidKeyCode::B),
        (mt!(Space, ModifierCombination::LALT), HidKeyCode::Space),
        (mt!(Enter, ModifierCombination::LGUI), HidKeyCode::Enter),
        (mt!(Escape, ModifierCombination::LSHIFT), HidKeyCode::Escape),
    ];

    for (key_action, expected_keycode) in test_cases {
        match key_action {
            KeyAction::TapHold(Action::Key(KeyCode::Hid(kc)), Action::Modifier(_), _) => {
                assert_eq!(kc, expected_keycode, "HID keycode should match");
            }
            _ => panic!("Expected TapHold with HID keycode"),
        }
    }
}

#[test]
fn test_mt_macro_boundary_indices() {
    // Test macro indices at boundaries (0, 1, 31, 127, 255)
    let test_indices = vec![0u8, 1, 31, 127, 255];

    for i in test_indices {
        let key = mt!(TriggerMacro(i), ModifierCombination::LSHIFT);
        match key {
            KeyAction::TapHold(Action::TriggerMacro(idx), Action::Modifier(_), _) => {
                assert_eq!(idx, i, "Macro index {} should match", i);
            }
            _ => panic!("Expected TapHold with TriggerMacro({})", i),
        }
    }
}

#[test]
fn test_mt_complex_modifier_combinations() {
    // Test with complex modifier combinations
    let mod1 = ModifierCombination::LSHIFT;
    let mod2 = ModifierCombination::LCTRL.with_left_shift(true);
    let mod3 = ModifierCombination::new_from(true, true, false, true, false); // right=true, gui=true, alt=false, shift=true, ctrl=false

    let key1 = mt!(TriggerMacro(0), mod1);
    let key2 = mt!(TriggerMacro(1), mod2);
    let key3 = mt!(TriggerMacro(2), mod3);

    // All should create valid TapHold actions with Modifier hold actions
    assert!(matches!(key1, KeyAction::TapHold(_, Action::Modifier(_), _)));
    assert!(matches!(key2, KeyAction::TapHold(_, Action::Modifier(_), _)));
    assert!(matches!(key3, KeyAction::TapHold(_, Action::Modifier(_), _)));
}
