#!/usr/bin/env sh

set -eu

repo_root="$(cd "$(dirname "$0")/.." && pwd)"

forbidden='(^|[^A-Za-z0-9_])Keyboard::new\(|KeyMap::new\(|wrap_keymap\('
forbidden="$forbidden|initialize_keymap_and_storage\("
forbidden="$forbidden|run_key_sequence_test\(|key_sequence_test!|sim_keyboard_""test!"
forbidden="$forbidden|run_keyboard_test\(|run_sim_keyboard_sequence\("
forbidden="$forbidden|USB_REPORT_CHANNEL|HOST_REQUEST_CHANNEL|FLASH_CHANNEL"

if rg -n -e "$forbidden" "$repo_root/rmk/tests" -g '*.rs'; then
    echo "rmk/tests must use the SimKeyboard end-to-end API for keyboard/input/protocol scenarios." >&2
    exit 1
fi
