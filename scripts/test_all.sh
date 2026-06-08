#!/bin/bash

# Exit on any error
set -e

command -v cargo-nextest >/dev/null 2>&1 || {
    echo "cargo-nextest not found. Install with: cargo install cargo-nextest --locked"
    exit 1
}

# Each crate in this repo is its own cargo workspace, so nextest's default
# `<workspace>/.config/nextest.toml` lookup would miss our shared config at
# repo root. Pass it explicitly.
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
nx=(nextest run --config-file "$repo_root/.config/nextest.toml")

# rmk-types: default-features run + host-feature run (the latter enables
# rmk_protocol/bulk/_ble/split, which is required to compile the wire-format
# snapshot tests under src/protocol/rmk/snapshots/).
cargo "${nx[@]}" --manifest-path rmk-types/Cargo.toml
cargo "${nx[@]}" --manifest-path rmk-types/Cargo.toml --features host
cargo "${nx[@]}" --manifest-path rmk-types/Cargo.toml --features steno

cargo "${nx[@]}" --manifest-path rmk/Cargo.toml --no-default-features --features "split,vial,async_matrix,_ble"
cargo "${nx[@]}" --manifest-path rmk/Cargo.toml --no-default-features --features "split,vial,async_matrix"
cargo "${nx[@]}" --manifest-path rmk/Cargo.toml --no-default-features --features "split,async_matrix"
cargo "${nx[@]}" --manifest-path rmk/Cargo.toml --no-default-features --features "split,async_matrix,_ble"
cargo "${nx[@]}" --manifest-path rmk/Cargo.toml --no-default-features --features "async_matrix,storage"
cargo "${nx[@]}" --manifest-path rmk/Cargo.toml --no-default-features --features "vial,storage"
cargo "${nx[@]}" --manifest-path rmk/Cargo.toml --no-default-features --features "vial,_ble"
cargo "${nx[@]}" --manifest-path rmk/Cargo.toml --no-default-features --features "passkey_entry"
cargo "${nx[@]}" --manifest-path rmk/Cargo.toml --no-default-features --features "split,vial,storage,passkey_entry"
# Steno (Plover HID): USB-only path runs the chord/descriptor unit tests;
# the _ble combo verifies the BLE silent-drop arm compiles.
cargo "${nx[@]}" --manifest-path rmk/Cargo.toml --no-default-features --features "vial,storage,steno"
cargo "${nx[@]}" --manifest-path rmk/Cargo.toml --no-default-features --features "split,vial,storage,async_matrix,_ble,steno"
# Rynk: runs the `rynk_loopback` end-to-end tests, which drive the real
# `run_session` over an in-memory embedded-io duplex (request/response Cmds,
# topic-emit, and oversized-frame resync). The feature flags also add *compile*
# coverage for the gated handler arms: `bulk` pulls in every bulk Cmd
# handler, `_ble` the BLE-only handlers, and `_ble` + `split` the peripheral-status
# handler — those arms are compiled here, not all exercised at runtime.
cargo "${nx[@]}" --manifest-path rmk/Cargo.toml --no-default-features --features "rynk,bulk,_ble,split,storage,async_matrix"
# Rynk again with the BLE/split/bulk handlers OFF: exercises the gated arms' absence
# and the `GetCapabilities` feature flags in their false state (so the cfg!()-based
# assertions aren't tautological), plus the no-`_ble`/no-`split` topic set.
cargo "${nx[@]}" --manifest-path rmk/Cargo.toml --no-default-features --features "rynk,storage"
cargo "${nx[@]}" --manifest-path rmk/Cargo.toml --no-default-features

# Doctests: nextest doesn't run them. rmk/ has `doctest = false` so only
# rmk-types needs a --doc pass.
cargo test --manifest-path rmk-types/Cargo.toml --doc
cargo test --manifest-path rmk-types/Cargo.toml --features host --doc

# Host tooling is a separate workspace; build it + run rmk-host's tests and the
# cross-stack rynk end-to-end test (real Client against the real run_session).
# (.github/ci/host.sh also runs the wasm32 check and clippy.)
(cd "$repo_root/rmk-host-tool" && cargo build --workspace --all-targets && cargo test -p rmk-host && cargo test -p rmk-rynk-e2e)
