pub mod common;

use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Instant, Timer};
use rmk::channel::USB_REPORT_CHANNEL;
use rmk::config::{
    BehaviorConfig, CombosConfig, MorsesConfig, OneShotConfig, OneShotModifiersConfig, PositionalConfig,
};
use rmk::core_traits::Runnable;
use rmk::event::{AsyncEventPublisher, AsyncPublishableEvent, KeyboardEvent};
use rmk::hid::Report;
use rmk::keyboard::Keyboard;
use rmk::keyboard::combo::{Combo, ComboConfig};
use rmk::sim::SimKeyboard;
use rmk::state::set_usb_state;
use rmk::types::action::KeyAction;
use rmk::types::connection::UsbState;
use rmk::types::keycode::HidKeyCode;
use rmk::types::modifier::ModifierCombination;
use rmk::{a, k, layer, osm, th, wm};
use rmk_types::morse::{MorseMode, MorseProfile};

use crate::common::test_block_on::test_block_on;
use crate::common::{KC_LCTRL, KC_LSHIFT, create_test_keyboard_with_config, wrap_keymap};

const STANDARD_2_KEY_COMBOS: [([KeyAction; 2], KeyAction); 4] = [
    ([k!(V), k!(B)], k!(LShift)),
    ([k!(R), k!(T)], k!(LAlt)),
    (
        [k!(E), k!(T)],
        osm!(ModifierCombination::new_from(false, false, false, true, false)),
    ),
    ([k!(E), k!(R)], k!(A)),
];

const STANDARD_3_KEY_COMBOS: [([KeyAction; 3], KeyAction); 2] =
    [([k!(E), k!(R), k!(T)], k!(Space)), ([k!(V), k!(B), k!(T)], k!(Space))];

#[test]
fn test_single_key_in_combo() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(crate::common::TEST_KEYMAP)
            .combos_on_layer(0, STANDARD_2_KEY_COMBOS)
            .combos_on_layer(0, STANDARD_3_KEY_COMBOS)
            .combo_timeout_ms(100)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(1, 3)
            .delay(50)
            .release(1, 3)
            .delay(10)
            .press(1, 4)
            .delay(50)
            .release(1, 4)
            .delay(10)
            .press(1, 5)
            .delay(10)
            .release(1, 5)
            .expect_keys([HidKeyCode::E])
            .expect_all_up()
            .expect_keys([HidKeyCode::R])
            .expect_all_up()
            .expect_keys([HidKeyCode::T])
            .expect_all_up()
            .run()
            .await;
    });
}
#[test]
fn test_combo_timeout_and_ignore() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(crate::common::TEST_KEYMAP)
            .combos_on_layer(0, STANDARD_2_KEY_COMBOS)
            .combos_on_layer(0, STANDARD_3_KEY_COMBOS)
            .combo_timeout_ms(100)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(3, 4)
            .delay(100)
            .release(3, 4)
            .expect_keys([HidKeyCode::V])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_combo_with_mod_then_mod_timeout() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(crate::common::TEST_KEYMAP)
            .combos_on_layer(0, STANDARD_2_KEY_COMBOS)
            .combos_on_layer(0, STANDARD_3_KEY_COMBOS)
            .combo_timeout_ms(100)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(3, 4) // Press V
            .delay(10)
            .press(3, 5) // Press B
            .delay(50)
            .press(1, 4) // Press R
            .delay(90)
            .release(1, 4) // Release R
            .delay(150)
            .release(3, 4) // Release V
            .delay(170)
            .release(3, 5) // Release B
            .expect_only_mods(KC_LSHIFT) // V + B = LShift
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::R]) // Press R
            .expect_only_mods(KC_LSHIFT) // Release R
            .expect_all_up() // Release V + B
            .run()
            .await;
    });
}

#[test]
fn test_combo_with_one_shot_modifier() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(crate::common::TEST_KEYMAP)
            .one_shot_timeout_ms(300)
            .combos_on_layer(0, STANDARD_2_KEY_COMBOS)
            .combos_on_layer(0, STANDARD_3_KEY_COMBOS)
            .combo_timeout_ms(100)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(1, 3)
            .delay(10)
            .press(1, 5)
            .delay(50)
            .release(1, 3)
            .delay(70)
            .release(1, 5)
            .delay(50)
            .press(1, 3)
            .delay(110)
            .release(1, 3)
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::E])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_combo_with_mod() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(crate::common::TEST_KEYMAP)
            .combos_on_layer(0, STANDARD_2_KEY_COMBOS)
            .combos_on_layer(0, STANDARD_3_KEY_COMBOS)
            .combo_timeout_ms(100)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(3, 4) // V
            .delay(10)
            .press(3, 5) // B
            .delay(50)
            .press(3, 6) // N, trigger V + B = LShift
            .delay(70)
            .release(3, 6)
            .delay(100)
            .release(3, 4)
            .delay(110)
            .release(3, 5)
            .expect_only_mods(KC_LSHIFT)
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::N])
            .expect_only_mods(KC_LSHIFT)
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_fully_overlapped_combo_timeout() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(crate::common::TEST_KEYMAP)
            .combos_on_layer(0, STANDARD_2_KEY_COMBOS)
            .combos_on_layer(0, STANDARD_3_KEY_COMBOS)
            .combo_timeout_ms(100)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(1, 3) // E
            .delay(10)
            .press(1, 4) // T
            .delay(170)
            .release(1, 3) // Timeout, should trigger E+T = A because E+T are triggered within the timeout window
            .delay(10)
            .release(1, 4)
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_fully_overlapped_combo_trigger_smaller() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(crate::common::TEST_KEYMAP)
            .combos_on_layer(0, STANDARD_2_KEY_COMBOS)
            .combos_on_layer(0, STANDARD_3_KEY_COMBOS)
            .combo_timeout_ms(100)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(1, 3) // E
            .delay(10)
            .press(1, 4) // T
            .delay(10)
            .release(1, 3)
            .delay(10)
            .release(1, 4)
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_fully_overlapped_combo() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(crate::common::TEST_KEYMAP)
            .combos_on_layer(0, STANDARD_2_KEY_COMBOS)
            .combos_on_layer(0, STANDARD_3_KEY_COMBOS)
            .combo_timeout_ms(100)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(1, 3) // E
            .delay(10)
            .press(1, 5) // T
            .delay(10)
            .press(1, 4) // R
            .delay(50)
            .release(1, 3)
            .delay(10)
            .release(1, 5)
            .delay(50)
            .release(1, 4)
            .delay(10)
            .press(1, 3) // E
            .delay(10)
            .press(1, 5) // T
            .delay(50)
            .release(1, 3)
            .delay(10)
            .release(1, 5)
            .delay(10)
            .press(1, 3) // E
            .delay(10)
            .press(1, 4) // R
            .delay(50)
            .release(1, 3)
            .delay(50)
            .release(1, 4)
            .delay(10)
            .press(1, 3) // E
            .delay(10)
            .press(1, 5) // T
            .delay(10)
            .press(1, 4) // R
            .delay(50)
            .release(1, 3)
            .delay(10)
            .release(1, 5)
            .delay(50)
            .release(1, 4)
            .expect_keys([HidKeyCode::Space])
            .expect_all_up()
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A])
            .expect_all_up()
            .expect_keys([HidKeyCode::Space])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_overlapped_combo() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(crate::common::TEST_KEYMAP)
            .combos_on_layer(0, STANDARD_2_KEY_COMBOS)
            .combos_on_layer(0, STANDARD_3_KEY_COMBOS)
            .combo_timeout_ms(100)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(1, 3)
            .delay(10)
            .press(1, 5)
            .delay(50)
            .release(1, 3)
            .delay(10)
            .release(1, 5)
            .delay(100)
            .press(1, 4)
            .delay(10)
            .press(1, 3)
            .delay(50)
            .release(1, 4)
            .delay(10)
            .release(1, 3)
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::A])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_taphold_with_combo() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(crate::common::TEST_KEYMAP)
            .morse_default_profile(MorseProfile::new(
                Some(false),
                Some(MorseMode::PermissiveHold),
                Some(250u16),
                Some(250u16),
            ))
            .combo_global([th!(A, LShift), th!(S, LGui), th!(Z, LAlt)], k!(C))
            .combo_timeout_ms(50)
            .build()
            .await;

        keyboard
            .delay(20)
            .press(2, 1) // Press th!(A,shift)
            .delay(20)
            .press(2, 2) // Press th!(S,LGui)
            .delay(20)
            .press(3, 1) // Press th!(Z,LAlt)
            .delay(10)
            .release(2, 1) // Release A
            .delay(10)
            .release(2, 2) // Release S
            .delay(10)
            .release(3, 1) // Release Z
            .expect_keys([HidKeyCode::C])
            .expect_all_up()
            .run()
            .await;
    });
}
// Reproduces a single-combo stuck-key bug: re-pressing a combo key while the
// combo is still held (one key of the chord was released, same key pressed
// again) leaked the re-press into the HID report and overwrote the combo
// output's slot. When the other combo key finally released, the combo output
// release couldn't find its slot, leaving the re-pressed key stuck.
#[test]
fn test_re_press_combo_key_while_triggered_does_not_leak_to_hid() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(crate::common::TEST_KEYMAP)
            .combo_on_layer(0, [k!(Comma), k!(Dot)], k!(Backspace))
            .combo_timeout_ms(40)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(3, 8) // Comma press
            .delay(10)
            .press(3, 9) // Dot press -> `,+.` triggers -> Backspace pressed
            .delay(10)
            .release(3, 9) // Dot release (partial release, swallowed)
            .delay(10)
            .press(3, 9) // Dot re-press while combo still held
            .delay(10)
            .release(3, 9) // Dot re-release (still part of combo)
            .delay(10)
            .release(3, 8) // Comma release -> combo fully releases -> Backspace released
            .expect_keys([HidKeyCode::Backspace])
            .expect_all_up()
            .run()
            .await;
    });
}

// Reproduces a stuck combo-output bug on overlapping triggered combos.
//
// Config: `M+,` → RightBracket, `,+.` → Equal. The two combos share Comma.
//
// Sequence: typing that ends with two triggered combos whose state bits overlap
// through Comma. When Comma is finally released, both combo outputs must
// unregister from the HID report. Previously only one did — the other got
// stuck on the host until the user pressed another key.
//
// The cascade specifically relies on state bits surviving across a prior combo
// trigger: pressing Dot+Comma triggers `,+.` (→ Equal) but leaves Comma's bit
// set in `M+,`, so a subsequent M press immediately completes `M+,` without
// re-pressing Comma.
#[test]
fn test_overlapping_triggered_combos_release_all_outputs() {
    crate::common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(crate::common::TEST_KEYMAP)
            .combo_on_layer(0, [k!(M), k!(Comma)], k!(RightBracket))
            .combo_on_layer(0, [k!(Comma), k!(Dot)], k!(Equal))
            .combo_timeout_ms(40)
            .build()
            .await;

        keyboard
            .delay(10)
            .press(3, 9) // Dot press
            .delay(10)
            .press(3, 8) // Comma press -> `,+.` triggers -> Equal pressed
            .delay(10)
            .release(3, 9) // Dot release (partial release of triggered combo)
            .delay(10)
            .press(3, 7) // M press -> `M+,` triggers (stale Comma bit) -> RightBracket pressed
            .delay(10)
            .release(3, 7) // M release (partial release of triggered combo)
            .delay(10)
            .release(3, 8) // Comma release -> must release BOTH combo outputs
            .expect_keys([HidKeyCode::Equal])
            .expect_keys([HidKeyCode::Equal, HidKeyCode::RightBracket])
            .expect_keys([HidKeyCode::Equal])
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn test_combo_with_one_shot_modifier_quick_release() {
    key_sequence_test! {
        keyboard: create_test_keyboard_with_config(BehaviorConfig {
            combo: get_combos_config(),
            one_shot: OneShotConfig {
                timeout: Duration::from_millis(300),
                ..Default::default()
            },
            one_shot_modifiers: OneShotModifiersConfig {
                quick_release: true,
                ..Default::default()
            },
            ..Default::default()
        }),
        sequence: [
            [1, 3, true, 10],
            [1, 5, true, 10],
            [1, 3, false, 50],
            [1, 5, false, 70],
            [1, 3, true, 50],
            [1, 3, false, 110],
        ],
        expected_reports: [
            [KC_LSHIFT, [HidKeyCode::E as u8, 0, 0, 0, 0, 0]],
            [0, [HidKeyCode::E as u8, 0, 0, 0, 0, 0]],
            [0, [0; 6]],
        ]
    }
}

#[test]
fn test_overlapped_combo_quick_release() {
    key_sequence_test! {
        keyboard: create_test_keyboard_with_config(BehaviorConfig {
            combo: get_combos_config(),
            one_shot_modifiers: OneShotModifiersConfig {
                quick_release: true,
                ..Default::default()
            },
            ..Default::default()
        }),
        sequence: [
            [1, 3, true, 10],
            [1, 5, true, 10],
            [1, 3, false, 50],
            [1, 5, false, 10],
            [1, 4, true, 100],
            [1, 3, true, 10],
            [1, 4, false, 50],
            [1, 3, false, 10],
        ],
        expected_reports: [
            [KC_LSHIFT, [HidKeyCode::A as u8, 0, 0, 0, 0, 0]],
            [0, [HidKeyCode::A as u8, 0, 0, 0, 0, 0]],
            [0, [0; 6]],
        ]
    }
}

// Regression check for the overlapping subset combos bug.
//
//   W + E     -> F2
//   E + R     -> F4
//   W + E + R -> Ctrl + Shift + R   (output *contains* R, itself a trigger key)
//
// Before the fix, firing W+E+R left the `W+E` subset combo "all-pressed" and it
// later fired as a phantom `F2`, tangling the release of `Ctrl+Shift+R` into
// `[R, F2]` instead of a clean release — stranding R/modifiers on the host.
// `reset_shadowed_combos` now clears every fully-pressed, not-triggered combo
// that shares a key with the one that fired.
//
// Positions in the test keymap (tests/common/mod.rs): W=[1,2] E=[1,3] R=[1,4]
fn subset_combos() -> CombosConfig {
    CombosConfig {
        combos: [
            Some(Combo::new(ComboConfig::new([k!(W), k!(E)].to_vec(), k!(F2), Some(0)))),
            Some(Combo::new(ComboConfig::new([k!(E), k!(R)].to_vec(), k!(F4), Some(0)))),
            Some(Combo::new(ComboConfig::new(
                [k!(W), k!(E), k!(R)].to_vec(),
                wm!(R, ModifierCombination::new_from(false, false, false, true, true)),
                Some(0),
            ))),
            None,
            None,
            None,
            None,
            None,
        ],
        timeout: Duration::from_millis(50),
        prior_idle_time: None,
    }
}

// Fire W+E+R, release everything, then press R alone. The combo must emit
// exactly Ctrl+Shift+R down then a clean release (no phantom F2, no stranded R),
// and the following lone R must still register.
#[test]
fn test_subset_combo_wer_then_r_alone() {
    key_sequence_test! {
        keyboard: create_test_keyboard_with_config(BehaviorConfig {
            combo: subset_combos(),
            ..Default::default()
        }),
        sequence: [
            [1, 2, true, 10],   // W press
            [1, 3, true, 10],   // E press
            [1, 4, true, 10],   // R press -> W+E+R triggers -> Ctrl+Shift+R
            [1, 2, false, 30],  // W release
            [1, 3, false, 10],  // E release
            [1, 4, false, 10],  // R release -> combo fully released
            [1, 4, true, 80],   // R press alone (past the 50ms combo timeout)
            [1, 4, false, 80],  // R release
        ],
        expected_reports: [
            [KC_LCTRL | KC_LSHIFT, [HidKeyCode::R as u8, 0, 0, 0, 0, 0]], // combo output
            [0, [0; 6]],                                                  // clean release
            [0, [HidKeyCode::R as u8, 0, 0, 0, 0, 0]],                    // lone R press
            [0, [0; 6]],                                                  // lone R release
        ]
    }
}

// Same, but releasing R first (rolling off the chord) — the ordering that most
// obviously exposed the stranded-R behaviour before the fix.
#[test]
fn test_subset_combo_wer_release_r_first() {
    key_sequence_test! {
        keyboard: create_test_keyboard_with_config(BehaviorConfig {
            combo: subset_combos(),
            ..Default::default()
        }),
        sequence: [
            [1, 2, true, 10],   // W press
            [1, 3, true, 10],   // E press
            [1, 4, true, 10],   // R press -> W+E+R triggers -> Ctrl+Shift+R
            [1, 4, false, 30],  // R release first
            [1, 3, false, 10],  // E release
            [1, 2, false, 10],  // W release -> combo fully released
            [1, 4, true, 80],   // R press alone
            [1, 4, false, 80],  // R release
        ],
        expected_reports: [
            [KC_LCTRL | KC_LSHIFT, [HidKeyCode::R as u8, 0, 0, 0, 0, 0]], // combo output
            [0, [0; 6]],                                                  // clean release
            [0, [HidKeyCode::R as u8, 0, 0, 0, 0, 0]],                    // lone R press
            [0, [0; 6]],                                                  // lone R release
        ]
    }
}

// Regression for the stuck mouse-wheel bug, end to end.
//
// Combo `MouseAccel2 + MouseWheelUp -> No` (a mouse key as a combo trigger).
// When the combo fires it discards the buffered WheelUp *press*, so the mouse
// layer never sees it. The combo must therefore also consume the WheelUp
// *release* — otherwise the mouse gets an unpaired release and the wheel
// auto-repeats forever ("Mouse Wheel Up stucks"). This drives the full
// keyboard and asserts the wheel is silent once every key is up.
//
// MouseWheelUp = [0,0], MouseAccel2 = [0,1].
fn wheel_combo_keyboard() -> Keyboard<'static> {
    let keymap: [[[KeyAction; 14]; 5]; 1] = [layer!([
        [
            k!(MouseWheelUp),
            k!(MouseAccel2),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No)
        ],
        [
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No)
        ],
        [
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No)
        ],
        [
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No)
        ],
        [
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No),
            a!(No)
        ]
    ])];
    let combos = CombosConfig {
        combos: [
            Some(Combo::new(ComboConfig::new(
                [k!(MouseAccel2), k!(MouseWheelUp)].to_vec(),
                a!(No),
                Some(0),
            ))),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        timeout: Duration::from_millis(50),
        prior_idle_time: None,
    };
    let config: &'static mut BehaviorConfig = Box::leak(Box::new(BehaviorConfig {
        combo: combos,
        ..Default::default()
    }));
    let per_key: &'static PositionalConfig<5, 14> = Box::leak(Box::new(PositionalConfig::default()));
    Keyboard::new(wrap_keymap(keymap, per_key, config))
}

/// Run `seq`, let things settle (draining the transient press/stop reports),
/// then count mouse reports still arriving. A healthy wheel is silent; a stuck
/// one keeps repeating with no key held.
fn ongoing_mouse_reports(seq: &'static [(u8, u8, bool, u64)]) -> u32 {
    test_block_on(async move {
        let mut keyboard = wheel_combo_keyboard();
        let sender = KeyboardEvent::publisher_async();
        sender.clear();
        USB_REPORT_CHANNEL.clear();
        set_usb_state(UsbState::Configured);

        let mut count = 0;
        select(keyboard.run(), async {
            for &(r, c, p, d) in seq {
                Timer::after(Duration::from_millis(d)).await;
                sender.publish_async(KeyboardEvent::key(r, c, p)).await;
            }
            let settle_end = Instant::now() + Duration::from_millis(250);
            loop {
                match select(Timer::at(settle_end), USB_REPORT_CHANNEL.receive()).await {
                    Either::First(_) => break,
                    Either::Second(_) => {}
                }
            }
            for _ in 0..6 {
                match select(Timer::after(Duration::from_millis(50)), USB_REPORT_CHANNEL.receive()).await {
                    Either::First(_) => {}
                    Either::Second(Report::MouseReport(_)) => count += 1,
                    Either::Second(_) => {}
                }
            }
        })
        .await;
        count
    })
}

// Combo fires (WheelUp press swallowed, Accel2 completes it); after both keys
// are up the wheel must be silent, not stuck repeating.
#[test]
fn test_mouse_key_combo_does_not_stick_wheel() {
    let seq: &[(u8, u8, bool, u64)] = &[
        (0, 0, true, 10),  // MouseWheelUp press  -> buffered (WaitingCombo)
        (0, 1, true, 10),  // MouseAccel2 press   -> completes combo -> No
        (0, 0, false, 30), // MouseWheelUp release
        (0, 1, false, 10), // MouseAccel2 release -> all keys up
    ];
    let n = ongoing_mouse_reports(seq);
    assert_eq!(
        n, 0,
        "wheel kept emitting {n} reports after all keys released (stuck scroll)"
    );
}

// Control: a plain WheelUp tap (combo never completes) dispatches on timeout,
// the release balances it, and the wheel goes quiet — guards against the combo
// fix wrongly swallowing a legitimate mouse press/release pair.
#[test]
fn test_mouse_wheel_tap_settles() {
    let seq: &[(u8, u8, bool, u64)] = &[
        (0, 0, true, 10),   // WheelUp press (combo times out; no Accel2)
        (0, 0, false, 120), // WheelUp release, well after the 50ms timeout
    ];
    let n = ongoing_mouse_reports(seq);
    assert_eq!(n, 0, "wheel still emitting {n} reports after release");
}
