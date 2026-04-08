#!/bin/bash
set -euo pipefail
# shellcheck source=_lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/_lib.sh"

export CARGO_NET_OFFLINE=false
# cargo test reuses the batch-built graph by writing into the same target dir.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$target_root/test}"
mkdir -p "$CARGO_TARGET_DIR"

require_cargo_batch
ensure_stable_toolchain
ensure_cargo_tool cargo-expand cargo-expand

# Feature-set matrix for rmk unit tests. Empty entry = --no-default-features only.
rmk_test_featuresets=(
    ""
    "log,std"
    "storage"
    "async_matrix,storage"
    "split,vial,storage"
    "passkey_entry"
    "split,vial,storage,passkey_entry"
)

log_section "Prebuilding test graph"
prebuild_args=(
    --- build --manifest-path rmk-config/Cargo.toml --tests
    --- build --manifest-path rmk-types/Cargo.toml --tests
    --- build --manifest-path rmk-macro/Cargo.toml --tests
)
for feats in "${rmk_test_featuresets[@]}"; do
    if [[ -z "$feats" ]]; then
        prebuild_args+=(--- build --manifest-path rmk/Cargo.toml --tests --no-default-features)
    else
        prebuild_args+=(--- build --manifest-path rmk/Cargo.toml --tests --no-default-features --features "$feats")
    fi
done
cargo +stable batch --target-dir "$CARGO_TARGET_DIR" "${prebuild_args[@]}"

log_section "Running tests"
cargo +stable test --manifest-path rmk-config/Cargo.toml --verbose
cargo +stable test --manifest-path rmk-types/Cargo.toml --verbose
cargo +stable test --manifest-path rmk-macro/Cargo.toml --verbose
for feats in "${rmk_test_featuresets[@]}"; do
    if [[ -z "$feats" ]]; then
        cargo +stable test --manifest-path rmk/Cargo.toml --no-default-features --verbose
    else
        cargo +stable test --manifest-path rmk/Cargo.toml --no-default-features --features "$feats" --verbose
    fi
done
