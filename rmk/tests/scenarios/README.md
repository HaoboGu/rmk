# Simulator scenarios

Each `.toml` file here defines a keyboard and a list of tests against it. The
`run_tests!` macro (registered in `tests/toml_scenarios.rs`) expands every file
into ordinary `#[test]` fns that drive the simulator harness in `tests/common`,
so scenarios run, filter, and report exactly like hand-written tests:

```console
cargo nextest run --no-default-features --features=split,vial,storage,async_matrix,_ble --test toml_scenarios
```

Test ids are `<file_stem>::<test name>`. Editing a scenario file triggers a
rebuild of the test binary; adding a new file needs one `run_tests!` line in
`tests/toml_scenarios.rs`.

## File layout

```toml
keyboard = "boards/test_keymap.toml"  # optional base board; sections below deep-merge over it
requires = ["storage"]                # cargo features every test in this file needs

[layout]                              # real keyboard.toml sections
[keymap]                              # [[keymap.layer]] blocks replace the base's layers wholesale
[behavior]
[aliases]

[[test]]
name = "hold"                         # unique, valid Rust identifier
requires = ["vial"]                   # per-test features, unioned with the file's
storage = true                        # attach an in-memory flash (default false)
keys = [ ... ]                        # per-test cell overrides
steps = [ ... ]

[test.behavior.morse]                 # per-test behavior delta, deep-merged
permissive_hold = false
```

The keyboard half is the real `keyboard.toml` format, resolved by the same
`rmk-config` + `rmk-macro` pipeline firmware uses — a user's `keyboard.toml`
works as a base board as-is (hardware sections are ignored). Only `[layout]`,
`[keymap]`, `[behavior]`, and `[aliases]` may appear in a scenario; `[rmk]`
capacities are compile-time constants and are rejected. Merge rule: tables
merge key-by-key, arrays and scalars replace wholesale.

Per-key hand assignment (for HRM/bilateral tests) lives in the `[layout].map`
tokens: `(row,col,L)`, `(row,col,R)`, `(row,col,*)`.

## Steps

| Step | Meaning |
|---|---|
| `{ press = [r, c] }` / `{ release = [r, c] }` | key down / up at matrix position |
| `{ tap = [r, c, ms] }` | press, wait `ms`, release |
| `{ delay = ms }` | advance virtual time |
| `{ expect = ["A", "B"] }` | next keyboard report: no modifiers, exactly these keycodes (order-insensitive) |
| `{ expect = { mods = ["LShift"], keys = ["B"] } }` | modifiers + keycodes; omit `keys` for modifier-only |
| `{ expect = "empty" }` | next keyboard report has nothing pressed |
| `{ no_report = ms }` | no HID report of any kind within the window |
| `{ expect_mouse = { buttons = 0, x = 0, y = 0, wheel = -1, pan = 0 } }` | exact mouse report (fields default to 0) |
| `"wait_storage"` | block until the pending flash write completes (needs `storage = true`) |
| `"restart"` | drop the keyboard, rebuild it from the same config and flash, continue |

Modifier names: `LCtrl` `LShift` `LAlt` `LGui` `RCtrl` `RShift` `RAlt` `RGui`.
Keycodes and action strings use the `keyboard.toml` vocabulary (`A`, `Kp1`,
`MT(B, LShift)`, `LT(1, D)`, `TD(0)`, `OSM(LShift)`, `WM(X, LShift | LCtrl)`,
aliases from `[aliases]`, named morse profiles as `MT(A, LShift, name)`).

Every test also inherits the harness's end-of-run checks: no unasserted
trailing report, no still-pressed inputs, empty held buffer.

## What stays in Rust

Vial/Rynk host-protocol transactions, passkey entry, harness self-tests, and
tests that need deliberately invalid configs or hand-built `Morse` values keep
using the `SimKeyboard` API directly — see the remaining `keyboard_*_test.rs`
files for the pattern.
