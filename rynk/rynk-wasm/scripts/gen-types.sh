#!/usr/bin/env bash
# Regenerate bindings/rynk.d.ts — the Rust↔TS wire-type contract for the Rynk
# protocol, emitted by tsify from rmk-types. Run after changing any wire type;
# CI diffs the result to catch drift.
set -euo pipefail
cd "$(dirname "$0")/.." # -> rynk-wasm/

command -v wasm-pack >/dev/null || {
    echo "gen-types: wasm-pack not found (cargo install wasm-pack)" >&2
    exit 1
}

wasm-pack build --target web >/dev/null

mkdir -p bindings
{
    echo "// Code-generated from rmk-types by tsify — DO NOT EDIT."
    echo "// Rust↔TS wire-type contract for the Rynk protocol."
    echo "// Regenerate: cd rynk/rynk-wasm && ./scripts/gen-types.sh"
    echo
    # wasm-pack emits the tsify type declarations at the top of the .d.ts,
    # followed by the RynkClient class + init glue (runtime, not the contract).
    # Print everything up to the last type, stopping before the class's JSDoc.
    awk '
        { line[NR] = $0 }
        /^export class RynkClient/ { cls = NR }
        END {
            if (!cls) { print "gen-types: RynkClient marker not found" > "/dev/stderr"; exit 1 }
            end = cls - 1
            while (end >= 1 && (line[end] ~ /^[[:space:]]*$/ || line[end] ~ /^ \*/ || line[end] ~ /^\/\*\*/)) end--
            for (i = 1; i <= end; i++) print line[i]
        }
    ' pkg/rynk_wasm.d.ts
} >bindings/rynk.d.ts

echo "wrote bindings/rynk.d.ts"
