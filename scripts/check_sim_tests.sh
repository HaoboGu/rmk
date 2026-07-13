#!/usr/bin/env sh

set -eu

repo_root="$(cd "$(dirname "$0")/.." && pwd)"

forbidden='(^|[^A-Za-z0-9_])Keyboard::new\(|wrap_keymap\(|run_key_sequence_test\(|key_sequence_test!'
forbidden="$forbidden|sim_keyboard_""test!"
forbidden="$forbidden|run_keyboard_test\(|run_sim_keyboard_sequence\(|create_[A-Za-z0-9_]*keyboard\("
forbidden="$forbidden|fn [A-Za-z0-9_]*keyboard\(|fn keyboard_[A-Za-z0-9_]*\("
forbidden="$forbidden|from_keymap_with_config_refs\(|from_initialized_keymap\("
forbidden="$forbidden|run_sequence\(|run_sequence_with_timeout\(|SimKeyEvent|leaked_keymap"
forbidden="$forbidden|run_with\(|publish_key_events\(|assert_no_report_for\("
forbidden="$forbidden|TestKeyboard|TestKeyPress|get_keymap\(|morse_behavior_config|get_combos_config"
forbidden="$forbidden|fn [A-Za-z0-9_]*_keymap\("
forbidden="$forbidden|hold_on_other_key_press_behavior|permissive_hold_behavior|hrm_behavior"
forbidden="$forbidden|bilateral_behavior|tap_dance_behavior|early_fire_behavior|flow_tap_early_fire_behavior"
forbidden="$forbidden|normal_unilateral_behavior|release_remap_behavior"
forbidden="$forbidden|USB_REPORT_CHANNEL|HOST_REQUEST_CHANNEL"
forbidden="$forbidden|initialize_keymap_and_storage\("
forbidden="$forbidden|common::report|kc_to_u8"
forbidden="$forbidden|SimHost::create|SimHost::new|expect_empty\(|expect_mods\("

if rg -n -e "$forbidden" "$repo_root/rmk/tests" -g '*.rs'; then
    echo "rmk/tests must use the SimKeyboard end-to-end API for keyboard/input/protocol scenarios." >&2
    exit 1
fi

if rg -n -e '(^| )pub const [A-Z0-9_]*KEYMAP:|(^| )const [A-Z0-9_]*KEYMAP:' "$repo_root/rmk/tests" -g '*.rs' \
    | rg -v 'rmk/tests/common/mod.rs:.*TEST_KEYMAP'; then
    echo "rmk/tests should share TEST_KEYMAP and use KeymapOverride for scenario-specific keys." >&2
    exit 1
fi
