#!/bin/bash
set -euo pipefail
# shellcheck source=_lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/_lib.sh"

require_cargo_batch
ensure_stable_toolchain
ensure_cargo_tool cargo-make cargo-make
ensure_cargo_tool flip-link flip-link
ensure_cargo_tool cargo-size cargo-binutils

# Fixed comparison surface: bloat cares about a stable set of representative
# examples, not breadth. Add/remove entries here when coverage shifts.
bloat_manifests=(
    examples/use_config/nrf52840_ble/Cargo.toml
    examples/use_config/nrf52840_ble_split/Cargo.toml
    examples/use_config/rp2040/Cargo.toml
    examples/use_config/pi_pico_w_ble/Cargo.toml
    examples/use_config/stm32h7/Cargo.toml
)

# Binary names for multi-bin examples (flat manifest path => bin list).
split_binaries=(central peripheral)

build_bloat_set() {
    local workspace_root="$1"
    local target_dir="$2"
    local args=()
    local manifest target
    for manifest in "${bloat_manifests[@]}"; do
        # cargo-batch runs from workspace_root and does not walk into each
        # example's .cargo/config.toml, so hand it the target triple directly.
        target="$(cd "$workspace_root" && get_example_target "$manifest")"
        if [[ -z "$target" ]]; then
            echo "bloat: skipping $manifest (no [build].target)" >&2
            continue
        fi
        args+=(--- build --manifest-path "$manifest" --release --target "$target")
    done
    (
        cd "$workspace_root"
        cargo +stable batch --target-dir "$target_dir" "${args[@]}"
    )
}

# $1=workspace_root $2=target_dir $3=manifest [$4=bin_name]
size_value() {
    local workspace_root="$1"
    local target_dir="$2"
    local manifest="$3"
    local bin_name="${4:-}"
    local output target

    target="$(cd "$workspace_root" && get_example_target "$manifest")"
    if [[ -z "$target" ]]; then
        echo "bloat: no [build].target for $manifest, cannot size" >&2
        return 1
    fi

    if [[ -n "$bin_name" ]]; then
        output="$(
            cd "$workspace_root"
            CARGO_TARGET_DIR="$target_dir" \
                cargo +stable size --manifest-path "$manifest" --release --target "$target" --bin "$bin_name"
        )"
    else
        output="$(
            cd "$workspace_root"
            CARGO_TARGET_DIR="$target_dir" \
                cargo +stable size --manifest-path "$manifest" --release --target "$target"
        )"
    fi

    printf '%s\n' "$output" | awk 'NR==2 {print $4}'
}

# Globals populated during run() and torn down by cleanup().
worktree_root=""
base_checkout=""

cleanup() {
    if [[ -n "$base_checkout" ]]; then
        git worktree remove --force "$base_checkout" >/dev/null 2>&1 || true
    fi
    if [[ -n "$worktree_root" ]]; then
        rm -rf "$worktree_root"
    fi
}
trap cleanup EXIT

base_sha="${RMK_CI_BASE_SHA:-}"
if [[ -z "$base_sha" ]]; then
    if git rev-parse --verify origin/main >/dev/null 2>&1; then
        base_sha="$(git merge-base HEAD origin/main)"
    elif git rev-parse --verify HEAD^ >/dev/null 2>&1; then
        base_sha="$(git rev-parse HEAD^)"
    else
        echo "No base revision available for bloat report" >&2
        exit 0
    fi
fi

worktree_root="$(mktemp -d "${TMPDIR:-/tmp}/rmk-bloat.XXXXXX")"
base_checkout="$worktree_root/base"
git worktree add --detach "$base_checkout" "$base_sha" >/dev/null

head_target="$target_root/bloat-head"
base_target="$target_root/bloat-base"

log_section "Building bloat head"
build_bloat_set "$repo_root" "$head_target"

log_section "Building bloat base"
build_bloat_set "$base_checkout" "$base_target"

log_section "Bloat report"
report_path="${RMK_CI_BLOAT_REPORT:-$worktree_root/bloat-report.md}"
{
    echo "# RMK size report"
    echo
    echo "Base revision: \`$base_sha\`"
    echo
    echo "| Target | Base | Head | Delta |"
    echo "| --- | ---: | ---: | ---: |"

    for manifest in "${bloat_manifests[@]}"; do
        # nrf52840_ble_split is multi-bin; its per-binary numbers are reported
        # separately below, so skip the single-bin row here.
        if [[ "$manifest" == *nrf52840_ble_split* ]]; then
            continue
        fi
        base_size="$(size_value "$base_checkout" "$base_target" "$manifest")"
        head_size="$(size_value "$repo_root"    "$head_target" "$manifest")"
        delta="$((head_size - base_size))"
        printf "| %s | %s | %s | %+d |\n" "$manifest" "$base_size" "$head_size" "$delta"
    done

    for bin_name in "${split_binaries[@]}"; do
        base_size="$(size_value "$base_checkout" "$base_target" examples/use_config/nrf52840_ble_split/Cargo.toml "$bin_name")"
        head_size="$(size_value "$repo_root"    "$head_target" examples/use_config/nrf52840_ble_split/Cargo.toml "$bin_name")"
        delta="$((head_size - base_size))"
        printf "| examples/use_config/nrf52840_ble_split/Cargo.toml [%s] | %s | %s | %+d |\n" \
            "$bin_name" "$base_size" "$head_size" "$delta"
    done
} | tee "$report_path"
