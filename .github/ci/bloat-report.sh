#!/bin/bash
# Assemble a bloat report from size-data artifact files.
# Each file contains lines of the form: dir|bin|base_size|head_size
# Writes a markdown table to $GITHUB_STEP_SUMMARY (or stdout if unset).
set -euo pipefail

artifact_dir="${1:?Usage: bloat-report.sh <artifact-dir>}"
output="${GITHUB_STEP_SUMMARY:-/dev/stdout}"

{
    echo "# RMK size report"
    echo
    echo "| Target | Base | Head | Delta |"
    echo "| --- | ---: | ---: | ---: |"

    # Process all size-data files, sorted for stable output order.
    for f in $(find "$artifact_dir" -name 'size-data.txt' -type f | sort); do
        while IFS='|' read -r dir bin base_size head_size; do
            delta=$((head_size - base_size))
            if [[ -n "$bin" ]]; then
                label="$dir [$bin]"
            else
                label="$dir"
            fi
            printf "| %s | %s | %s | %+d |\n" "$label" "$base_size" "$head_size" "$delta"
        done < "$f"
    done
} | tee -a "$output"
