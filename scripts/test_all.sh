#!/bin/bash

# Exit on any error
set -e

cd rmk 
cargo test --no-default-features --features "split,vial,async_matrix,_ble"
cargo test --no-default-features --features "split,vial,async_matrix"
cargo test --no-default-features --features "split,async_matrix"
cargo test --no-default-features --features "split,async_matrix,_ble"
cargo test --no-default-features --features "async_matrix,storage"
cargo test --no-default-features --features "vial,storage"
cargo test --no-default-features --features "vial,_ble"
cargo test --no-default-features