#!/bin/bash

# Exit on any error
set -e

# Format rmk and rmk-macro
cd rmk && cargo clippy --fix --allow-dirty && cd ..
cd rmk-macro && cargo clippy --fix --allow-dirty&& cd ..
cd rmk-config && cargo clippy --fix --allow-dirty&& cd ..
cd rmk-types && cargo clippy --fix --allow-dirty&& cd ..

# Format all directories under examples/use_rust
for dir in examples/use_rust/*/; do
    if [ -d "$dir" ] && [ -d "$dir/src" ]; then
        cd "$dir"
        # Check if it's an ESP project
        if [[ "$dir" == *"esp"* ]]; then
            cargo clippy --fix --allow-dirty
        else
            cargo clippy --fix --allow-dirty
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
            cargo clippy --fix --allow-dirty
        else
            cargo clippy --fix --allow-dirty
        fi
        cd ../../..
    fi
done