# shellcheck shell=bash
#
# Shared bootstrap for RMK CI scripts. Source this from other scripts in
# .github/ci/ to pick up common env, cache, helpers, and example discovery.
#
#     source "$(dirname "${BASH_SOURCE[0]}")/_lib.sh"
#
# Expected preamble in the caller:
#
#     #!/bin/bash
#     set -euo pipefail
#

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

export CARGO_TERM_COLOR=always
export CARGO_TERM_PROGRESS_WHEN=never
export CARGO_NET_GIT_FETCH_WITH_CLI=true
export TERM="${TERM:-dumb}"

# Cache root: if RMK_CI_CACHE_ROOT is set (self-hosted runner or local reuse),
# redirect rustup/cargo homes and target dir there. Otherwise fall back to
# an in-tree target/ci dir so local runs don't clobber the default target.
if [[ -n "${RMK_CI_CACHE_ROOT:-}" ]]; then
    export RUSTUP_HOME="${RUSTUP_HOME:-$RMK_CI_CACHE_ROOT/rustup}"
    export CARGO_HOME="${CARGO_HOME:-$RMK_CI_CACHE_ROOT/cargo}"
    export PATH="$CARGO_HOME/bin:$PATH"
    mkdir -p "$RMK_CI_CACHE_ROOT"
    if [[ -f "$HOME/.cargo/config.toml" && ! -f "$CARGO_HOME/config.toml" ]]; then
        mkdir -p "$CARGO_HOME"
        cp "$HOME/.cargo/config.toml" "$CARGO_HOME/config.toml"
    fi
    target_root="$RMK_CI_CACHE_ROOT/target"
else
    target_root="$repo_root/target/ci"
fi
mkdir -p "$target_root"

# Embedded Rust targets every stable toolchain build needs.
TARGETS=(
    thumbv6m-none-eabi
    thumbv7m-none-eabi
    thumbv7em-none-eabi
    thumbv7em-none-eabihf
    thumbv8m.main-none-eabihf
    riscv32imc-unknown-none-elf
    riscv32imac-unknown-none-elf
)

log_section() {
    printf "\n==> %s\n" "$1"
}

ensure_toolchain() {
    rustup toolchain install "$1" --profile minimal --no-self-update
}

ensure_stable_toolchain() {
    ensure_toolchain stable
    rustup component add clippy rustfmt llvm-tools --toolchain stable
    rustup target add --toolchain stable "${TARGETS[@]}"
}

# $1=bin $2=crate-name
ensure_cargo_tool() {
    if command -v "$1" >/dev/null 2>&1; then
        return
    fi
    cargo +stable install --locked "$2"
}

require_cargo_batch() {
    if command -v cargo-batch >/dev/null 2>&1; then
        return
    fi
    cargo install --git https://github.com/embassy-rs/cargo-batch cargo --bin cargo-batch --locked
}

ensure_esp_toolchain() {
    ensure_cargo_tool espup espup
    espup install
    if [[ ! -f "$HOME/export-esp.sh" ]]; then
        echo "espup did not create $HOME/export-esp.sh" >&2
        exit 1
    fi
}

# Echoes Cargo.toml paths for every buildable example, one per line.
# A buildable example is a direct child of examples/use_{rust,config}/ that
# has both a src/ dir and a Cargo.toml (filters out placeholders like fix/).
list_example_manifests() {
    for dir in examples/use_rust/*/ examples/use_config/*/; do
        if [[ -d "$dir/src" && -f "$dir/Cargo.toml" ]]; then
            printf '%s\n' "${dir}Cargo.toml"
        fi
    done
}
