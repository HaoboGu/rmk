#!/bin/bash
set -euo pipefail
# shellcheck source=_lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/_lib.sh"

export CARGO_NET_OFFLINE=false
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$target_root/test}"
mkdir -p "$CARGO_TARGET_DIR"

# Each crate is its own cargo workspace in this repo, so nextest's default
# `<workspace>/.config/nextest.toml` lookup would miss our shared config at
# repo root. Pass it explicitly.
nextest_cfg="$repo_root/.config/nextest.toml"
nx=(nextest run --config-file "$nextest_cfg" --profile ci)

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
cargo +stable "${nx[@]}" --manifest-path rmk-config/Cargo.toml
cargo +stable "${nx[@]}" --manifest-path rmk-types/Cargo.toml
# Exercise the rmk_protocol module (gated behind `rmk_protocol`) so the wire-format
# snapshot tests under rmk-types/src/protocol/rmk/snapshots/ run in CI. `host`
# enables rmk_protocol + bulk + _ble + split, covering every snapshot.
cargo +stable "${nx[@]}" --manifest-path rmk-types/Cargo.toml --features host
cargo +stable "${nx[@]}" --manifest-path rmk-macro/Cargo.toml
for feats in "${rmk_test_featuresets[@]}"; do
    if [[ -z "$feats" ]]; then
        cargo +stable "${nx[@]}" --manifest-path rmk/Cargo.toml --no-default-features
    else
        cargo +stable "${nx[@]}" --manifest-path rmk/Cargo.toml --no-default-features --features "$feats"
    fi
done

# Doctests: nextest does not run them. rmk/ and rmk-macro/ have `doctest = false`,
# so only rmk-types and rmk-config need a separate --doc pass.
log_section "Running doctests"
cargo +stable test --manifest-path rmk-types/Cargo.toml --doc
cargo +stable test --manifest-path rmk-types/Cargo.toml --features host --doc
cargo +stable test --manifest-path rmk-config/Cargo.toml --doc
