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

log_section "Clippy"
cargo +stable clippy --workspace --lib --tests --examples -- -D warnings
