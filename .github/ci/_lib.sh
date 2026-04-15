# shellcheck shell=bash
#
# Shared bootstrap for RMK CI scripts. Source this from other scripts in
# .github/ci/ to pick up common env, cache, and example discovery helpers.
#
#     source "$(dirname "${BASH_SOURCE[0]}")/_lib.sh"
#
# Expected preamble in the caller:
#
#     #!/bin/bash
#     set -euo pipefail
#
# Toolchain + tool installation (rustup components/targets, cargo-batch,
# cargo-expand, flip-link, espup) is the workflow's responsibility and lives
# in .github/workflows/ci.yml. Locally the repo's rust-toolchain.toml takes
# care of it, so these scripts stay side-effect-free on your machine.

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

export CARGO_TERM_COLOR=always
export CARGO_TERM_PROGRESS_WHEN=never
export CARGO_NET_GIT_FETCH_WITH_CLI=true
export TERM="${TERM:-dumb}"

# Cache root: if RMK_CI_CACHE_ROOT is set (self-hosted runner or local reuse),
# redirect the target dir there. Otherwise fall back to an in-tree target/ci
# dir so local runs don't clobber the default target.
if [[ -n "${RMK_CI_CACHE_ROOT:-}" ]]; then
    mkdir -p "$RMK_CI_CACHE_ROOT"
    target_root="$RMK_CI_CACHE_ROOT/target"
else
    target_root="$repo_root/target/ci"
fi
mkdir -p "$target_root"

log_section() {
    printf "\n==> %s\n" "$1"
}

# Examples auto-discovery currently skips over:
#   - nrf54lm20_ble: Cargo.toml references local path deps
#     (/Users/.../nrf-sdc, /Users/.../nrf-mpsl) that only exist on the author's
#     workstation. The git-rev alternatives sit commented-out next to them;
#     once those are swapped in, this entry can be removed.
#   - esp32_ble_split: dual-target split example that only builds through the
#     `build-central` / `build-peripheral` cargo aliases (different chip per
#     bin, no [build].target set). Not buildable via a plain `cargo build`.
EXAMPLE_SKIPLIST=(
    "examples/use_rust/nrf54lm20_ble"
    "examples/use_config/esp32_ble_split"
)

# Echoes Cargo.toml paths for every buildable example, one per line.
# A buildable example is a direct child of examples/use_{rust,config}/ that
# has both a src/ dir and a Cargo.toml (filters out placeholders like fix/),
# and is not listed in EXAMPLE_SKIPLIST.
list_example_manifests() {
    local dir stripped skip entry
    for dir in examples/use_rust/*/ examples/use_config/*/; do
        [[ -d "$dir/src" && -f "$dir/Cargo.toml" ]] || continue
        stripped="${dir%/}"
        skip=0
        for entry in "${EXAMPLE_SKIPLIST[@]}"; do
            if [[ "$stripped" == "$entry" ]]; then
                skip=1
                break
            fi
        done
        (( skip == 0 )) && printf '%s\n' "${dir}Cargo.toml"
    done
}

# Echoes the default build target triple declared in the manifest's sibling
# .cargo/config.toml ([build].target). Only the first uncommented occurrence
# is emitted; returns empty if the file or the key is absent. Trailing
# TOML comments on the value are stripped.
get_example_target() {
    local manifest="$1"
    local dir config
    dir="$(dirname "$manifest")"
    config="$dir/.cargo/config.toml"
    [[ -f "$config" ]] || return 0
    awk '
        /^\[/ { section = $0; next }
        section == "[build]" && /^[[:space:]]*target[[:space:]]*=/ {
            sub(/^[[:space:]]*target[[:space:]]*=[[:space:]]*/, "")
            sub(/[[:space:]]*#.*$/, "")
            sub(/^"/, "")
            sub(/"[[:space:]]*$/, "")
            print
            exit
        }
    ' "$config"
}
