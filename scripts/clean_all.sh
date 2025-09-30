# Clean build caches
for dir in examples/use_rust/*/ examples/use_config/*/; do
    if [ -d "$dir" ] && [ -d "$dir/src" ]; then
        cd "$dir"
        echo "Cleaning $dir"
        cargo clean
        cd ../../..
    fi
done

for dir in rmk/ rmk-config/ rmk-macro/ rmk-types/; do
    if [ -d "$dir" ] && [ -d "$dir/src" ]; then
        cd "$dir"
        echo "Cleaning $dir"
        cargo clean
        cd ..
    fi
done