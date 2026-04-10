#!/bin/bash
# Build ESP examples that need the xtensa toolchain.
# Stable-toolchain examples are built via the matrix job in ci.yml.
set -euo pipefail
# shellcheck source=_lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/_lib.sh"

ensure_stable_toolchain
ensure_cargo_tool flip-link flip-link

esp_manifests=()
while IFS= read -r manifest; do
    case "$manifest" in
        */esp32s3_ble/*) esp_manifests+=("$manifest") ;;
    esac
done < <(list_example_manifests)

if (( ${#esp_manifests[@]} > 0 )); then
    log_section "Building esp32s3 examples"
    ensure_esp_toolchain
    # shellcheck source=/dev/null
    source "$HOME/export-esp.sh"
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
