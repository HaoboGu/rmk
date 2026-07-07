#!/bin/bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/../.github/ci/_lib.sh"

echo "Keep nightly up-to-date so rustfmt matches CI: rustup update nightly --force"

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

if [ "${1:-}" = "--help" ]; then
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
if [ "${1:-}" = "--touched" ]; then
    CHANGED=$(git status --porcelain | awk '/^[? MARC][? MARC] .*\.rs$/ { print $2 }')
    format_changed $CHANGED
    exit 0
fi

# If --touched-branch is passed, only format .rs files changed since branching off main
if [ "${1:-}" = "--touched-branch" ]; then
    BASE=$(git merge-base HEAD main 2>/dev/null || true)
    if [ -z "$BASE" ]; then
        exit 0
    fi
    CHANGED=$(git diff --diff-filter=d --name-only "$BASE" HEAD | grep '\.rs$' || true)
    format_changed $CHANGED
    exit 0
fi

# If --touched-since <ref> is passed, only format .rs files changed since the given ref
if [ "${1:-}" = "--touched-since" ]; then
    if [ -z "${2:-}" ]; then
        echo "Usage: $0 --touched-since <ref>"
        exit 1
    fi
    CHANGED=$(git diff --diff-filter=d --name-only "$2" HEAD | grep '\.rs$' || true)
    format_changed $CHANGED
    exit 0
fi

log_section "Formatting workspace crates"
for crate in rmk rmk-config rmk-macro rmk-types; do
    cargo +nightly fmt --manifest-path "$crate/Cargo.toml"
done

log_section "Formatting examples"
while IFS= read -r manifest; do
    cargo +nightly fmt --manifest-path "$manifest"
done < <(list_example_manifests)
