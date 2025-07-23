# Clean all examples
for dir in examples/use_rust/*/ examples/use_config/*/; do
    if [ -d "$dir" ] && [ -d "$dir/src" ]; then
        cd "$dir"
        cargo clean
        cd ../../..
    fi
done