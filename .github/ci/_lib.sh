# shellcheck shell=bash
#
# Shared bootstrap for RMK CI scripts. Source this from other scripts in
# .github/ci/ to pick up common env and example discovery helpers.
#
#     source "$(dirname "${BASH_SOURCE[0]}")/_lib.sh"
#
# Expected preamble in the caller:
#
#     #!/bin/bash
#     set -euo pipefail
#
# Toolchain + tool installation (rustup components/targets, cargo-batch,
# cargo-expand, espup) is the workflow's responsibility and lives in
# .github/workflows/ci.yml. Locally the repo's rust-toolchain.toml takes
# care of it, so these scripts stay side-effect-free on your machine.

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

export CARGO_TERM_COLOR=always
export CARGO_TERM_PROGRESS_WHEN=never
export CARGO_NET_GIT_FETCH_WITH_CLI=true
export TERM="${TERM:-dumb}"

# Shared target dir for scripts that run cargo. Callers that need it should
# `mkdir -p "$target_root"` before use; we don't create it here so scripts
# that don't run cargo (e.g. discover.sh) don't leave an empty directory.
target_root="$repo_root/target/ci"

log_section() {
    printf "\n==> %s\n" "$1"
}

# Feature-set matrix for rmk check/clippy/test. An empty entry means
# `--no-default-features` with no extra features on top. Kept here so
# check.sh and test.sh stay in lockstep — a set added for check is also
# exercised by tests, and vice versa.
RMK_FEATURESETS=(
    ""
    "log,std"
    "storage"
    "async_matrix,storage"
    "split,vial,storage"
    "passkey_entry"
    "split,vial,storage,passkey_entry"
)

# Examples auto-discovery skiplist. Reasons:
#   - nrf54lm20_ble: Cargo.toml references local path deps that only exist on
#     the author's workstation.
#   - esp32_ble_split: dual-target split example; only builds through the
#     `build-central` / `build-peripheral` cargo aliases.
#   - py32f07x, sf32lb52x_usb: not currently buildable in CI.
EXAMPLE_SKIPLIST=(
    "examples/use_rust/nrf54lm20_ble"
    "examples/use_config/esp32_ble_split"
    "examples/use_rust/py32f07x"
    "examples/use_rust/sf32lb52x_usb"
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
