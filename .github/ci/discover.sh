#!/bin/bash
# Discover buildable examples and emit GitHub Actions matrix JSON.
# Outputs:
#   stable  — JSON array of {dir, target, bloat, bloat_bins} for stable-toolchain examples
#   esp     — JSON array of {dir, target} for xtensa/ESP examples
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

# Inline the discovery + target-extraction logic so this script stays
# lightweight (no toolchain install, no target-dir mkdir).

EXAMPLE_SKIPLIST=(
    "examples/use_rust/nrf54lm20_ble"
    "examples/use_config/esp32_ble_split"
    "examples/use_rust/py32f07x"
    "examples/use_rust/sf32lb52x_usb"
)

# Examples tracked for binary-size regression (bloat) reports on PRs.
# Keep in sync with the comment in the array — changes are rare.
BLOAT_DIRS=(
    # use_config — TOML-driven path
    "examples/use_config/nrf52832_ble"
    "examples/use_config/nrf52840_ble"
    "examples/use_config/nrf52840_ble_split"
    "examples/use_config/rp2040"
    "examples/use_config/rp2040_split"
    "examples/use_config/pi_pico_w_ble"
    "examples/use_config/stm32f1"
    "examples/use_config/stm32h7"
    # use_rust — pure-Rust API path (mirrors use_config set)
    "examples/use_rust/nrf52832_ble"
    "examples/use_rust/nrf52840_ble"
    "examples/use_rust/nrf52840_ble_split"
    "examples/use_rust/rp2040"
    "examples/use_rust/rp2040_split"
    "examples/use_rust/pi_pico_w_ble"
    "examples/use_rust/stm32f1"
    "examples/use_rust/stm32h7"
)

# nrf52840_ble_split produces two binaries; list them comma-separated.
bloat_bins_for() {
    case "$1" in
        */nrf52840_ble_split|*/rp2040_split) echo "central,peripheral" ;;
        *) echo "" ;;
    esac
}

is_bloat_dir() {
    local d
    for d in "${BLOAT_DIRS[@]}"; do
        [[ "$1" == "$d" ]] && return 0
    done
    return 1
}

stable='['
esp='['
first_stable=1
first_esp=1

for dir_slash in examples/use_rust/*/ examples/use_config/*/; do
    [[ -d "$dir_slash/src" && -f "$dir_slash/Cargo.toml" ]] || continue
    dir="${dir_slash%/}"

    skip=0
    for entry in "${EXAMPLE_SKIPLIST[@]}"; do
        [[ "$dir" == "$entry" ]] && { skip=1; break; }
    done
    (( skip )) && continue

    config="$dir/.cargo/config.toml"
    [[ -f "$config" ]] || continue
    target="$(awk '
        /^\[/ { section = $0; next }
        section == "[build]" && /^[[:space:]]*target[[:space:]]*=/ {
            sub(/^[[:space:]]*target[[:space:]]*=[[:space:]]*/, "")
            sub(/[[:space:]]*#.*$/, "")
            sub(/^"/, ""); sub(/"[[:space:]]*$/, "")
            print; exit
        }
    ' "$config")"
    [[ -z "$target" ]] && continue

    case "$dir" in
        */esp32s3_ble)
            entry="{\"dir\":\"$dir\",\"target\":\"$target\"}"
            (( first_esp )) && first_esp=0 || esp+=','
            esp+="$entry"
            ;;
        *)
            bloat=false
            bins=""
            if is_bloat_dir "$dir"; then
                bloat=true
                bins="$(bloat_bins_for "$dir")"
            fi
            entry="{\"dir\":\"$dir\",\"target\":\"$target\",\"bloat\":$bloat,\"bloat_bins\":\"$bins\"}"
            (( first_stable )) && first_stable=0 || stable+=','
            stable+="$entry"
            ;;
    esac
done

stable+=']'
esp+=']'

echo "stable=$stable" >> "$GITHUB_OUTPUT"
echo "esp=$esp" >> "$GITHUB_OUTPUT"

# Debug: show what was discovered
echo "Stable examples: $(echo "$stable" | grep -o '"dir"' | wc -l | tr -d ' ')"
echo "ESP examples: $(echo "$esp" | grep -o '"dir"' | wc -l | tr -d ' ')"
echo "Bloat examples: $(echo "$stable" | grep -o '"bloat":true' | wc -l | tr -d ' ')"
