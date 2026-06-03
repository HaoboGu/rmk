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

**Non-goal:** any new user-facing feature. This is internal consolidation plus
the race fix; observable behavior is identical except that the OSM timeout race
goes away.

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
  keeping both is also what compat requires.
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

**New tests:** none for unchanged behavior — the existing suite already defines
parity. Add a test only if the Stage 1 race fix creates a newly-correct behavior
the old suite did not pin (e.g. a key event arriving in the exact timeout window).
If found, that is one targeted regression test, not a suite.

---

## Section 5 — Risks & non-goals

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

**Non-goals (explicitly out of scope this round):**

- Folding OSL fully into the latch (keeps its own layer calls — documented seam
  only). This is the eventual "everything folds into sticky-key" direction
  HaoboGu wants, deferred to a follow-up.
- Touching the wire format, Via/Vial keycodes, or the two config blocks (frozen
  per Section 1).
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

**Frozen (do not touch):**

- `rmk-types/src/action/mod.rs` — `Action` variants + wire order.
- `rmk/src/host/via/keycode_convert.rs` — OSM/OSL keycodes.
- `rmk/src/config/behavior.rs` — `OneShotModifiersConfig` + `StickyKeyConfig`
  (both blocks stay).
- `rmk/tests/keyboard_one_shot_test.rs`, `rmk/tests/keyboard_sticky_key_test.rs`
  — the parity oracle.

---

## Open questions for the implementation plan

- Exact home of `StickyLatch` + `Preset` (new module vs. folded into
  `sticky_key.rs`) — decide during Stage 2.
- Whether the foreign-key hook (3b) is best expressed as one function with a
  preset branch, or two small functions sharing the latch — decide by which reads
  cleaner once the state is merged.
