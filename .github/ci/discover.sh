#!/bin/bash
# Discover buildable examples and emit GitHub Actions matrix JSON.
# Outputs:
#   stable  — JSON array of {dir, target} for stable-toolchain examples
#   esp     — JSON array of {dir, target} for xtensa/ESP examples
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

# Inline the discovery + target-extraction logic so this script stays
# lightweight (no toolchain install, no target-dir mkdir).

EXAMPLE_SKIPLIST=(
    "examples/use_rust/nrf54lm20_ble"
    "examples/use_config/esp32_ble_split"
)

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

    entry="{\"dir\":\"$dir\",\"target\":\"$target\"}"
    case "$dir" in
        */esp32s3_ble)
            (( first_esp )) && first_esp=0 || esp+=','
            esp+="$entry"
            ;;
        *)
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
