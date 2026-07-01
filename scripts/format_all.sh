#!/bin/bash

set -e

usage() {
    echo "Usage: $0 [OPTION]"
    echo "Format all Rust source files in the repository."
    echo ""
    echo "Options:"
    echo "  --help              Show this help message and exit"
    echo "  --touched           Format only .rs files changed in the working tree"
    echo "  --touched-branch    Format only .rs files changed since branching off main"
    echo "  --touched-since REF Format only .rs files changed since REF"
}

if [ "$1" = "--help" ]; then
    usage
    exit 0
fi

format_changed() {
    if [ -z "$1" ]; then
        exit 0
    fi
    rustfmt +nightly "$@"
}

# If --touched is passed, only format .rs files changed in the working tree
if [ "$1" = "--touched" ]; then
    CHANGED=$(git status --porcelain | awk '/^[? MARC][? MARC] .*\.rs$/ { print $2 }')
    format_changed $CHANGED
    exit 0
fi

# If --touched-branch is passed, only format .rs files changed since branching off main
if [ "$1" = "--touched-branch" ]; then
    BASE=$(git merge-base HEAD main 2>/dev/null || true)
    if [ -z "$BASE" ]; then
        exit 0
    fi
    CHANGED=$(git diff --diff-filter=d --name-only "$BASE" HEAD | grep '\.rs$' || true)
    format_changed $CHANGED
    exit 0
fi

# If --touched-since <ref> is passed, only format .rs files changed since the given ref
if [ "$1" = "--touched-since" ]; then
    if [ -z "$2" ]; then
        echo "Usage: $0 --touched-since <ref>"
        exit 1
    fi
    CHANGED=$(git diff --diff-filter=d --name-only "$2" HEAD | grep '\.rs$' || true)
    format_changed $CHANGED
    exit 0
fi

# Format rmk and rmk-macro
cd rmk && cargo +nightly fmt && cd ..
cd rmk-macro && cargo +nightly fmt && cd ..
cd rmk-config && cargo +nightly fmt && cd ..
cd rmk-types && cargo +nightly fmt && cd ..

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