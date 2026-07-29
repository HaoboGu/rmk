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
keyboard = "boards/split.toml"        # the keyboard; sections below deep-merge over it

[behavior]                            # real keyboard.toml behavior config for this file's tests

features = ["vial"]                   # cargo features every test here needs

[[test]]
name = "hold"                         # unique, valid Rust identifier
features = ["storage"]                # extra features for this test only
steps = [ ... ]

[test.behavior.morse]                 # per-test behavior delta, deep-merged
permissive_hold = false
```

`features` becomes a `#[cfg(all(feature = ..))]` on the generated test, so a
scenario needing `steno` or `passkey_entry` still compiles in the feature sets
that lack them. File-level and per-test lists are unioned.

Shared keys live on the boards under `boards/`:

- `boards/split.toml` — a Sofle-style 5x14 split with 4 layers carrying every
  fixture (one-shot row, HRM/morse home row, combo letters, thumb/variant row,
  passkey row) plus two encoders. Its header documents the cell assignments.
- `boards/alt_layout_split.toml` — a 36-key split with morse keys on the home
  row, replicating the real setup behind the rollover regressions in
  `morse_rollover.toml`.

The keyboard half is the real `keyboard.toml` format, resolved by the same
`rmk-config` + `rmk-macro` pipeline firmware uses — a user's `keyboard.toml`
works as a board as-is. Simulation reads only its keymap and behavior and never
resolves its hardware, so `[matrix]`, `[split]`, `[ble]` and friends come along
without taking effect: a scenario is free to override `[layout]` and test a
smaller matrix than the board's pins describe. A scenario may also define
`[layout]`/`[keymap]`/`[aliases]` itself (tables merge key-by-key, arrays and
scalars replace wholesale). Files whose subject *is* one narrow keyboard —
`hid_reports.toml`, `steno.toml` — carry their own instead of adding one-off
keys to a shared board. `[rmk]` capacities are the one exception: they are compile-time
constants baked into the test binary, so a scenario that sets them is rejected
rather than silently ignored. Per-key hand assignment lives in the
`[layout].map` tokens: `(row,col,L)`, `(row,col,R)`, `(row,col,*)`.

Encoders come from `[[input_device.encoder]]`, or from the halves' own
`[input_device]` on a split board — simulated knobs need only the board-wide
count. Their pins are ignored; `[[keymap.layer]].encoders` maps them per layer.

## Steps

| Step | Meaning |
|---|---|
| `{ press = [r, c] }` / `{ release = [r, c] }` | key down / up at matrix position |
| `{ tap = [r, c, ms] }` | press, wait `ms`, release |
| `{ rotary_cw = id }` / `{ rotary_ccw = id }` | one detent of encoder `id` (a press plus a release) |
| `{ delay = ms }` | advance virtual time |
| `{ expect = ["A", "B"] }` | next keyboard report: no modifiers, exactly these keycodes (order-insensitive) |
| `{ expect = { mods = ["LShift"], keys = ["B"] } }` | modifiers + keycodes; omit `keys` for modifier-only |
| `{ expect = "empty" }` | next keyboard report has nothing pressed |
| `{ no_report = ms }` | no HID report of any kind within the window |
| `{ expect_mouse = { buttons = 0, x = 0, y = 0, wheel = -1, pan = 0 } }` | exact mouse report (fields default to 0) |
| `{ expect_consumer = "AudioVolUp" }` / `{ expect_system = "SystemSleep" }` | next consumer / system-control report; `"empty"` is the all-released one |
| `{ expect_steno = ["S1"] }` | next steno chord report; `"empty"` for no keys down |
| `{ passkey = "begin" }` / `{ passkey = "end" }` | open / close BLE passkey entry around a stretch of the timeline |
| `{ expect_passkey = 123456 }` / `{ expect_passkey = "cancelled" }` | the passkey the entry submitted, or that it was cancelled |

Modifier names: `LCtrl` `LShift` `LAlt` `LGui` `RCtrl` `RShift` `RAlt` `RGui`.
Keycodes and action strings use the `keyboard.toml` vocabulary (`A`, `Kp1`,
`MT(B, LShift)`, `LT(1, D)`, `TD(0)`, `OSM(LShift)`, `WM(X, LShift | LCtrl)`,
aliases from `[aliases]`, named morse profiles as `MT(A, LShift, name)`).

Every test also inherits the harness's end-of-run checks: no unasserted
trailing report, no still-pressed inputs, empty held buffer.

## What stays in Rust

A scenario's contract is one keyboard plus a timeline of physical input and HID
output. Two kinds of test fall outside it, by construction rather than by budget:

- **Wire protocol.** `rynk_loopback.rs` and `rynk_hid_loopback.rs` assert
  frames, error codes and COBS/HID reframing with no keyboard behind them, and
  `host_integration_test.rs` interleaves a host session with matrix input to
  prove the write path and the keyboard's read path share one `KeyMap`. A
  protocol vocabulary in TOML would have to restate wire shapes that the typed
  `host.vial(..)` / `host.rynk(..)` builders already model.
- **The harness itself.** `simulator_test.rs` asserts that the timeline runs and
  that the end-of-run checks fire. Asserting that through the codegen which
  consumes it would be circular.

Invalid config is *not* one of them: rejecting a bad `keyboard.toml` is
`rmk-config`'s job and is tested there. What rmk tests instead is the runtime
guard for state a host can write over the wire but no config can express — see
`rynk_out_of_range_default_layer_write_is_ignored`.
