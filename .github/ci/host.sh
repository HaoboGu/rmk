#!/bin/bash
set -euo pipefail
# shellcheck source=_lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/_lib.sh"

# The host tooling is its own cargo workspace.
cd "$repo_root/rmk-host-tool"

export CARGO_TERM_COLOR=always
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$target_root/host}"
mkdir -p "$CARGO_TARGET_DIR"

log_section "Core check"
cargo +stable check -p rmk-host
cargo +stable check -p rmk-host --target wasm32-unknown-unknown

log_section "Native transport checks"
cargo +stable check -p rmk-host-serial
cargo +stable check -p rmk-host-ble

log_section "Tests"
cargo +stable test -p rmk-host
cargo +stable test -p rmk-host-serial
# Cross-stack rynk end-to-end test: the real host Client against the real
# firmware run_session (links rmk for the host target — see rmk-rynk-e2e).
cargo +stable test -p rmk-rynk-e2e

log_section "Clippy"
cargo +stable clippy --workspace --all-targets -- -D warnings
