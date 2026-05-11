#!/bin/bash

# Exit on any error
set -e

# Format rmk and rmk-macro
cd rmk && cargo +nightly fmt && cd ..
cd rmk-macro && cargo +nightly fmt && cd ..
cd rmk-config && cargo +nightly fmt && cd ..
cd rmk-types && cargo +nightly fmt && cd ..
cd rmk-host-tool && cargo +nightly fmt --all && cd ..

# Format all directories under examples/use_rust
for dir in examples/use_rust/*/; do
    if [ -d "$dir" ] && [ -d "$dir/src" ]; then
        cd "$dir"
        cargo +nightly fmt
        cd ../../..
    fi
done

# Format all directories under examples/use_config
for dir in examples/use_config/*/; do
    if [ -d "$dir" ] && [ -d "$dir/src" ]; then
        cd "$dir"
        cargo +nightly fmt
        cd ../../..
    fi
done