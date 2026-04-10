#!/bin/bash
set -euo pipefail
# shellcheck source=_lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/_lib.sh"

ensure_toolchain nightly
rustup component add rustfmt --toolchain nightly

log_section "Formatting workspace crates"
for crate in rmk rmk-config rmk-macro rmk-types; do
    cargo +nightly fmt --manifest-path "$crate/Cargo.toml" --check
done

log_section "Formatting examples"
while IFS= read -r manifest; do
    cargo +nightly fmt --manifest-path "$manifest" --check
done < <(list_example_manifests)
