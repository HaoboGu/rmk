#!/bin/bash

# Build and clean examples under examples/use_rust
for dir in examples/use_rust/*/; do
    if [ -d "$dir" ] && [ -d "$dir/src" ]; then
        cd "$dir"
        # Handle ESP projects
        if [[ "$dir" == *"esp32s3"* ]]; then
            cargo +esp build --release
        else
            cargo update && cargo build --release
        fi
        cd ../../..
    fi
done

# Build and clean examples under examples/use_config
for dir in examples/use_config/*/; do
    if [ -d "$dir" ] && [ -d "$dir/src" ]; then
        cd "$dir"
        # Handle ESP projects
        if [[ "$dir" == *"esp32s3"* ]]; then
            cargo +esp build --release
        else
            cargo update && cargo build --release
        fi
        cd ../../..
    fi
done

# Clean all examples
for dir in examples/use_rust/*/ examples/use_config/*/; do
    if [ -d "$dir" ] && [ -d "$dir/src" ]; then
        cd "$dir"
        cargo clean
        cd ../../..
    fi
done