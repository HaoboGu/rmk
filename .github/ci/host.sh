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

log_section "Tests"
cargo +stable test -p rmk-host

log_section "Clippy"
cargo +stable clippy -p rmk-host --all-targets -- -D warnings
