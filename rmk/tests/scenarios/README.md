# Simulator scenarios

Each file defines a keyboard and named input/output cases. Every `*.toml` in this
directory is a scenario: `run_tests!("tests/scenarios")` expands each one into a
`mod` of ordinary tests, so dropping a file in is all it takes to register it.
Board fixtures live in `boards/`, a subdirectory, so they are not scenarios
themselves. Run them with:

```console
cargo nextest run --no-default-features --features=vial,host_lock,_no_usb,steno,passkey_entry --test integration
cargo nextest run --no-default-features --features=rynk,_ble,split,async_matrix,storage --test integration
```

`vial` and `rynk` are mutually exclusive, so the host-protocol files split
across those two rows.

```toml
keyboard = "boards/split.toml"
features = ["vial"]

[behavior.morse]
permissive_hold = true

[[test]]
name = "hold"
features = ["storage"]
behavior.morse = { hold_timeout = "250ms" }
steps = [
  { press = [1, 1] },
  { delay = 300 },
  { release = [1, 1] },
]
expect = [["LShift"], []]
```

`keyboard` is optional. Its path is relative to the scenario, and the current
file deep-merges over it. The keyboard portion uses normal `keyboard.toml`
layout, keymap, alias, encoder, and behavior syntax. Hardware sections are
ignored. `[rmk]` is rejected because its capacities are compile-time constants.
A test's own `behavior.<section>` key overrides the shared behavior for that
test only. Tables merge key by key; arrays such as `morses` replace wholesale.

`features` adds `#[cfg(feature = "...")]` gates; file and test features are
combined. Test IDs are `<file>::<name>`.

## Files

One file per feature, except morse/tap-hold, which is most of the suite and
splits by resolution mode. Those all start with `morse_`, so
`nextest run morse_` runs the family:

| File | Covers |
|---|---|
| `morse_normal`, `morse_permissive_hold`, `morse_hold_on_other_press`, `morse_hrm` | The same home-row fixture under each resolution mode |
| `morse_combo_*` | One combo table over morse keys, under each of those modes |
| `morse_tap_dance`, `morse_bilateral`, `morse_rollover`, `morse_layer_release`, `morse_quick_tap` | Morse behavior that isn't mode-specific |
| `rynk_*` | The Rynk host protocol, one file per endpoint group |
| everything else | One feature each — `combo`, `one_shot`, `layer`, `encoder`, `macros`, `hid_reports`, `passkey`, `steno` |

Cases that only differ by mode share a name across files, so
`nextest run two_key_misses_window` prints one row of the matrix. Keep that
up when adding a case that an existing file already covers under another mode.

## Input

| Step | Meaning |
|---|---|
| `{ press = [row, col] }` / `{ release = [row, col] }` | Matrix key down/up |
| `{ tap = { pos = [row, col], duration = ms } }` | Press, wait `ms`, then release |
| `{ rotary_cw = id }` / `{ rotary_ccw = id }` | Encoder detent |
| `{ delay = ms }` | Advance virtual time |
| `{ no_report = ms }` | Advance virtual time, failing if anything reaches the host |
| `{ passkey = "begin" }` / `{ passkey = "end" }` | Bound BLE passkey entry |
| `{ rynk = { cmd = "...", payload = ..., reply = ... } }` | One Rynk request and the reply it must draw |
| `{ rynk_topic = { topic = "...", payload = ... } }` | Assert the device's next frame is this topic push |
| `{ rynk_publish = { topic = "...", payload = ... } }` | Publish the internal event a topic forwards, to cause that push |
| `{ rynk_raw = [bytes] }` / `{ rynk_reply = [bytes] }` | Unframed bytes onto the link, and the reply they must draw |
| `{ rynk_no_reply = ms }` | Assert the device stays silent |

`no_report` is a step rather than an expectation because an assertion deferred
to the end of the timeline can no longer say *when* nothing was reported.

Every step runs 10 ms after the one before it unless a `delay` sets that
interval instead — far below any behavior timeout, so it only orders the steps.
Spell out a `delay` where the timing is what the case is about, and leave it out
everywhere else.

## Output

The input runs to completion first and the assertions follow it; reports queue
up meanwhile, so the split only moves when they are asserted, not what arrives.

Keyboard reports are arrays containing modifiers and keycodes in any order;
`[]` is the all-released report. Modifier names are `LCtrl`, `LShift`, `LAlt`,
`LGui`, `RCtrl`, `RShift`, `RAlt`, and `RGui`. Keep `expect` on one line — it
reads as the trace the host saw — and break it up only to annotate a report or
when it runs long.

```toml
expect = [
  ["LShift", "A"],
  { consumer = ["AudioVolUp"] },
  { system = ["SystemSleep"] },
  { mouse = { x = 5 } },
  { steno = ["S1"] },
  { passkey = 123456 }, # or "cancelled"
]
```

Every case also rejects trailing reports, pressed inputs, and buffered keyboard
state.

## Rynk

A `rynk` step names a command from the protocol's endpoint table and spells its
payload with the request type's own serde shape — there is no second vocabulary
to keep in step with the protocol, and a new endpoint needs no code change.
`reply` is the expected response payload, defaulting to the unit response every
setter returns; `error` replaces it with a `RynkError` variant.

```toml
{ rynk = { cmd = "SetKeyAction", payload = { position = { layer = 0, row = 0, col = 0 }, action = { Single = { Key = { Hid = "B" } } } } } },
{ rynk = { cmd = "GetVersion", reply = { major = 0, minor = 1 } } },
{ rynk = { cmd = "GetMatrixState", error = "Locked" } },
```

Payloads reach the test as JSON and are deserialized there, against the
firmware's own `rmk-types` — not encoded at macro expansion, where `Action`'s
`#[cfg(feature = "steno")]` variant would shift every postcard discriminant
after it. An omitted field is `None` (or `false` for a bitfield flag), which is
how TOML says `null`.

A request is a barrier: its reply must arrive before the next step runs, so a
write is applied by the time the matrix input after it is played.

`[host]` configures the lock gate, and its `unlock_keys` are ordinary matrix
positions — so a scenario completes the unlock ceremony by pressing them.

## Adding a case

**Pick the file first**, using the table above. A new file is picked up by
dropping it in this directory, but its `features` have to fit a feature set in
`.github/ci/_lib.sh` — a case gated on a feature no row enables compiles out of
every row and silently never runs, while an ungated case runs in all three.

**Name it after the behavior it pins**, not the input that gets there. The name
is what a failing CI row prints and what `nextest run <name>` reruns, so it has
to say what broke without opening the file.

**Reuse a board.** The files in `boards/` are the whole fixture set, and a case
that needs different keys almost always finds them somewhere on the board it
already has. Positions carry no names in `steps`, so a file that leans on
particular keys maps them out in its header comment, as `combo.toml` does. Add
a board only for a geometry the existing ones can't express.

**Say only what the case is about.** Behavior that several cases share belongs
on the board or the file, and a test's own `behavior.<section>` states just the
delta it turns on — that delta is then the visible reason the case exists. A
spelled-out `delay` reads the same way, as the timing the case turns on.

**Decide the trace before running it.** The mismatch message prints what
arrived in `expect` syntax, so it pastes straight back into the file. That is
for fixing a wrong expectation, not for writing one: pasted blindly, it records
whatever the firmware does today, bug included.

**Confirm it fails.** Flip a keycode or revert the fix under test, and check
that the failure names the step you meant:

```console
expect[0]: keyboard report mismatch
  expected ["B"]
    actual ["V"]
```

`expect[k]` and `reply[k]` count the assertions the run played, in order, so
they index the scenario's own lists. Confirming matters most when a case
asserts absence: omitting `expect` asserts only that nothing was reported and
nothing stuck, which also holds when the input never reached the keyboard.

**Carry the bug in a regression case.** The comment says what the wrong
behavior was, not just that there was a bug — a later reader deciding whether
the case still earns its place needs the symptom. When the fix could regress
into "nothing happens at all", add the control that proves the feature still
fires: `mouse_key_combo_does_not_stick_wheel` is paired with
`mouse_wheel_tap_settles` for that reason.

**Write it in Rust only when a timeline has no vocabulary for it** — what
`tests/integration/rynk.rs` and `tests/integration/vial.rs` hold is read-chunk
boundaries, buffer occupancy, two concurrent sessions, and two keyboards built
over one flash. Prefer that over teaching the schema a word one case needs. Vial
has no scenario vocabulary at all, so its whole surface lives there. Rust cases
still drive the keyboard end-to-end through `SimKeyboard` rather than reaching
past it for `Keyboard::new`, `KeyMap::new`, or a report channel.
