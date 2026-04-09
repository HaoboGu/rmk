#!/bin/bash
set -euo pipefail
# shellcheck source=_lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/_lib.sh"

require_cargo_batch
ensure_stable_toolchain
ensure_cargo_tool cargo-make cargo-make
ensure_cargo_tool flip-link flip-link

# Partition examples between the stable toolchain and the xtensa +esp
# toolchain (currently only esp32s3_ble). We build each example in its own
# cargo-batch invocation started from the example directory so that cargo
# loads the per-example .cargo/config.toml — it contains env vars like
# KEYBOARD_TOML_PATH that `#[rmk_keyboard]` relies on at macro-expansion
# time. Sharing --target-dir across invocations keeps dependency artifacts
# hot, so the loss vs one giant batch is modest.
stable_manifests=()
esp_manifests=()
while IFS= read -r manifest; do
    target="$(get_example_target "$manifest")"
    if [[ -z "$target" ]]; then
        echo "Skipping $manifest: no [build].target in .cargo/config.toml" >&2
        continue
    fi
    case "$manifest" in
        */esp32s3_ble/*)
            esp_manifests+=("$manifest")
            ;;
        *)
            stable_manifests+=("$manifest")
            ;;
    esac
done < <(list_example_manifests)

# $1=toolchain $2=target-dir $3..=example manifest paths
batch_examples() {
    local toolchain="$1"
    local target_dir="$2"
    shift 2
    local manifest target dir
    for manifest in "$@"; do
        target="$(get_example_target "$manifest")"
        dir="$(dirname "$manifest")"
        (
            cd "$dir"
            cargo "+$toolchain" batch --target-dir "$target_dir" \
                --- build --release --target "$target"
        )
    done
}

log_section "Building stable-toolchain examples"
batch_examples stable "$target_root/build" "${stable_manifests[@]}"

if (( ${#esp_manifests[@]} > 0 )); then
    log_section "Building esp32s3 examples"
    ensure_esp_toolchain
    # shellcheck source=/dev/null
    source "$HOME/export-esp.sh"
    # xtensa-esp32s3-none-elf has no prebuilt sysroot; rely on build-std
    # instead of the per-example [unstable].build-std that we don't inherit.
    CARGO_UNSTABLE_BUILD_STD=alloc,core \
        batch_examples esp "$target_root/build-esp" "${esp_manifests[@]}"
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
