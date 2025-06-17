#!/bin/bash

# Exit on any error
set -e

# Format rmk and rmk-macro
cd rmk && cargo +nightly fmt && cd ..
cd rmk-macro && cargo +nightly fmt && cd ..

# Format all directories under examples/use_rust
for dir in examples/use_rust/*/; do
    if [ -d "$dir" ] && [ -d "$dir/src" ]; then
        cd "$dir"
        # Check if it's an ESP project
        if [[ "$dir" == *"esp"* ]]; then
            cargo +esp fmt
        else
            cargo +nightly fmt
        fi
        cd ../../..
    fi
done

# Format all directories under examples/use_config
for dir in examples/use_config/*/; do
    if [ -d "$dir" ] && [ -d "$dir/src" ]; then
        cd "$dir"
        # Check if it's an ESP project
        if [[ "$dir" == *"esp"* ]]; then
            cargo +esp fmt
        else
            cargo +nightly fmt
        fi
        cd ../../..
    fi
done