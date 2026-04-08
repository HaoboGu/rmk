#!/bin/bash
set -euo pipefail
# shellcheck source=_lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/_lib.sh"

require_cargo_batch
ensure_stable_toolchain
ensure_cargo_tool cargo-make cargo-make
ensure_cargo_tool flip-link flip-link

# Split discovered examples into those that build on the stable toolchain and
# those that require the xtensa +esp toolchain (currently only esp32s3_ble).
stable_args=()
esp_args=()
while IFS= read -r manifest; do
    case "$manifest" in
        */esp32s3_ble/*)
            esp_args+=(--- build --manifest-path "$manifest" --release)
            ;;
        *)
            stable_args+=(--- build --manifest-path "$manifest" --release)
            ;;
    esac
done < <(list_example_manifests)

log_section "Building stable-toolchain examples"
cargo +stable batch --target-dir "$target_root/build" "${stable_args[@]}"

if (( ${#esp_args[@]} > 0 )); then
    log_section "Building esp32s3 examples"
    ensure_esp_toolchain
    # shellcheck source=/dev/null
    source "$HOME/export-esp.sh"
    cargo +esp batch --target-dir "$target_root/build-esp" "${esp_args[@]}"
fi

log_section "UF2 smoke"
for dir in \
    examples/use_rust/nrf52840_ble \
    examples/use_rust/rp2040 \
    examples/use_config/nrf52840_ble_split
do
    (
        cd "$dir"
        cargo +stable make uf2 --release
    )
done
