#!/bin/bash
# Build ESP examples that need the xtensa toolchain.
# Stable-toolchain examples are built via the matrix job in ci.yml.
#
# The workflow installs the esp toolchain (via espup) before invoking this
# script; $HOME/export-esp.sh is expected to exist.
set -euo pipefail
# shellcheck source=_lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/_lib.sh"

esp_manifests=()
while IFS= read -r manifest; do
    case "$manifest" in
        */esp32s3_ble/*) esp_manifests+=("$manifest") ;;
    esac
done < <(list_example_manifests)

if (( ${#esp_manifests[@]} > 0 )); then
    log_section "Building esp32s3 examples"
    # shellcheck source=/dev/null
    source "$HOME/export-esp.sh"
    # Share a single target dir across all esp examples. The examples have
    # identical dependency trees (esp-hal, esp-radio, embassy, rmk), so the
    # second build reuses ~300 dep crates and the Xtensa sysroot from the
    # first instead of rebuilding from scratch.
    repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
    export CARGO_TARGET_DIR="$repo_root/target-esp"
    for manifest in "${esp_manifests[@]}"; do
        target="$(get_example_target "$manifest")"
        dir="$(dirname "$manifest")"
        (
            cd "$dir"
            CARGO_UNSTABLE_BUILD_STD=alloc,core \
                cargo +esp build --release --target "$target"
        )
    done
fi
