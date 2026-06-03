# Unify One-Shot Modifier (OSM) and Sticky Key (SK) — Design

**Date:** 2026-06-02
**Status:** Designed (not yet implemented)
**Branch:** `feat/osm-sticky-key-merge` (forked from `feat/sticky-mod`)
**Repo:** rmk-fork (RMK firmware), consumed by RMKSofleV2 via `[patch.crates-io]`
**Context:** RMK PR [#859](https://github.com/HaoboGu/rmk/pull/859)

## Overview

The `feat/sticky-mod` branch added a `StickyKey` (SK) behavior and HaoboGu (RMK
owner) asked, in PR #859, that one-shot modifiers and sticky-mod be **unified**
into a single behavior. As implemented, they are **not** unified — OSM and SK
share essentially no runtime logic. They have separate state types, separate
timeout mechanisms, and separate release-trigger plumbing. The only shared code
is the final modifier-merge sink (`resolve_explicit_modifiers`).

This design unifies the **runtime** of OSM and SK behind one shared latch state
machine, driven by a small per-behavior **preset**, while keeping the public
surface (wire format, keycodes, config blocks, TOML syntax, tests) frozen for
strict backward compatibility. One-shot **layer** (OSL) is deliberately left on
its own layer-activation path this round, but rides the shared plumbing and is
left with a documented seam to fold in later.

**Goals:**

1. **Backward compatible** — no change to the postcard wire format, Via/Vial
   keycodes, config blocks, or TOML keymap syntax.
2. **Keep every feature** of both the current OSM implementation *and* SK —
   nothing dropped. The existing test suites are the parity oracle.
3. **Cut shared code** — one state type, one timeout mechanism, one
   foreign-key release pass, one modifier-resolve sink.
4. **Fix the OSM "select race"** as a natural consequence of moving OSM onto
   SK's non-blocking deadline mechanism.

**Mostly internal.** The unification itself adds no user-facing feature —
OSM/OSL observable behavior is identical except that the OSM timeout race goes
away. The one intentional new capability is **SK-only**: a global
`[behavior.sticky_key]` default plus an optional per-key **profile** that can
override any SK setting field-by-field (Section 5). It is purely additive —
existing SK configs, the SK keymap syntax, the `StickyKeyAction` wire struct,
and the 11 SK tests are all untouched.

---

## Current state (why they are not unified)

Three plumbing layers diverge today.

### State representation

- **OSM/OSL** — `OneShotState<T>` (`rmk/src/keyboard/oneshot.rs:10`), a 4-state
  machine `Initial(T)` / `Single(T)` / `Held(T)` / `None`, generic over
  `T = ModifierCombination` (OSM) or `T = u8` (OSL).
- **SK** — `StickyKeyState` (`rmk/src/keyboard/sticky_key.rs:24`), a 2-state
  machine `None` / `Active { mods, repeat_count, max_repeat, exit_on_layer_change,
  deadline }`.

### Timeout mechanism (the core divergence)

- **SK** is **non-blocking**: it stores `deadline: Option<Instant>` in its state,
  and the `run()` loop (`rmk/src/keyboard.rs:158-185`) races the event subscriber
  against `sk_deadline`. Expiry is handled in the main loop.
- **OSM/OSL** are **blocking**: on release they call
  `select(Timer::after(timeout), subscriber.next_message())` inline
  (`oneshot.rs:75-93` for OSM, `139-152` for OSL). If a real key event wins the
  race, it is pushed onto `self.unprocessed_events` to be replayed. This inline
  `select` is the source of the documented "select race" (RMKSofleV2 `TODO.md`)
  and the `keyboard.rs:146` TODO wondering whether `unprocessed_events` "can be
  removed in the future."

### Release-trigger plumbing

- **OSM** consume runs in `process_action_key` (`keyboard.rs:1586`):
  `update_osm(event)` flips `Single → None`, honoring `quick_release` (consume on
  next *press* vs. next *release*) and promoting `Initial → Held` (the held /
  mouse-click path).
- **SK** release runs in `process_key_action_normal` (`keyboard.rs:1220-1230`):
  any non-SK, non-modifier key tears the latch down.

### The one shared sink

`resolve_explicit_modifiers` (`keyboard.rs:1380-1403`) already merges
`held_modifiers + osm_state.value() + sticky_key_state.value()`. This is the
only place the two behaviors meet today.

### Why HaoboGu's sketch is insufficient

PR #859 sketches `OSM(mod) == SK(mod, [], 1)`. That is a *simplification* that
drops OSM's richer behaviors (modifier accumulation, held-promotion, double-press
un-latch, `quick_release`, `activate_on_keypress`). The unified mechanism must
therefore be a **superset** with OSM and SK as two **presets**, not a collapse of
one into the other.

---

## Section 1 — Scope & compatibility contract

**Frozen surface (nothing here moves — this is what guarantees backward compat):**

- `Action::OneShotModifier`, `Action::OneShotLayer`, `Action::StickyKey` stay as
  distinct variants in the **same wire order** (`rmk-types/src/action/mod.rs:57`).
  The `Action` enum derives `Serialize`/`Deserialize`/`MaxSize` and postcard
  encodes by variant order, so stored keymaps and Vial stay valid.
- Via/Vial keycode mappings for OSM/OSL
  (`rmk/src/host/via/keycode_convert.rs`) unchanged. (`StickyKey` is absent from
  that map today — confirming it is *not* on the keycode wire — and stays absent.)
- **Two separate config blocks stay:** `[behavior.one_shot]` +
  `one_shot_modifiers` (`activate_on_keypress`, `quick_release`) and
  `[behavior.sticky_key]` (`timeout`). Independent timeouts are a feature, and
  keeping both is also what compat requires. (`[behavior.sticky_key]` *grows*
  additively — optional default fields plus a `profiles` subtable, per
  Section 5; a config that sets only `timeout` is unaffected.)
- TOML keymap syntax `OSM(...)` / `OSL(...)` / `SK(...)` unchanged.
- **Parity oracle:** all existing OSM/OSL tests
  (`rmk/tests/keyboard_one_shot_test.rs`, 25 tests) and SK tests
  (`rmk/tests/keyboard_sticky_key_test.rs`, 11 tests) stay green, **unmodified**.
  They *are* the definition of "all features preserved."

**In scope:** merge the *runtime* of OSM and SK into one shared mechanism.
**Out of scope this round:** OSL keeps its own layer activate/deactivate code path
(documented seam only).

---

## Section 2 — The unified state + preset model

Replace `OneShotState<T>` and `StickyKeyState` with **one** internal latch type
that is the union of both:

```rust
enum StickyLatch {
    None,
    Engaged {
        mods:         ModifierCombination, // OSM accumulates; SK = `keep`
        key:          Option<KeyCode>,     // SK bundled key; None for OSM
        phase:        Phase,               // Pressed | Latched | Held (≈ OSM Initial/Single/Held)
        repeat_count: u16,                 // SK cycling; OSM stays 1
        preset:       Preset,              // which feature-set is active
        deadline:     Option<Instant>,     // unified timeout
    },
}
```

`Preset` is the small config that selects *which* behaviors are live, so OSM and
SK become two configurations of one machine:

| Preset field | OSM value | SK value |
|---|---|---|
| `accumulate` (combine repeated presses) | yes | no |
| `held_promotion` (foreign key while held → normal modifier) | yes | no |
| `double_press_consume` (re-press same mod un-latches) | yes | no |
| `quick_release` / `activate_on_keypress` | from config | n/a |
| `bundles_key` | no | yes |
| `max_repeat` (0 = infinite) | 1 | from action |
| `keep_set` (which keys re-arm vs. release) | empty | `keep` mods |
| `exit_on_layer_change` | no | from action |
| `timeout_source` | `[behavior.one_shot]` | per-key or `[behavior.sticky_key]` |

The `Action::OneShotModifier` and `Action::StickyKey` dispatch arms
(`keyboard.rs:1321-1328`) become **thin adapters** that build the right `Preset`
and hand off to the shared engine.

**Readability guardrail:** the engine keeps OSM's and SK's transition logic as
preset-aware paths over this one state. We are explicitly **not** forcing a single
mega-`match` if it hurts readability. The win is one *state type* + one plumbing
layer + one set of release/timeout rules — not necessarily one giant function.

---

## Section 3 — Shared plumbing (and what gets deleted)

### 3a. Timeout — one mechanism (deadline), delete the inline `select`

The unified latch carries `deadline: Option<Instant>` (already in the Section 2
shape). The `run()` loop's existing deadline race (`keyboard.rs:158-185`) handles
expiry for **both** OSM and SK. When the deadline fires, the engine runs the same
consume/release path SK uses today.

**Deleted:** the `select(timeout, next_message)` blocks in `process_action_osm`
(`oneshot.rs:75-93`) and `process_action_osl` (`oneshot.rs:139-152`); and — if
nothing else still pushes to it — the `unprocessed_events` re-queue path.

**Audit gate:** before removing `unprocessed_events`, confirm OSM/OSL are its
only producers (grep). If something else uses it, it stays and only the OSM/OSL
producers are removed.

This is the part that **fixes the select race** rather than carrying it forward,
and it is what the `keyboard.rs:146` TODO is asking for.

### 3b. Release-on-foreign-key — one pass

OSM's `update_osm` consume (`keyboard.rs:1586`) and SK's non-SK-key release
(`keyboard.rs:1220-1230`) merge into **one "foreign key arrived" hook** over the
unified latch, parameterized by the preset:

- OSM preset: "consume per `quick_release`; promote `Initial → Held` first."
- SK preset: "release unless the key is in `keep_set` or is another SK press that
  cycles."

Same call site; the preset picks the rule.

### 3c. Layer-change release + the resolve sink

- Layer-change release is SK-only today (`exit_on_layer_change`, fired from
  `process_action_layer_switch:1600` and four spots in
  `process_key_action_normal`). It stays, now reading the unified latch's preset
  flag. OSM's preset leaves it off → no behavior change for OSM.
- `resolve_explicit_modifiers` (`keyboard.rs:1380-1403`) already merges held +
  OSM + SK modifiers. After unification it reads one `latch.value()` instead of
  two. Pure simplification.

### Net deletion target

- `OneShotState<T>` **and** `StickyKeyState` both go away → replaced by the one
  `StickyLatch`.
- The two inline-`select` timeout blocks go away.
- `unprocessed_events` re-queue likely goes away (pending the 3a audit).
- OSL keeps its own layer activate/deactivate calls (documented seam) but rides
  the same latch state and the same deadline / foreign-key plumbing.

---

## Section 4 — Staging & test strategy

Every commit stays test-green against the frozen 25 OSM/OSL + 11 SK tests. The
merge proceeds in dependency order — plumbing first, state second — so any
regression is bisectable to one stage. Tests run via `cargo nextest`.

- **Stage 0 — Characterize.** Run the full OSM/OSL/SK suite; record the green
  baseline. No code change.
- **Stage 1 — Unify the timeout plumbing, keep both state types.** Move OSM/OSL
  off inline `select` onto a `deadline` surfaced to the `run()` loop (reusing SK's
  deadline race). `OneShotState<T>` and `StickyKeyState` still exist separately —
  only the *expiry mechanism* is shared. Delete the `select` blocks; remove the
  OSM/OSL `unprocessed_events` producers (pending 3a audit). **Gate: all 36 tests
  green.** This isolates the single riskiest change (the timeout-semantics shift)
  to one commit — `git bisect` lands here if a hardware surprise appears.
- **Stage 2 — Merge the state representation.** Replace `OneShotState<T>` +
  `StickyKeyState` with `StickyLatch` + `Preset`. The `OneShotModifier` /
  `StickyKey` dispatch arms become thin preset-building adapters. Fold the
  foreign-key hook (3b) and resolve sink (3c) onto the single latch. **Gate: all
  36 tests green.**
- **Stage 3 — Tidy + document the OSL seam.** OSL still does its own layer
  activate/deactivate but now rides the shared latch + plumbing. Leave a clearly
  commented seam (a `// OSL fold point:` marker + short note) describing what a
  future "fold OSL fully in" change would collapse. **Gate: all 36 tests green +
  `cargo clippy` clean.**
- **Stage S — SK profile override (independent; Section 5).** Config-resolve +
  codegen only; orthogonal to the runtime-merge Stages 1–3, so it can land before
  or after them. Adds the new profile resolution tests (below) and keeps the 36
  existing tests green. **Gate: 36 existing tests green + new resolution tests
  green.**

**New tests:** for the *merge* (Stages 1–3), none for unchanged behavior — the
existing suite already defines parity; add one targeted regression test only if
the Stage 1 race fix creates a newly-correct behavior the old suite did not pin
(e.g. a key event arriving in the exact timeout window). For the *SK profile
override* (Stage S), add the small resolution-tier test set described in
Section 5.

---

## Section 5 — SK config: global default + per-key profile override (new SK capability)

This is the one intentional new feature in this work. It applies to **SK only** —
OSM/OSL are frozen (Section 1) and keep their global-only config. It is an
**independent workstream**: it touches the config-resolve + codegen layers, not
the runtime engine, so it can land before or after the Section 4 merge stages,
each step staying test-green.

### Requirement: profile-first configuration

The TOML profile (`[behavior.sticky_key]` plus its named `profiles`) is the
**preferred and primary** way to specify SK settings. Per-key overrides in the
`SK(...)` action string are a **secondary convenience, retained for now but
explicitly optional** — they may be removed in a later pass to simplify the code.

Consequence for the design: nothing may architecturally depend on key-level
overrides existing. Because every setting is resolved to a concrete value at
codegen (profile/global folded in), the runtime engine reads only fully-resolved
values and is blind to *where* a value came from. Dropping the per-key override
syntax later must therefore be a clean deletion of parser/codegen arms — no
runtime change, no engine coupling.

### Motivation

SK's current keymap form `SK(key, [mods], max_repeat, timeout_ms,
exit_on_layer_change)` is the only 5-positional-argument action in RMK; the
trailing `0, 0, true` is unreadable and the `0` sentinels are easy to mis-order.
We want a global default that every SK key inherits, with any individual key able
to override any field — **without** inventing inline named-parameter syntax (no
RMK action uses `key=value` inside the action string; named params live only in
`[behavior.*]` TOML tables).

### The three RMK override patterns (and which we pick)

RMK already solves "global default, override per key" two ways, and uses a third
(global-only) for OSM:

1. **Global-only** (OSM): one `[behavior.one_shot]` block, no per-key override.
   *Rejected* — too rigid for the stated need.
2. **Sentinel fallback** (SK `timeout_ms = 0` today → inherit global). Works for a
   numeric field with a spare sentinel, but can't express "inherit" for a `bool`,
   and `max_repeat = 0` already means "infinite" so `0` is taken there.
3. **Option-field merge** (morse / tap-hold profiles): a per-key named profile
   whose `Option<T>` fields override the global default *field by field*; unset
   fields inherit. Most general, reads naturally in TOML, and is a pattern RMK
   users and maintainers already recognize.

**Chosen: pattern 3 — Option-field merge, mirroring morse profiles.**

### TOML surface (additive)

`[behavior.sticky_key]` gains the full set of SK defaults; a new
`[behavior.sticky_key.profiles.<name>]` subtable defines named overrides:

```toml
[behavior.sticky_key]                    # global default for every SK key
timeout = "5s"
max_repeat = 0                           # 0 = infinite
exit_on_layer_change = false

[behavior.sticky_key.profiles.tabber]    # overrides only what it names
max_repeat = 3
exit_on_layer_change = true              # timeout inherited from the global default
```

Purely additive: an existing config that sets only `timeout` keeps working; the
new default fields and the `profiles` table are optional.

### Keymap DSL (reuses MT/LT/TH's optional profile slot)

The bare form is unchanged; an optional trailing **profile name** stands in for
the positional numeric tail:

```toml
SK(Tab, [LAlt])            # every setting from the global default
SK(Tab, [LAlt], tabber)    # override per profile "tabber", inherit the rest
```

This reuses the same optional 3rd positional slot that `MT` / `LT` / `TH` already
use for their morse profile, so it introduces no new DSL shape. The parser
distinguishes the slot by token kind: a **numeric** token keeps the legacy
positional `SK(key,[mods],max_repeat,timeout_ms,exit)` parse (so the 11 existing
SK tests stay green, unmodified); an **identifier** token is resolved as a
profile name.

### Resolution — codegen-time merge, no wire change

Because both the global defaults and the named profiles are compile-time TOML,
the merge happens entirely at **codegen** — exactly as morse's `expand_profile`
bakes resolved values. For each field the resolution order is:

> explicit per-key value (positional arg, if present) →
> named-profile field (if `Some`) →
> `[behavior.sticky_key]` global default (if set) →
> built-in default (`timeout` sentinel `0`, `max_repeat` `0`, `exit` `false`).

The codegen folds this down to concrete values and emits the existing
`sk!(key, mods, max_repeat, timeout_ms, exit)` macro. Therefore:

- **`StickyKeyAction` is unchanged** — still all-concrete `{ key, keep,
  max_repeat, timeout_ms, exit_on_layer_change }`. No `Option` fields reach the
  wire; `MaxSize` and the postcard encoding are untouched.
- **The SK runtime engine is unchanged** by this feature — it still receives one
  fully-resolved action. The profile indirection is a zero-runtime-cost
  compile-time convenience.

### Tests

The legacy positional form keeps its 11 tests unmodified (parity oracle). Add a
small set of **new** codegen/resolution tests for the profile form: bare key
inherits all global defaults; a profile overrides only its named fields and
inherits the rest; a missing global default falls to the built-in default;
numeric-vs-identifier slot disambiguation.

---

## Section 6 — Risks & non-goals

**Risks (ranked):**

1. **OSM timeout semantics shift (Stage 1).** Moving from blocking inline
   `select` to the run-loop deadline changes *when* expiry is observed relative
   to an incoming event. Mitigated by: isolated to one commit, the 25 OSM/OSL tests,
   and a possible targeted race test. This is the one to watch on real hardware.
2. **`unprocessed_events` removal.** Only safe if OSM/OSL are its sole producers.
   Mitigated by an explicit grep/audit before deletion; if shared, it stays and
   only the OSM/OSL pushes are removed.
3. **Preset adapter drift.** Risk that an OSM behavioral axis (double-press
   toggle, `activate_on_keypress`, held-promotion, accumulation, `quick_release`)
   is dropped when re-expressed as a preset. Mitigated by the frozen test suite —
   each axis has a named test.
4. **SK profile-merge resolution (Section 5).** Risk that the codegen merge
   resolves a field from the wrong tier (per-key vs. profile vs. global vs.
   built-in), or that numeric-vs-identifier slot disambiguation misreads a token.
   Mitigated by: the merge is pure compile-time logic with no runtime state, the
   legacy positional path is left intact (existing tests pin it), and the new
   resolution tests cover each tier and the slot-kind split. Low blast radius — a
   bad resolve produces a wrong baked constant caught at build/test time, not a
   runtime hazard.

**Non-goals (explicitly out of scope this round):**

- Folding OSL fully into the latch (keeps its own layer calls — documented seam
  only). This is the eventual "everything folds into sticky-key" direction
  HaoboGu wants, deferred to a follow-up.
- Touching the wire format, Via/Vial keycodes, or the OSM/OSL config blocks
  (frozen per Section 1). The SK config block grows additively only (Section 5).
- Extending the per-key profile override to OSM/OSL. OSM stays global-only; the
  new profile mechanism is SK-only this round.
- `OneShotKey` (OSK) — still unsupported, stays a warning (`keyboard.rs:1329`).
- Any new user-facing feature.

---

## File map (anticipated)

**Modify:**

- `rmk/src/keyboard/oneshot.rs` — remove inline `select` timeout; OSM/OSL onto
  deadline (Stage 1); replaced by `StickyLatch` usage (Stage 2).
- `rmk/src/keyboard/sticky_key.rs` — `StickyKeyState` replaced by `StickyLatch`
  (Stage 2); SK becomes a preset adapter.
- `rmk/src/keyboard.rs` — state fields (`osl_state`, `osm_state`,
  `sticky_key_state` → unified latch), deadline race, dispatch arms
  (`1321-1328`), foreign-key hook (`1220-1230`, `1586`), layer-change release
  (`1600` + four spots), `resolve_explicit_modifiers` (`1380-1403`); remove
  `unprocessed_events` producers (pending audit).
- Likely a new shared module (e.g. `rmk/src/keyboard/sticky_latch.rs`) housing
  `StickyLatch` + `Preset`, depending on how Stage 2 shakes out.

**Modify (Section 5 — SK profile override, independent of the merge stages):**

- `rmk-config/src/resolved/behavior.rs` — extend the `[behavior.sticky_key]`
  resolve to read the new default fields (`max_repeat`, `exit_on_layer_change`)
  and a `profiles` map; plus the corresponding raw-TOML config structs.
- `rmk-macro/src/codegen/action_parser.rs` — SK parse path: numeric-vs-identifier
  slot disambiguation, profile lookup, and the codegen-time tier merge that bakes
  concrete values into the existing `sk!(...)` emission.
- New resolution tests for the profile form (alongside the existing SK tests).

**Frozen (do not touch):**

- `rmk-types/src/action/mod.rs` — `Action` variants + wire order, **and the
  `StickyKeyAction` struct** (Section 5 bakes resolved values into the existing
  fields at codegen, so the wire struct stays all-concrete and unchanged).
- `rmk/src/host/via/keycode_convert.rs` — OSM/OSL keycodes.
- `rmk/src/config/behavior.rs` — `OneShotModifiersConfig` + `StickyKeyConfig`
  (both blocks stay).
- `rmk/tests/keyboard_one_shot_test.rs`, `rmk/tests/keyboard_sticky_key_test.rs`
  — the parity oracle (the legacy positional SK form keeps these green unmodified).

---

## Open questions for the implementation plan

- Exact home of `StickyLatch` + `Preset` (new module vs. folded into
  `sticky_key.rs`) — decide during Stage 2.
- Whether the foreign-key hook (3b) is best expressed as one function with a
  preset branch, or two small functions sharing the latch — decide by which reads
  cleaner once the state is merged.
