# Sticky Key Absorbs One-Shot (OSM + OSL) — Design

**Date:** 2026-06-03
**Status:** Designed (not yet implemented)
**Branch:** `feat/osm-sticky-key-merge`
**Repo:** rmk-fork (RMK firmware), consumed by RMKSofleV2 via `[patch.crates-io]`
**Context:** RMK PR [#859](https://github.com/HaoboGu/rmk/pull/859)
**Supersedes:** `2026-06-02-unify-osm-sticky-key-design.md` (kept as the historical
record of the strict-backward-compat approach).

## Overview

The 06-02 design unified the OSM and SK **runtime** while freezing every public
surface for strict backward compatibility, and deliberately left one-shot
**layer** (OSL) on its own path. That strict-compat work has since been merged
into the branch/PR.

This design takes the next step HaoboGu approved in PR #859: **completely replace
one-shot with sticky key.** Sticky Key becomes the single behavior; OSM and OSL
are absorbed into it and cease to exist as independent behaviors. The user-facing
`OSM(...)` / `OSL(...)` keymap forms are replaced by `SK(...)` forms, the three
one-shot/sticky config tables collapse into one, and the runtime is a single
engine whose behavior is selected by the **shape of the SK action**, not by which
legacy syntax was used.

This is intentionally **not** backward compatible — it changes user-facing syntax,
config table names, and default values. Those breaks are accepted: the goal is one
coherent feature set with the maximum code reuse, not preservation of the old
surface.

**Goals:**

1. **One behavior.** A single SK engine and a single `Action::StickyKey` path
   absorb OSM (one-shot modifier) and OSL (one-shot layer). Reuse as much of the
   existing OSM and SK code as possible.
2. **One config table.** Collapse `[behavior.one_shot]`,
   `[behavior.one_shot_modifiers]`, and `[behavior.sticky_key]` into a single
   `[behavior.sticky_key]`.
3. **Keep every capability** of today's OSM, OSL, and SK — modifier accumulation,
   held-promotion, `quick_release`, `activate_on_keypress`, alt-tab cycling
   (`max_repeat`), layer-change release, one-shot layers. Nothing is dropped; the
   features are reorganized, not removed.
4. **Zero cost when unused.** Because there are no per-key overrides and no named
   profiles in this round (deferred — see Section 4), every SK setting resolves to
   a concrete value at codegen and the runtime engine reads only fully-resolved
   values. RAM/flash reflect the simple single-profile model.

**The central idea — behavior is selected by action shape.** A single SK action
takes one of three shapes, and the shape alone decides which behaviors are live:

| Shape | Example | Old equivalent | Engine behavior |
|---|---|---|---|
| **Pure modifier** (`key == No`) | `SK(LGui)` | `OSM(LGui)` | One-shot modifier: honors `activate_on_keypress`/`quick_release`, applies the held mod **to** the terminating key, accumulates across taps. |
| **Tap-key + mods** (`key != No`) | `SK(Tab, [LAlt])` | sticky-mod / alt-tab | Holds mods, taps the key, cycles up to `max_repeat`, releases on a foreign key **without** applying the mod to it. Ignores `activate_on_keypress`/`quick_release`. |
| **Layer** (`SK(MO(n))`) | `SK(MO(1))` | `OSL(1)` | One-shot layer: activates the layer for the next key, then reverts. Reuses OSL's layer activate/deactivate logic. |

This shape-based dispatch is what lets one engine serve all three without
per-key config flags for the OSM↔alt-tab differences (Section 3).

---

## Section 1 — Syntax migration (user-facing)

The `SK(...)` action becomes the single entry point. Migration:

```txt
old: OSM(LGui)                          new: SK(LGui)
old: OSL(1)                             new: SK(MO(1))
old: SK(Tab, [LAlt], 0, 0, true)        new: SK(Tab, [LAlt])
old: SK(Tab, [LCtrl])                   new: SK(Tab, [LCtrl])      # unchanged
```

**Mental model:** *SK makes the wrapped thing one-shot / sticky.*
`SK(LGui)` = sticky modifier; `SK(MO(1))` = sticky momentary-layer (= one-shot
layer); `SK(Tab, [LAlt])` = sticky-mod (alt-tab).

**The trailing positional tail is gone.** The legacy
`SK(key, [mods], max_repeat, timeout_ms, exit_on_layer_change)` form — the only
5-positional-arg action in RMK, with its unreadable `0, 0, true` tail — is
**removed**. `max_repeat`, `timeout`, and layer-release now come from the
`[behavior.sticky_key]` table (Section 4). The bare forms above are the entire
surface.

**`OSM(...)` / `OSL(...)` keymap forms — lowered to `SK(...)` at codegen, then
removed.** To ease migration of existing keymaps, the parser may retain `OSM(m)`
and `OSL(n)` as **deprecated aliases** that lower to `SK(m)` and `SK(MO(n))`
respectively at codegen (zero runtime cost — they produce the identical
`Action::StickyKey`). This is a confirm-or-drop decision (see Open Questions); the
default position is to lower-and-deprecate now, delete later, matching the
"keep for now, simplify once consolidation is proven" approach used for the config.

**OSL payload requires the SK action to carry a layer.** `SK(MO(n))` cannot be
expressed by today's `StickyKeyAction { key, keep, max_repeat, timeout_ms,
exit_on_layer_change }`. The action gains a layer-carrying shape (Section 5). This
is the accepted wire-format break that absorbing OSL requires.

---

## Section 2 — Config consolidation

The three tables collapse into one. **Old:**

```toml
[behavior.one_shot]
timeout = "1s"                  # shared by OSM + OSL

[behavior.one_shot_modifiers]
activate_on_keypress = false
quick_release = false

[behavior.sticky_key]
timeout = "5s"                  # default was: no timeout (Duration::MAX)
max_repeat = 0                  # 0 = infinite
exit_on_layer_change = false
```

**New — a single `[behavior.sticky_key]`:**

```toml
[behavior.sticky_key]
timeout = "1s"                  # default 1s; applies to every SK shape
activate_on_keypress = false    # honored by pure-mod SK only (Section 3)
quick_release = false           # honored by pure-mod SK only (Section 3)
max_repeat = 0                  # 0 = infinite; governs tap-key cycling
release_on_layer_change = false # renamed from exit_on_layer_change
```

**Decisions baked in (all confirmed):**

- **One shared `timeout`** for all shapes, default **1s**. This intentionally
  changes two prior behaviors, both accepted:
  - alt-tab SKs previously had *no* timeout; they now auto-release after 1s of no
    tap.
  - the timeout *action* is uniform — on expiry the latch simply releases.
  A future per-key override / named profile is the planned way to give alt-tab a
  longer timeout without lengthening OSM's window; it is **deferred** this round
  (Section 4).
- **`exit_on_layer_change` → `release_on_layer_change`** (same polarity: `true` =
  layer change releases the SK; `false` = SK survives layer changes). Default
  `false` (survives), matching the preferred behavior for one-shot mods.
- **`max_repeat` is harmless to pure-mod SK.** A pure-mod SK has no tap-key to
  re-press, so cycling never triggers; its termination is the foreign-key path.
  The shared default `0` is therefore safe for OSM-shaped keys.

**Why a single table is correct here.** Of the prior cross-table conflicts, only
`timeout` is a genuine single-default compromise, and it is accepted. The two
transmission fields (`activate_on_keypress`, `quick_release`) are resolved
*structurally* by action shape, not by a config value (Section 3), so they need no
per-key distinction. `max_repeat` and `release_on_layer_change` apply cleanly
across shapes. Nothing forces the tables to stay separate.

---

## Section 3 — The engine model

### 3a. Shape-driven behavior (the unifying rule)

The engine reads the action's shape and applies the matching rules. Two fields,
`activate_on_keypress` and `quick_release`, are **honored only for the pure-mod
shape** and **ignored for the tap-key shape** — this is structural, not
configurable:

| | Pure modifier (`key == No`) | Tap-key (`key != No`) |
|---|---|---|
| transmission on press | per `activate_on_keypress` | **always immediate** |
| `quick_release` | honored | **n/a** |
| termination by foreign key | apply held mod **to** that key, then release | release **without** applying |
| accumulation across taps | yes (`Ctrl` then `Shift` then `P` → `Ctrl+Shift+P`) | no |
| `max_repeat` cycling | n/a (no tap-key) | yes |

**Why ignoring those two fields for tap-key SK is principled, not a hack:**
`activate_on_keypress` means "defer the mod and fuse it into the *next* key." A
tap-key SK has nothing to defer — the tap *is* the action on each press, and you
cannot fold a `Tab` keystroke into a later key. So "deferred" is undefined for the
tap-key shape; immediate transmission is the only coherent behavior. `quick_release`
(consume on the next key's press vs. release) is likewise meaningless when
termination is a foreign key that the mod is not applied to. Both ride the same
`key == No` axis the engine already needs for the termination rule, so honoring
them only in the pure-mod arm costs no new machinery — one branch, reused.

This is also what makes a **single** `[behavior.sticky_key]` profile serve both
OSM and alt-tab correctly on the transmission axes from day one: the
`activate_on_keypress = false` default gives clean OSM chords, while alt-tab keys
auto-force immediate transmission by virtue of having a tap-key. The only residual
single-default compromise is `timeout`.

### 3b. The terminating-key behavior (the real new work)

Today (`keyboard.rs:1220-1232`) a foreign key press releases the SK **before** the
foreign key is processed, so the foreign key is sent **without** the held mod.
That is correct for alt-tab (no Alt on the Enter that picks a window) but **wrong**
for OSM (`SK(LGui)` then `P` must send `Gui+P`). The engine must therefore branch
on shape:

- **pure-mod:** the held mod must remain applied **through** the terminating key's
  report, then release (on that key's press or release per `quick_release`). This
  is OSM's existing "decorate the next key" behavior, now driven from the SK path.
- **tap-key:** unchanged from today — release first, foreign key sent clean.

### 3c. Modifier accumulation (preserve OSM's behavior)

Today a second SK press just increments `repeat_count` and **ignores** the new
mods (`sticky_key.rs:90-102`). OSM instead accumulates (`oneshot.rs:42/59`,
`cur | new`). For the pure-mod shape the engine must accumulate so
`SK(LCtrl)` then `SK(LShift)` then `P` yields `Ctrl+Shift+P`. The tap-key shape
keeps the repeat-count behavior.

### 3d. Layer shape (absorb OSL)

`SK(MO(n))` activates layer `n` as one-shot. The engine reuses OSL's existing
layer activate/deactivate logic (`oneshot.rs:116-161`, `184-193`) but on the
shared latch + the shared deadline/foreign-key plumbing — fully folded in, not the
"documented seam" the 06-02 spec deferred.

### 3e. Shared latch + plumbing (reused from the 06-02 design)

The unification mechanics from the prior design still apply and should be reused:

- **One latch state** replacing `OneShotState<T>` and `StickyKeyState`, carrying
  `mods`, optional `key`, optional `layer`, `phase` (Pressed/Latched/Held),
  `repeat_count`, and `deadline: Option<Instant>`.
- **One timeout mechanism** — the non-blocking deadline raced in the `run()` loop
  (`keyboard.rs:158-185`). Delete the blocking inline `select(timeout, …)` blocks
  in `process_action_osm`/`process_action_osl` (`oneshot.rs:75-93`, `139-152`) and
  the `unprocessed_events` re-queue path (pending the audit that OSM/OSL are its
  only producers). This also fixes the documented OSM "select race."
- **One foreign-key hook** and **one modifier-resolve sink**
  (`resolve_explicit_modifiers`, `keyboard.rs:1380-1403`), now reading one latch.

The "preset" concept from the 06-02 spec is subsumed here: the preset is no longer
a tag carried alongside the action — it is **derived from the action shape**
(`key == No` / `key != No` / layer), which is strictly simpler.

---

## Section 4 — Deferred: per-key overrides & named profiles

Both per-key argument overrides and named `[behavior.sticky_key.profiles.<name>]`
subtables are **out of scope this round.** Rationale: keep the consolidation as
simple as possible, get it working and validated, **then** measure the RAM/flash
impact of adding overrides before committing to them.

Design constraint this imposes: nothing may architecturally depend on per-key
overrides or named profiles existing. Every setting resolves to a concrete value
at codegen from the single global table, and the runtime engine is blind to where
a value came from. Adding overrides/profiles later must be an additive change to
the config-resolve + codegen layers with **no** runtime/engine coupling — and the
known first use is giving alt-tab keys a longer `timeout` than OSM keys.

---

## Section 5 — Action payload (wire shape)

Absorbing OSL forces the SK action to carry a layer, which the current
all-concrete `StickyKeyAction` cannot. The action must represent the three shapes.
Two candidate encodings (decide during implementation):

- **Tagged variant** — `StickyKeyAction` becomes a small enum:
  `Mods { keep, key, max_repeat }` | `Layer { layer }`, sharing the
  table-sourced `timeout` / `activate_on_keypress` / `quick_release` /
  `release_on_layer_change` at runtime.
- **Added optional field** — keep a struct, add `layer: Option<u8>`; `Some`
  marks the layer shape (`key`/`keep` unused), `None` is the mod/tap-key shape
  with `key == No` distinguishing the two.

Either way this is a **postcard wire-order / struct change** that invalidates
stored keymaps in flash and Vial state. Accepted because (a) full replacement is
the goal, and (b) the firmware is reflashed on every change. Migration note for
users: reflash both halves and re-sync Vial after upgrading.

The `OneShotModifier` / `OneShotLayer` `Action` variants are **removed** from the
wire once `OSM(...)`/`OSL(...)` are lowered to `SK(...)` at codegen — there is no
remaining producer. (If the deprecated aliases are kept per Section 1, they still
lower to `Action::StickyKey`; the old variants do not survive.)

---

## Section 6 — Documentation requirement

The pure-mod vs. tap-key shape distinction — and specifically that
`activate_on_keypress` and `quick_release` are **honored only for pure-mod SKs and
silently ignored for tap-key SKs** — MUST be explained clearly and prominently in
the user docs (keymap config reference and the `[behavior.sticky_key]` section).
This is the one piece of "magic" in the model: a setting present in the table that
applies to some SK keys and not others. Leaving it implicit would make tap-key
behavior look like a bug. The docs must state the rule, the rationale (a tap-key
has nothing to defer), and the three-shape table from the Overview.

---

## Section 7 — Staging & tests

The existing OSM/OSL tests (`keyboard_one_shot_test.rs`, 25) and SK tests
(`keyboard_sticky_key_test.rs`, 11) are the **capability oracle** — but unlike the
06-02 design they will **not** all stay byte-for-byte green, because syntax,
config, and defaults change. They are instead the checklist of *behaviors* that
must still exist after migration; each gets re-expressed against the new surface.
Tests run via `cargo nextest`.

- **Stage 0 — Characterize.** Catalogue every behavior the 36 tests pin (one row
  per OSM/OSL/SK axis). This list is the parity contract for the new surface.
- **Stage 1 — Config + parser.** Collapse the three tables into
  `[behavior.sticky_key]` (with the rename); add the `SK(LGui)` / `SK(MO(n))`
  parse paths; lower `OSM`/`OSL` (and remove the legacy 5-positional SK tail).
  Update keymaps/tests to the new syntax. **Gate: rewritten config/parse tests
  green.**
- **Stage 2 — Engine: shape dispatch + absorb OSM.** Single latch; pure-mod path
  with terminating-key application (3b), accumulation (3c), and shape-gated
  `activate_on_keypress`/`quick_release` (3a). Delete the inline `select` and (per
  audit) `unprocessed_events`. **Gate: all OSM-behavior tests green against the new
  syntax; SK tests green.**
- **Stage 3 — Engine: absorb OSL.** Fold `SK(MO(n))` onto the latch reusing the
  layer activate/deactivate logic; `release_on_layer_change` reads the latch.
  **Gate: all OSL-behavior tests green; full suite + `cargo clippy` clean.**
- **Stage 4 — Docs.** Write the Section 6 documentation. **Gate: docs reviewed.**

**New tests:** a targeted test that the pure-mod terminating-key behavior applies
the mod to the consuming key (the OSM-via-SK regression that today's SK engine
gets wrong), and one for cross-tap accumulation on the pure-mod shape.

---

## Section 8 — Risks & non-goals

**Risks (ranked):**

1. **Terminating-key semantics (3b).** Making the held mod survive *through* the
   foreign key for pure-mod SK while still dropping it for tap-key SK is the core
   behavioral change and the easiest to get subtly wrong (off-by-one on
   press/release ordering). Mitigated by the dedicated regression test and the
   re-expressed OSM suite.
2. **OSM timeout-semantics shift.** Moving OSM off the blocking inline `select`
   onto the run-loop deadline changes *when* expiry is observed relative to an
   incoming event (carried over from the 06-02 risk list). Watch on real hardware.
3. **Wire/struct break (Section 5).** Invalidates stored keymaps + Vial. Accepted,
   but must be called out in release notes with the reflash/re-sync migration step.
4. **`unprocessed_events` removal.** Only safe if OSM/OSL were its sole producers;
   grep/audit before deleting.
5. **Behavior loss during re-expression.** Any OSM/OSL axis (double-press un-latch,
   held-promotion, accumulation, `quick_release`, `activate_on_keypress`, layer
   one-shot) could be dropped when re-homed into the SK engine. Mitigated by the
   Stage 0 catalogue used as the parity checklist.

**Non-goals (explicitly out of scope this round):**

- Per-key argument overrides and named `profiles` subtables (Section 4 —
  deferred, pending RAM/flash measurement).
- Separate timeouts for OSM vs. alt-tab keys (the deferred override is the planned
  mechanism; this round uses one shared `timeout`).
- `OneShotKey` (OSK) — still unsupported, stays a warning.
- Preserving the old `OSM`/`OSL`/legacy-positional-`SK` surfaces beyond the
  optional deprecated lowering aliases (Section 1).

---

## File map (anticipated)

**Modify — config:**

- `rmk-config/src/lib.rs` — replace `StickyKeyConfig { timeout }` and the
  one-shot config structs with the unified `[behavior.sticky_key]` shape
  (`timeout`, `activate_on_keypress`, `quick_release`, `max_repeat`,
  `release_on_layer_change`); remove `[behavior.one_shot]` /
  `[behavior.one_shot_modifiers]`.
- `rmk/src/config/behavior.rs` — collapse `OneShotConfig` +
  `OneShotModifiersConfig` + `StickyKeyConfig` into one resolved config; new
  defaults (Section 2).

**Modify — parser/codegen:**

- `rmk-macro/src/codegen/action_parser.rs` — `SK(LGui)` (pure-mod), `SK(MO(n))`
  (layer), `SK(key,[mods])` parse; remove the 5-positional tail; lower
  `OSM`/`OSL` to `SK` (or drop them per Open Questions).
- `rmk/src/layout_macro.rs` — update/remove the `SK(...)` macro arms for the new
  shapes; layer-carrying payload.

**Modify — engine:**

- `rmk/src/keyboard/sticky_key.rs` — `StickyKeyState` → unified latch; shape
  dispatch (3a), terminating-key application (3b), accumulation (3c), layer shape
  (3d).
- `rmk/src/keyboard/oneshot.rs` — absorb OSM/OSL logic into the latch; delete the
  inline `select` timeout blocks; this file likely shrinks to nothing or merges
  into `sticky_key.rs`.
- `rmk/src/keyboard.rs` — state fields (`osm_state`, `osl_state`,
  `sticky_key_state` → one latch), deadline race, dispatch arms, foreign-key hook
  (`1220-1232`, `1586`), layer-change release (`1600` + spots),
  `resolve_explicit_modifiers` (`1380-1403`); remove `unprocessed_events`
  producers (pending audit).

**Modify — wire:**

- `rmk-types/src/action/mod.rs` — `StickyKeyAction` gains the layer shape
  (Section 5); remove `OneShotModifier` / `OneShotLayer` variants.
- `rmk/src/host/via/keycode_convert.rs` — drop OSM/OSL keycode mappings.
- `rmk/src/storage/mod.rs` — `one_shot_timeout` persisted field → the unified
  config; Vial one-shot-timeout handling.

**Modify — docs/tests:**

- User docs — Section 6 documentation requirement.
- `rmk/tests/keyboard_one_shot_test.rs`, `keyboard_sticky_key_test.rs` —
  re-expressed against the new surface; add the 3b/3c regression tests.

---

## Open questions for the implementation plan

1. **Keep `OSM(...)`/`OSL(...)` as deprecated lowering aliases, or drop the syntax
   outright?** Default: keep-and-lower now, delete later (low cost, eases keymap
   migration). Confirm before Stage 1.
2. **Action payload encoding (Section 5):** tagged variant vs. added
   `layer: Option<u8>` — decide by which keeps the engine dispatch cleanest once
   the latch is merged.
3. **Home of the unified latch** — fold `oneshot.rs` into `sticky_key.rs`, or a
   new shared module — decide during Stage 2.
4. **Vial one-shot-timeout control** — does the unified `timeout` keep a Vial
   runtime-set path, or is that dropped with the OSM keycodes? Decide in Stage 1.
