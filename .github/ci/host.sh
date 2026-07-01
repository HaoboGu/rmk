#!/bin/bash
set -euo pipefail
# shellcheck source=_lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/_lib.sh"

# The host tooling is its own cargo workspace.
cd "$repo_root/rynk"

export CARGO_TERM_COLOR=always
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$target_root/host}"
mkdir -p "$CARGO_TARGET_DIR"

log_section "Tests"
cargo +stable test --workspace --lib --tests

log_section "Doctests"
cargo +stable test -p rynk --doc

log_section "Wasm smoke check"
cargo +stable check -p rynk --lib --target wasm32-unknown-unknown
cargo +stable check -p rynk-wasm --target wasm32-unknown-unknown

log_section "TS type contract"
# bindings/rynk.d.ts is the checked-in Rust↔TS contract; regenerating it must be
# a no-op, else the committed copy is stale.
rynk-wasm/scripts/gen-types.sh >/dev/null
git diff --exit-code -- rynk-wasm/bindings/rynk.d.ts || {
    echo "::error::rynk-wasm/bindings/rynk.d.ts is out of date — run rynk/rynk-wasm/scripts/gen-types.sh and commit"
    exit 1
}

log_section "Clippy"
cargo +stable clippy --workspace --lib --tests --examples -- -D warnings
cargo +stable clippy -p rynk-wasm --target wasm32-unknown-unknown -- -D warnings
