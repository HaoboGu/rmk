#!/bin/bash
set -euo pipefail
# shellcheck source=_lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/_lib.sh"

export CARGO_NET_OFFLINE=false
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$target_root/test}"
mkdir -p "$CARGO_TARGET_DIR"

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

log_section "Running tests"
cargo +stable test --manifest-path rmk-config/Cargo.toml
cargo +stable test --manifest-path rmk-types/Cargo.toml
# Exercise the rmk_protocol module (gated behind `rmk_protocol`) so the wire-format
# snapshot tests under rmk-types/src/protocol/rmk/snapshots/ run in CI. `host`
# enables rmk_protocol + bulk + _ble + split, covering every snapshot.
cargo +stable test --manifest-path rmk-types/Cargo.toml --features host
cargo +stable test --manifest-path rmk-macro/Cargo.toml
for feats in "${rmk_test_featuresets[@]}"; do
    if [[ -z "$feats" ]]; then
        cargo +stable test --manifest-path rmk/Cargo.toml --no-default-features
    else
        cargo +stable test --manifest-path rmk/Cargo.toml --no-default-features --features "$feats"
    fi
done
