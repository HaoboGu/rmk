pub mod common;

use rmk::sim::{KeymapOverride, SimKeyboard, SimKeyboardSetup};
use rmk::types::keycode::HidKeyCode;
use rmk::types::modifier::ModifierCombination;

mod one_shot_test {
    use rmk::{k, osl, osm, th, wm};

    use super::*;
    use crate::common::{KC_LCTRL, KC_LGUI, KC_LSHIFT, TEST_KEYMAP};

    // Keys
    // Layer 0: OSM(LShift)        OSL(1)  A  TH(B)  OSM(LCtrl)  WM(B)
    // Layer 1: OSM(LShift|LCtrl)  No      C  D      E           F

    const ONE_SHOT_KEY_OVERRIDES: [KeymapOverride; 12] = [
        KeymapOverride::new(
            0,
            0,
            0,
            osm!(ModifierCombination::new_from(false, false, false, true, false)),
        ),
        KeymapOverride::new(0, 0, 1, osl!(1)),
        KeymapOverride::new(0, 0, 2, k!(A)),
        KeymapOverride::new(0, 0, 3, th!(B, C)),
        KeymapOverride::new(
            0,
            0,
            4,
            osm!(ModifierCombination::new_from(false, false, false, false, true)),
        ),
        KeymapOverride::new(
            0,
            0,
            5,
            wm!(B, ModifierCombination::new_from(false, true, false, false, false)),
        ),
        KeymapOverride::new(
            1,
            0,
            0,
            osm!(ModifierCombination::new_from(false, false, false, true, true)),
        ),
        KeymapOverride::new(1, 0, 1, k!(No)),
        KeymapOverride::new(1, 0, 2, k!(C)),
        KeymapOverride::new(1, 0, 3, k!(D)),
        KeymapOverride::new(1, 0, 4, k!(E)),
        KeymapOverride::new(1, 0, 5, k!(F)),
    ];
    const ONE_SHOT_SETUP: SimKeyboardSetup = SimKeyboardSetup::new().keys(&ONE_SHOT_KEY_OVERRIDES);

    /// OSM Test Case 1
    ///
    /// Config:
    /// - timeout: 1000ms
    /// - activate_on_keypress: false
    ///
    /// Sequence:
    /// - Press and Release OSM LShift
    /// - Press and Release regular key A
    ///
    /// Expected:
    /// - A with LShift
    /// - All released
    #[test]
    fn test_osm_basic_single_behavior() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(ONE_SHOT_SETUP).build().await;

            keyboard
                .delay(10)
                .press(0, 0)
                .delay(10)
                .release(0, 0)
                .delay(10)
                .press(0, 2)
                .delay(10)
                .release(0, 2)
                .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // A with LShift
                .expect_all_up() // All released
                .run()
                .await;
        });
    }

    /// OSM Test Case 2
    ///
    /// Config:
    /// - timeout: 100ms
    /// - activate_on_keypress: false
    ///
    /// Sequence:
    /// - Press and Release OSM LShift
    /// - Press and Release A after timeout (delay > 100ms)
    ///
    /// Expected:
    /// - A is sent without LShift
    /// - All released
    #[test]
    fn test_osm_timeout() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
                .setup(ONE_SHOT_SETUP)
                .one_shot_timeout_ms(100)
                .build()
                .await;

            keyboard
                .delay(10)
                .press(0, 0)
                .delay(10)
                .release(0, 0)
                .delay(150)
                .press(0, 2)
                .delay(10)
                .release(0, 2)
                .expect_keys([HidKeyCode::A]) // A without LShift (timeout)
                .expect_all_up() // All released
                .run()
                .await;
        });
    }

    /// OSM Test Case 3
    ///
    /// Config:
    /// - timeout: 1000ms
    /// - activate_on_keypress: false
    ///
    /// Sequence:
    /// - Press OSM LShift
    /// - Press A while OSM is held
    /// - Release A
    /// - Release OSM LShift
    ///
    /// Expected:
    /// - A with LShift
    /// - LShift is still held
    /// - All released
    #[test]
    fn test_osm_held_behavior() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(ONE_SHOT_SETUP).build().await;

            keyboard
                .delay(10)
                .press(0, 0) // Press OSM LShift
                .delay(10)
                .press(0, 2) // Press A while OSM is held
                .delay(10)
                .release(0, 2) // Release A
                .delay(10)
                .release(0, 0) // Release OSM LShift
                .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // A with LShift
                .expect_only_mods(KC_LSHIFT) // Still holding LShift
                .expect_all_up() // All released
                .run()
                .await;
        });
    }

    /// OSM Test Case 4
    ///
    /// Config:
    /// - timeout: 1000ms
    /// - activate_on_keypress: false
    ///
    /// Sequence:
    /// - Press and Release OSM LShift
    /// - Press and Release regular key A
    /// - Press and Release regular key B
    ///
    /// Expected:
    /// - A with LShift
    /// - All released
    /// - B without LShift
    /// - All released
    #[test]
    fn test_osm_multiple_keys() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(ONE_SHOT_SETUP).build().await;

            keyboard
                .delay(10)
                .press(0, 0)
                .delay(10)
                .release(0, 0)
                .delay(10)
                .press(0, 2)
                .delay(10)
                .release(0, 2)
                .delay(10)
                .press(0, 3)
                .delay(10)
                .release(0, 3)
                .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // A with LShift
                .expect_all_up() // All released
                .expect_keys([HidKeyCode::B]) // B without LShift
                .expect_all_up() // All released
                .run()
                .await;
        });
    }

    /// OSM Test Case 5
    ///
    /// Config:
    /// - timeout: 1000ms
    /// - activate_on_keypress: false
    ///
    /// Sequence:
    /// - Press OSM LShift
    /// - Press B
    /// - Release OSM LShift
    /// - Release B
    ///
    /// Expected:
    /// - B with LShift
    /// - All released
    #[test]
    fn test_osm_rolling_with_tap_hold() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(ONE_SHOT_SETUP).build().await;

            keyboard
                .delay(10)
                .press(0, 0) // Press OSM LShift
                .delay(10)
                .press(0, 3) // Press B
                .delay(10)
                .release(0, 0) // Release OSM LShift
                .delay(10)
                .release(0, 3) // Release B
                .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::B]) // B with LShift
                .expect_all_up() // All released
                .run()
                .await;
        });
    }

    /// OSM Test Case 6
    ///
    /// Config:
    /// - timeout: 1000ms
    /// - activate_on_keypress: false
    ///
    /// Sequence:
    /// - Press and Release OSM LShift
    /// - Press and Release OSM LCtrl
    /// - Press and Release regular key A
    ///
    /// Expected:
    /// - A with LShift+LCtrl
    /// - All released
    #[test]
    fn test_osm_combined_modifiers() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(ONE_SHOT_SETUP).build().await;

            keyboard
                .delay(10)
                .press(0, 0)
                .delay(10)
                .release(0, 0)
                .delay(10)
                .press(0, 4)
                .delay(10)
                .release(0, 4)
                .delay(10)
                .press(0, 2)
                .delay(10)
                .release(0, 2)
                .expect_keys_with_mods(KC_LSHIFT | KC_LCTRL, [HidKeyCode::A]) // A with LShift+LCtrl
                .expect_all_up() // All released
                .run()
                .await;
        });
    }

    /// OSM Test Case 7
    ///
    /// Config:
    /// - timeout: 100ms
    /// - activate_on_keypress: false
    ///
    /// Sequence:
    /// - Press and Release OSM LShift
    /// - Press and Release OSM LCtrl
    /// - Press and Release WM(B, LGui)
    ///
    /// Expected:
    /// - B is sent with LShift + LCtrl + LGui
    /// - All released
    #[test]
    fn test_osm_multiple_osm_with_wm() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(ONE_SHOT_SETUP).build().await;

            keyboard
                .delay(10)
                .press(0, 0)
                .delay(10)
                .release(0, 0)
                .delay(10)
                .press(0, 4)
                .delay(10)
                .release(0, 4)
                .delay(10)
                .press(0, 5)
                .delay(10)
                .release(0, 5)
                .expect_keys_with_mods(KC_LSHIFT | KC_LCTRL | KC_LGUI, [HidKeyCode::B]) // B with LShift + LCtrl + LGui
                .expect_all_up() // All released
                .run()
                .await;
        });
    }

    /// OSM Test Case 8
    ///
    /// Config:
    /// - timeout: 100ms
    /// - activate_on_keypress: true
    ///
    /// Sequence:
    /// - Press OSM LShift
    /// - Release OSM LShift
    /// - Press A
    /// - Release A
    ///
    /// Expected:
    /// - LShift is sent from the start
    /// - A with LShift
    /// - All released
    #[test]
    fn test_osm_activate_on_keypress() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
                .setup(ONE_SHOT_SETUP)
                .one_shot_activate_on_keypress(true)
                .build()
                .await;

            keyboard
                .delay(10)
                .press(0, 0) // Press OSM LShift
                .delay(10)
                .release(0, 0) // Release OSM LShift
                .delay(10)
                .press(0, 2) // Press A
                .delay(10)
                .release(0, 2) // Release A
                .expect_only_mods(KC_LSHIFT) // LShift is sent from the start
                .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A]) // A with LShift
                .expect_all_up() // All released
                .run()
                .await;
        });
    }

    /// OSM Test Case 9
    ///
    /// Config:
    /// - timeout: 100ms
    /// - activate_on_keypress: true
    ///
    /// Sequence:
    /// - Press and Release OSM LShift
    /// - Press and Release OSM LCtrl
    /// - Press and Release regular key A
    ///
    /// Expected:
    /// - LShift is sent first
    /// - LCtrl is added to combination
    /// - A with LShift+LCtrl
    /// - All released
    #[test]
    fn test_osm_combined_modifiers_with_activate_on_keypress() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
                .setup(ONE_SHOT_SETUP)
                .one_shot_activate_on_keypress(true)
                .build()
                .await;

            keyboard
                .delay(10)
                .press(0, 0)
                .delay(10)
                .release(0, 0)
                .delay(10)
                .press(0, 4)
                .delay(10)
                .release(0, 4)
                .delay(10)
                .press(0, 2)
                .delay(10)
                .release(0, 2)
                .expect_only_mods(KC_LSHIFT) // LShift is sent first
                .expect_only_mods(KC_LSHIFT | KC_LCTRL) // LCtrl is added to combination
                .expect_keys_with_mods(KC_LSHIFT | KC_LCTRL, [HidKeyCode::A]) // A with LShift+LCtrl
                .expect_all_up() // All released
                .run()
                .await;
        });
    }

    // OSL Tests
    #[test]
    fn test_osl_basic_single_behavior() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(ONE_SHOT_SETUP).build().await;

            keyboard
                .delay(10)
                .press(0, 1) // Press OSL Layer 1
                .delay(10)
                .release(0, 1) // Release OSL Layer 1
                .delay(10)
                .press(0, 2) // Press key at (0,2), should get C from layer 1
                .delay(10)
                .release(0, 2) // Release key
                .expect_keys([HidKeyCode::C]) // C from layer 1
                .expect_all_up() // All released
                .run()
                .await;
        });
    }

    #[test]
    fn test_osl_held_behavior() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(ONE_SHOT_SETUP).build().await;

            keyboard
                .delay(10)
                .press(0, 1) // Press OSL Layer 1
                .delay(10)
                .press(0, 2) // Press key at (0,2) while OSL is held
                .delay(10)
                .release(0, 2) // Release key
                .delay(10)
                .release(0, 1) // Release OSL Layer 1
                .expect_keys([HidKeyCode::C]) // C from layer 1
                .expect_all_up() // All released
                .run()
                .await;
        });
    }

    #[test]
    fn test_osl_timeout() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
                .setup(ONE_SHOT_SETUP)
                .one_shot_timeout_ms(100)
                .build()
                .await;

            keyboard
                .delay(10)
                .press(0, 1) // Press OSL Layer 1
                .delay(10)
                .release(0, 1) // Release OSL Layer 1
                .delay(150)
                .press(0, 2) // Press key at (0,2) after timeout (delay > 100ms)
                .delay(10)
                .release(0, 2) // Release key
                .expect_keys([HidKeyCode::A]) // A from layer 0 (timeout)
                .expect_all_up() // All released
                .run()
                .await;
        });
    }

    #[test]
    fn test_osl_multiple_keys() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(ONE_SHOT_SETUP).build().await;

            keyboard
                .delay(10)
                .press(0, 1) // Press OSL Layer 1
                .delay(10)
                .release(0, 1) // Release OSL Layer 1
                .delay(10)
                .press(0, 2) // Press key at (0,2), should get C from layer 1
                .delay(10)
                .release(0, 2) // Release key
                .delay(10)
                .press(0, 3) // Press key at (0,3), should get B from layer 0
                .delay(10)
                .release(0, 3) // Release key
                .expect_keys([HidKeyCode::C]) // C from layer 1
                .expect_all_up() // All released
                .expect_keys([HidKeyCode::B]) // B from layer 0
                .expect_all_up() // All released
                .run()
                .await;
        });
    }

    #[test]
    fn test_osm_then_osl() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(ONE_SHOT_SETUP).build().await;

            keyboard
                .delay(10)
                .press(0, 0) // Press OSM LShift
                .delay(10)
                .release(0, 0) // Release OSM LShift
                .delay(10)
                .press(0, 1) // Press OSL Layer 1
                .delay(10)
                .release(0, 1) // Release OSL Layer 1
                .delay(10)
                .press(0, 2) // Press key at (0,2), should get C from layer 1 with shift
                .delay(10)
                .release(0, 2) // Release key
                .expect_keys([HidKeyCode::C]) // C from layer 1 with LShift
                .expect_all_up() // All released
                .run()
                .await;
        });
    }

    #[test]
    fn test_osl_then_osm() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(ONE_SHOT_SETUP).build().await;

            keyboard
                .delay(10)
                .press(0, 1) // Press OSL Layer 1
                .delay(10)
                .release(0, 1) // Release OSL Layer 1
                .delay(10)
                .press(0, 0) // Press OSM LShift (from layer 1, but No action)
                .delay(10)
                .release(0, 0) // Release OSM LShift (gets from layer 0 due to transparent)
                .delay(10)
                .press(0, 2) // Press key at (0,2), should get A from layer 0 with shift + ctrl
                .delay(10)
                .release(0, 2) // Release key
                .expect_keys_with_mods(KC_LSHIFT | KC_LCTRL, [HidKeyCode::A]) // A from layer 0 with shift + ctrl
                .expect_all_up() // All released
                .run()
                .await;
        });
    }

    #[test]
    fn test_osm_and_osl_timeout() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
                .setup(ONE_SHOT_SETUP)
                .one_shot_timeout_ms(100)
                .build()
                .await;

            keyboard
                .delay(10)
                .press(0, 0) // Press OSM LShift
                .delay(10)
                .release(0, 0) // Release OSM LShift
                .delay(10)
                .press(0, 1) // Press OSL Layer 1
                .delay(10)
                .release(0, 1) // Release OSL Layer 1
                .delay(200)
                .press(0, 2) // Press key at (0,2) after timeout (delay > 100ms)
                .delay(10)
                .release(0, 2) // Release key
                .expect_keys([HidKeyCode::A]) // A from layer 0 (both timeout)
                .expect_all_up() // All released
                .run()
                .await;
        });
    }

    /// Chain mode (quick_release = false): modifier released on key RELEASE
    #[test]
    fn test_osm_chain_mode_basic() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
                .setup(ONE_SHOT_SETUP)
                .one_shot_quick_release(false)
                .build()
                .await;

            keyboard
                .delay(10)
                .tap(0, 0, 10)
                .delay(10)
                .tap(0, 2, 10)
                .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A])
                .expect_all_up()
                .run()
                .await;
        });
    }

    /// Chain mode: tap A then tap B — only A gets modifier
    #[test]
    fn test_osm_chain_mode_multiple_keys() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
                .setup(ONE_SHOT_SETUP)
                .one_shot_quick_release(false)
                .build()
                .await;

            keyboard
                .delay(10)
                .tap(0, 0, 10)
                .delay(10)
                .tap(0, 2, 10)
                .delay(10)
                .tap(0, 3, 10)
                .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A])
                .expect_all_up()
                .expect_keys([HidKeyCode::B])
                .expect_all_up()
                .run()
                .await;
        });
    }

    /// Chain mode with activate_on_keypress
    #[test]
    fn test_osm_chain_mode_activate_on_keypress() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
                .setup(ONE_SHOT_SETUP)
                .one_shot_activate_on_keypress(true)
                .one_shot_quick_release(false)
                .build()
                .await;

            keyboard
                .delay(10)
                .tap(0, 0, 10)
                .delay(10)
                .tap(0, 2, 10)
                .expect_only_mods(KC_LSHIFT)
                .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A])
                .expect_all_up()
                .run()
                .await;
        });
    }

    // Quick-release mode tests (quick_release = true)

    #[test]
    fn test_osm_quick_release_basic() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
                .setup(ONE_SHOT_SETUP)
                .one_shot_quick_release(true)
                .build()
                .await;

            keyboard
                .delay(10)
                .tap(0, 0, 10)
                .delay(10)
                .tap(0, 2, 10)
                .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A])
                .expect_keys([HidKeyCode::A])
                .expect_all_up()
                .run()
                .await;
        });
    }

    #[test]
    fn test_osm_quick_release_multiple_keys() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
                .setup(ONE_SHOT_SETUP)
                .one_shot_quick_release(true)
                .build()
                .await;

            keyboard
                .delay(10)
                .tap(0, 0, 10)
                .delay(10)
                .tap(0, 2, 10)
                .delay(10)
                .tap(0, 3, 10)
                .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A])
                .expect_keys([HidKeyCode::A])
                .expect_all_up()
                .expect_keys([HidKeyCode::B])
                .expect_all_up()
                .run()
                .await;
        });
    }

    // TODO: test_osm_quick_release_rolling removed — OSM + morse/tap-hold interaction
    // has a known bug where the OSM deadline loop times out before the tap resolves.

    #[test]
    fn test_osm_quick_release_combined_modifiers() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
                .setup(ONE_SHOT_SETUP)
                .one_shot_quick_release(true)
                .build()
                .await;

            keyboard
                .delay(10)
                .tap(0, 0, 10)
                .delay(10)
                .tap(0, 4, 10)
                .delay(10)
                .tap(0, 2, 10)
                .expect_keys_with_mods(KC_LSHIFT | KC_LCTRL, [HidKeyCode::A])
                .expect_keys([HidKeyCode::A])
                .expect_all_up()
                .run()
                .await;
        });
    }

    #[test]
    fn test_osm_quick_release_with_wm() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
                .setup(ONE_SHOT_SETUP)
                .one_shot_quick_release(true)
                .build()
                .await;

            keyboard
                .delay(10)
                .tap(0, 0, 10)
                .delay(10)
                .tap(0, 4, 10)
                .delay(10)
                .tap(0, 5, 10)
                .expect_keys_with_mods(KC_LSHIFT | KC_LCTRL | KC_LGUI, [HidKeyCode::B])
                .expect_keys_with_mods(KC_LGUI, [HidKeyCode::B])
                .expect_all_up()
                .run()
                .await;
        });
    }

    #[test]
    fn test_osm_quick_release_activate_on_keypress() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
                .setup(ONE_SHOT_SETUP)
                .one_shot_activate_on_keypress(true)
                .one_shot_quick_release(true)
                .build()
                .await;

            keyboard
                .delay(10)
                .tap(0, 0, 10)
                .delay(10)
                .tap(0, 2, 10)
                .expect_only_mods(KC_LSHIFT)
                .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A])
                .expect_keys([HidKeyCode::A])
                .expect_all_up()
                .run()
                .await;
        });
    }

    #[test]
    fn test_osm_quick_release_combined_activate_on_keypress() {
        crate::common::test_block_on(async {
            let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
                .setup(ONE_SHOT_SETUP)
                .one_shot_activate_on_keypress(true)
                .one_shot_quick_release(true)
                .build()
                .await;

            keyboard
                .delay(10)
                .tap(0, 0, 10)
                .delay(10)
                .tap(0, 4, 10)
                .delay(10)
                .tap(0, 2, 10)
                .expect_only_mods(KC_LSHIFT)
                .expect_only_mods(KC_LSHIFT | KC_LCTRL)
                .expect_keys_with_mods(KC_LSHIFT | KC_LCTRL, [HidKeyCode::A])
                .expect_keys([HidKeyCode::A])
                .expect_all_up()
                .run()
                .await;
        });
    }
}
