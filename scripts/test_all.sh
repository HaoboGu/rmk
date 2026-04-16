#!/bin/bash

# Exit on any error
set -e

# rmk-types: default-features run + host-feature run (the latter enables
# rmk_protocol/bulk/_ble/split, which is required to compile the wire-format
# snapshot tests under src/protocol/rmk/snapshots/).
cargo test --manifest-path rmk-types/Cargo.toml
cargo test --manifest-path rmk-types/Cargo.toml --features host

cd rmk
cargo test --no-default-features --features "split,vial,async_matrix,_ble"
cargo test --no-default-features --features "split,vial,async_matrix"
cargo test --no-default-features --features "split,async_matrix"
cargo test --no-default-features --features "split,async_matrix,_ble"
cargo test --no-default-features --features "async_matrix,storage"
cargo test --no-default-features --features "vial,storage"
cargo test --no-default-features --features "vial,_ble"
cargo test --no-default-features --features "passkey_entry"
cargo test --no-default-features --features "split,vial,storage,passkey_entry"
cargo test --no-default-features