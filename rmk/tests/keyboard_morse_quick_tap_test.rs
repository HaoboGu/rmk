/// Tests for the `quick_tap_timeout_ms` morse profile option.
///
/// When the user re-presses a morse/tap-hold key within `quick_tap_timeout_ms` of its
/// previous release, the tap action fires immediately on press. Holding the key
/// down then triggers OS-level auto-repeat of the tap action instead of
/// resolving to the hold action.
pub mod common;

use heapless::Vec;
use rmk::config::{BehaviorConfig, MorsesConfig, PositionalConfig};
use rmk::keyboard::Keyboard;
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};
use rmk::types::morse::{HOLD, Morse, MorseMode, MorsePattern, MorseProfile};
use rmk::{k, td};

use crate::common::wrap_keymap;

const KEYMAP_LAYER2: [[KeyAction; 4]; 1] = [[k!(Kp1), k!(Kp2), k!(Kp3), k!(Kp4)]];

fn default_profile(quick_tap: Option<u16>) -> MorseProfile {
    let p = MorseProfile::new(
        Some(false),
        Some(MorseMode::HoldOnOtherPress),
        Some(250u16),
        Some(250u16),
    );
    if let Some(t) = quick_tap {
        p.with_quick_tap_timeout_ms(Some(t))
    } else {
        p
    }
}

fn make_keyboard_with_morse(morses: Vec<Morse, 8>, global_profile: MorseProfile) -> Keyboard<'static> {
    let keymap = [[[td!(0), k!(E), k!(F), k!(A)]], KEYMAP_LAYER2];

    let behavior_config = BehaviorConfig {
        morse: MorsesConfig {
            enable_flow_tap: false,
            default_profile: global_profile,
            morses,
            ..Default::default()
        },
        ..Default::default()
    };

    let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(behavior_config));
    let per_key_config: &'static PositionalConfig<1, 4> = Box::leak(Box::new(PositionalConfig::default()));
    Keyboard::new(wrap_keymap(keymap, per_key_config, behavior_config))
}

fn make_keyboard_with_keyaction(key_action: KeyAction, global_profile: MorseProfile) -> Keyboard<'static> {
    let keymap = [[[key_action, k!(E), k!(F), k!(B)]], KEYMAP_LAYER2];

    let behavior_config = BehaviorConfig {
        morse: MorsesConfig {
            enable_flow_tap: false,
            default_profile: global_profile,
            ..Default::default()
        },
        ..Default::default()
    };

    let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(behavior_config));
    let per_key_config: &'static PositionalConfig<1, 4> = Box::leak(Box::new(PositionalConfig::default()));
    Keyboard::new(wrap_keymap(keymap, per_key_config, behavior_config))
}

fn vial_morse(
    tap: HidKeyCode,
    hold: HidKeyCode,
    hold_after_tap: Option<HidKeyCode>,
    double_tap: Option<HidKeyCode>,
    profile: MorseProfile,
) -> Vec<Morse, 8> {
    let hat = hold_after_tap
        .map(|k| Action::Key(KeyCode::Hid(k)))
        .unwrap_or(Action::No);
    let dt = double_tap.map(|k| Action::Key(KeyCode::Hid(k))).unwrap_or(Action::No);
    Vec::from_slice(&[Morse::new_from_vial(
        Action::Key(KeyCode::Hid(tap)),
        Action::Key(KeyCode::Hid(hold)),
        hat,
        dt,
        profile,
    )])
    .unwrap()
}

/// Tap once, then press again within the quick-tap window and hold past the hold
/// timeout. The press should fire the tap action immediately (and stay pressed
/// while held) rather than triggering the hold action.
#[test]
fn quick_tap_fires_tap_on_held_second_press() {
    let morses = vial_morse(
        HidKeyCode::A,
        HidKeyCode::B,
        None,
        Some(HidKeyCode::D),
        MorseProfile::const_default(),
    );
    key_sequence_test! {
        keyboard: make_keyboard_with_morse(morses, default_profile(Some(200))),
        sequence: [
            [0, 0, true, 50],   // 1st press
            [0, 0, false, 50],  // 1st release (Released(TAP))
            [0, 0, true, 100],  // 2nd press within 200ms → quick-tap fires A
            [0, 0, false, 400], // hold past hold_timeout; release still produces A release
        ],
        expected_reports: [
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

/// When the second press lands AFTER both the quick-tap window AND the gap
/// timeout (so the morse pattern has been fully resolved as a tap), holding
/// the second press should resolve to the hold action (B) — quick-tap should
/// NOT kick in.
#[test]
fn quick_tap_disabled_outside_window() {
    let morses = vial_morse(
        HidKeyCode::A,
        HidKeyCode::B,
        None,
        Some(HidKeyCode::D),
        MorseProfile::const_default(),
    );
    key_sequence_test! {
        keyboard: make_keyboard_with_morse(morses, default_profile(Some(200))),
        sequence: [
            [0, 0, true, 50],   // 1st press
            [0, 0, false, 50],  // 1st release (Released(TAP))
            [0, 0, true, 300],  // 2nd press AFTER quick-tap (200ms) AND gap_timeout (250ms)
            [0, 0, false, 400], // hold past hold_timeout → fires hold action B
        ],
        expected_reports: [
            // 1st tap fires after gap_timeout elapses; then the 2nd press starts a
            // fresh morse sequence and holds → hold action B.
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
            [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

/// Mod-tap with quick_tap_timeout_ms: holding the 2nd press should send the tap key
/// (A) instead of the modifier (LShift).
#[test]
fn quick_tap_mod_tap_held_second_press() {
    let profile = default_profile(Some(180));
    let mt = KeyAction::TapHold(
        Action::Key(KeyCode::Hid(HidKeyCode::A)),
        Action::Modifier(rmk::types::modifier::ModifierCombination::LSHIFT),
        profile,
    );
    key_sequence_test! {
        keyboard: make_keyboard_with_keyaction(mt, default_profile(None)),
        sequence: [
            [0, 0, true, 50],
            [0, 0, false, 50],
            [0, 0, true, 80],   // within 180ms → quick-tap fires A
            [0, 0, false, 400],
        ],
        expected_reports: [
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // stash fire: A press
            [0, [0, 0, 0, 0, 0, 0]],             // stash fire: A release
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // quick-tap: A press (NOT LShift!)
            [0, [0, 0, 0, 0, 0, 0]],             // quick-tap release: A release
        ]
    };
}

/// Regression: with quick_tap_timeout_ms unset, hold behavior must not change.
/// Same physical sequence as `quick_tap_fires_tap_on_held_second_press` but
/// without quick-tap configured. With `hold_after_tap = C`, the held second
/// press must resolve to C (not the tap action A) — proving the quick-tap
/// branch was not taken.
#[test]
fn quick_tap_unset_does_not_change_hold_behavior() {
    let morses = vial_morse(
        HidKeyCode::A,
        HidKeyCode::B,
        Some(HidKeyCode::C),
        Some(HidKeyCode::D),
        MorseProfile::const_default(),
    );
    key_sequence_test! {
        keyboard: make_keyboard_with_morse(morses, default_profile(None)),
        sequence: [
            [0, 0, true, 50],
            [0, 0, false, 50],
            [0, 0, true, 100],  // feature off → normal morse path resolves to HOLD_AFTER_TAP
            [0, 0, false, 400],
        ],
        expected_reports: [
            [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn quick_tap_falls_back_to_global_default() {
    let per_morse_profile = default_profile(None);
    let morses = vial_morse(
        HidKeyCode::A,
        HidKeyCode::B,
        None,
        Some(HidKeyCode::D),
        per_morse_profile,
    );
    key_sequence_test! {
        keyboard: make_keyboard_with_morse(morses, default_profile(Some(200))),
        sequence: [
            [0, 0, true, 50],
            [0, 0, false, 50],
            [0, 0, true, 100],  // within global 200ms window → quick-tap fires A
            [0, 0, false, 400],
        ],
        expected_reports: [
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

#[test]
fn quick_tap_uses_per_morse_profile() {
    let per_morse_profile = default_profile(Some(150));
    let morses = vial_morse(
        HidKeyCode::A,
        HidKeyCode::B,
        None,
        Some(HidKeyCode::D),
        per_morse_profile,
    );
    key_sequence_test! {
        keyboard: make_keyboard_with_morse(morses, default_profile(None)),
        sequence: [
            [0, 0, true, 50],
            [0, 0, false, 50],
            [0, 0, true, 80],   // within per-morse window (150ms) → quick-tap fires A
            [0, 0, false, 400],
        ],
        expected_reports: [
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

/// Quick-tap from `EarlyFired` state: a morse with tap==hold_after_tap fires
/// the tap action early on first release (per `can_fire_early`). A re-press
/// within quick_tap_timeout_ms must still go through the quick-tap path.
#[test]
fn quick_tap_fires_from_early_fired_state() {
    let morses = vial_morse(
        HidKeyCode::Enter,
        HidKeyCode::B,
        Some(HidKeyCode::Enter),
        None,
        MorseProfile::const_default(),
    );
    key_sequence_test! {
        keyboard: make_keyboard_with_morse(morses, default_profile(Some(200))),
        sequence: [
            [0, 0, true, 50],   // press
            [0, 0, false, 50],  // release → Enter fires early; state = EarlyFired(TAP)
            [0, 0, true, 80],   // re-press within 200ms while EarlyFired → quick-tap fires Enter
            [0, 0, false, 400], // hold past hold_timeout; release Enter
        ],
        expected_reports: [
            [0, [kc_to_u8!(Enter), 0, 0, 0, 0, 0]], // early-fire
            [0, [0, 0, 0, 0, 0, 0]],                // released
            [0, [kc_to_u8!(Enter), 0, 0, 0, 0, 0]], // quick-tap fire on re-press
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

/// Defensive: when the cached pattern is `HOLD` (not all-taps), the quick-tap
/// path must be skipped even if the re-press lands inside the window. We
/// construct a morse with a `HOLD` continuation (`HOLD.followed_by_hold()`)
/// so the state lands in `Released(HOLD)` after the first hold release
/// instead of resolving and removing the key.
#[test]
fn quick_tap_skips_hold_pattern() {
    let mut morse = Morse::default();
    let _ = morse.put(
        MorsePattern::from_u16(0b10), // TAP
        Action::Key(KeyCode::Hid(HidKeyCode::A)),
    );
    let _ = morse.put(HOLD, Action::Key(KeyCode::Hid(HidKeyCode::B)));
    let _ = morse.put(HOLD.followed_by_hold(), Action::Key(KeyCode::Hid(HidKeyCode::C)));

    let morses: Vec<Morse, 8> = Vec::from_slice(&[morse]).unwrap();
    key_sequence_test! {
        keyboard: make_keyboard_with_morse(morses, default_profile(Some(200))),
        sequence: [
            [0, 0, true, 50],   // press
            [0, 0, false, 400], // held past hold_timeout, release → Released(HOLD)
            [0, 0, true, 100],  // re-press within 200ms BUT pattern=HOLD → no quick-tap
            [0, 0, false, 400], // hold further → resolves to HOLD.followed_by_hold() = C
        ],
        expected_reports: [
            [0, [kc_to_u8!(C), 0, 0, 0, 0, 0]],
            [0, [0, 0, 0, 0, 0, 0]],
        ]
    };
}

/// Morse with ONLY tap and hold (no double_tap, no hold_after_tap).
/// This goes through the stash_for_quick_tap path on first release
/// (try_predict_final_action returns Some because there are no longer
/// continuations), unlike the main test which has double_tap=D.
#[test]
fn quick_tap_tap_hold_only_fires_tap_on_held_second_press() {
    let morses = vial_morse(HidKeyCode::A, HidKeyCode::B, None, None, MorseProfile::const_default());
    key_sequence_test! {
        keyboard: make_keyboard_with_morse(morses, default_profile(Some(200))),
        sequence: [
            [0, 0, true, 50],   // 1st press
            [0, 0, false, 50],  // 1st release -> stash fires A tap (press+release)
            [0, 0, true, 100],  // 2nd press within 200ms -> quick-tap fires A press
            [0, 0, false, 400], // hold past hold_timeout; release produces A release
        ],
        expected_reports: [
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // stash fire: A press
            [0, [0, 0, 0, 0, 0, 0]],             // stash fire: A release
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // quick-tap: A press (NOT B!)
            [0, [0, 0, 0, 0, 0, 0]],             // quick-tap release: A release
        ]
    };
}

/// An intervening key event discards the stashed `EarlyFired` entry (same as a
/// pending hold_after_tap continuation), so a re-press that lands inside the
/// quick-tap window starts a fresh morse sequence and holding it resolves to
/// the hold action.
#[test]
fn quick_tap_stash_dropped_by_intervening_key() {
    let morses = vial_morse(HidKeyCode::A, HidKeyCode::B, None, None, MorseProfile::const_default());
    key_sequence_test! {
        keyboard: make_keyboard_with_morse(morses, default_profile(Some(200))),
        sequence: [
            [0, 0, true, 50],   // 1st press
            [0, 0, false, 50],  // 1st release -> stash fires A tap (press+release)
            [0, 1, true, 50],   // press E -> stashed EarlyFired entry is dropped
            [0, 1, false, 50],  // release E
            [0, 0, true, 80],   // re-press within 200ms BUT no stash -> fresh sequence
            [0, 0, false, 400], // hold past hold_timeout -> hold action B
        ],
        expected_reports: [
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // stash fire: A press
            [0, [0, 0, 0, 0, 0, 0]],             // stash fire: A release
            [0, [kc_to_u8!(E), 0, 0, 0, 0, 0]], // E press
            [0, [0, 0, 0, 0, 0, 0]],             // E release
            [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // fresh hold: B press (NOT quick-tap A!)
            [0, [0, 0, 0, 0, 0, 0]],             // B release
        ]
    };
}
