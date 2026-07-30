#!/usr/bin/env bash
#
# Keyboard/input/protocol cases must go through the SimKeyboard end-to-end API,
# not the primitives it is built from.

set -eu

repo_root="$(cd "$(dirname "$0")/.." && pwd)"

forbidden='(^|[^A-Za-z0-9_])(Keyboard|KeyMap)::new\(|initialize_keymap_and_storage\('
forbidden="$forbidden|USB_REPORT_CHANNEL|BLE_REPORT_CHANNEL|FLASH_CHANNEL"

# The simulator harness implements this API, so it may touch the raw primitives.
if rg -n -e "$forbidden" "$repo_root/rmk/tests" -g '*.rs' -g '!**/simulator/**'; then
    echo "rmk/tests must use the SimKeyboard end-to-end API for keyboard/input/protocol scenarios." >&2
    exit 1
fi
