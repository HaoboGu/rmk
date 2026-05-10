#!/bin/bash

# Run `cargo fetch` in every directory with a Cargo.toml.
#
# Usage:
#   scripts/fetch_all.sh           # refresh each crate's Cargo.lock to
#                                  # satisfy current Cargo.toml requirements
#                                  # (run after a deps bump like a885a16f).
#   scripts/fetch_all.sh --check   # fail if any Cargo.lock is out of sync
#                                  # with its Cargo.toml. Used in CI.

set -e

mode=fetch
case "${1:-}" in
    --check) mode=check ;;
    "") ;;
    *) echo "usage: $0 [--check]" >&2; exit 2 ;;
esac

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

# `cargo fetch` only resolves and downloads — it doesn't compile — so the
# ESP examples' `rust-toolchain.toml` channel doesn't need to be installed.
# Override with stable so the script works without `espup install`.
export RUSTUP_TOOLCHAIN=stable

# Skip dirs that aren't meaningfully fetchable here:
#  - no src/ (e.g. examples/use_config/nrf52810_ble: declares `[[bin]]` but
#    ships no source — `cargo` rejects the manifest). Mirrors the gate in
#    scripts/check_all.sh and scripts/clean_all.sh.
#  - Cargo.lock not tracked by git (the library crates rmk/rmk-config/
#    rmk-macro/rmk-types per the .gitignore "ignore Cargo.lock of library,
#    but keep it for binaries" convention). Nothing to refresh into a
#    commit, and on fresh checkouts `--locked` would fail to create one.
while IFS= read -r manifest; do
    dir="$(dirname "$manifest")"
    if [ ! -d "$dir/src" ]; then
        echo "Skipping $dir (no src/)"
        continue
    fi
    if ! git ls-files --error-unmatch "$dir/Cargo.lock" >/dev/null 2>&1; then
        echo "Skipping $dir (Cargo.lock not tracked)"
        continue
    fi
    if [ "$mode" = check ]; then
        echo "Checking $dir"
        (cd "$dir" && cargo fetch --locked)
    else
        echo "Fetching $dir"
        (cd "$dir" && cargo fetch)
    fi
done < <(find . -name Cargo.toml -not -path './target/*' -not -path '*/target/*' | sort)
