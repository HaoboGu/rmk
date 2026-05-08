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
# rmk_protocol matrix: USB-only and split+BLE combos. The std build also runs
# the rmk_protocol_loopback integration test and the COBS reframer unit tests
# in src/host/rmk_protocol/wire_ble.rs.
cargo "${nx[@]}" --manifest-path rmk/Cargo.toml --no-default-features --features "rmk_protocol,bulk_transfer,storage"
cargo "${nx[@]}" --manifest-path rmk/Cargo.toml --no-default-features --features "rmk_protocol,bulk_transfer,storage,_ble,split,async_matrix"
cargo "${nx[@]}" --manifest-path rmk/Cargo.toml --no-default-features

# Doctests: nextest doesn't run them. rmk/ has `doctest = false` so only
# rmk-types needs a --doc pass.
cargo test --manifest-path rmk-types/Cargo.toml --doc
cargo test --manifest-path rmk-types/Cargo.toml --features host --doc
