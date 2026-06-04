# SK / OneShot Parity Catalogue (Stage 0)

This document is the **parity checklist** for the effort to make a single "Sticky Key"
(SK) engine fully absorb one-shot modifier (OSM) and one-shot layer (OSL) behaviors.
It has one row per behavior axis pinned by the existing tests. Every later stage is
graded against this catalogue.

**Source files characterized (verified, not copied from the plan):**

- `rmk/tests/keyboard_one_shot_test.rs` — 25 tests (verified count == 25).
- `rmk/tests/keyboard_sticky_key_test.rs` — 11 tests (verified count == 11).

**Verification method:** each row below was derived by reading the keymap definitions,
the per-test `BehaviorConfig` / `OneShotModifiersConfig` / `StickyKeyConfig`, and the
literal `expected_reports` assertions in the test bodies — not from the plan's prose.

## Shared keymap context

### OSM/OSL keymap (`keyboard_one_shot_test.rs`)

```text
Layer 0: OSM(LShift)        OSL(1)  A  TH(B,C)  OSM(LCtrl)  WM(B, LGui)
Layer 1: OSM(LShift|LCtrl)  No      C  D        E           F
```

Cols by index: `0=OSM(LShift)`, `1=OSL(1)`, `2=A`, `3=TH(B,C)`, `4=OSM(LCtrl)`,
`5=WM(B,LGui)`.

`OneShotConfig` default `timeout = 1000ms`. `OneShotModifiersConfig` fields exercised:
`activate_on_keypress` (default false), `quick_release` (default — see note below).

### SK keymap (`keyboard_sticky_key_test.rs`)

```text
Layer 0: A  B  C  MO(1)  LShift  No
Layer 1: SK(Tab,LAlt,exit=true)  SK(Tab,LCtrl,exit=true)  SK(Tab,LCtrl|LShift,exit=true)  Transparent  Transparent  No
```

SK macro shape used today is **5-positional**:
`sk!(key, mods, max_repeat, per_key_timeout_ms, exit_on_layer_change)`.
Default `StickyKeyConfig { timeout }` (global). Several alternate keymaps exist
(`KEYMAP_MAX_REPEAT`, `KEYMAP_PER_KEY_TIMEOUT`, `KEYMAP_NO_EXIT`).

---

## Step 1 — OSM / OSL test catalogue (25 rows)

| test name | behavior axis | syntax+config used | maps to (new SK shape/setting) |
|---|---|---|---|
| `test_osm_basic_single_behavior` | OSM applies mod to next key then releases | `osm!(LShift)`; default cfg (timeout 1000ms, activate_on_keypress=false). Tap OSM, tap A → `[LShift, A]` then `[0]` | pure-mod SK, terminating-key (3b): SK(LShift) then A emits Shift+A, mod auto-clears |
| `test_osm_timeout` | OSM expires after timeout; next key clean | `OneShotConfig.timeout=100ms`; A pressed at 150ms → `[0, A]` (no Shift) | shared global timeout on pure-mod SK |
| `test_osm_held_behavior` | held past key press; mod stays until OSM release | press OSM, press A (mod held), release A → `[LShift, A]`, `[LShift]`, then release OSM → `[0]` | pure-mod held-promotion (hold past consuming key keeps mod live) |
| `test_osm_multiple_keys` | mod applies only to the next key | tap OSM, tap A (`[LShift,A]`), tap B (`[0,B]` no Shift) | pure-mod single-consume (one terminating key only) |
| `test_osm_rolling_with_tap_hold` | mod ordering: OSM released before key release still applies | press OSM, press B (col 3 `TH(B,C)`, 10ms tap → B), release OSM, release B → `[LShift, B]` | pure-mod ordering / rolling-release: mod sticks through interleaved release |
| `test_osm_combined_modifiers` | two OSM presses accumulate | tap OSM(LShift) col0, tap OSM(LCtrl) col4, tap A → `[LShift\|LCtrl, A]` | pure-mod accumulation (3c): cross-tap mods stack onto one terminating key |
| `test_osm_multiple_osm_with_wm` | accumulation + WM interaction | OSM(LShift)+OSM(LCtrl)+`WM(B,LGui)` col5 → `[LShift\|LCtrl\|LGui, B]` | accumulation merges with WM's own mod (mods union, not overwrite) |
| `test_osm_activate_on_keypress` | mod emitted immediately on OSM press | `activate_on_keypress=true`; tap OSM → `[LShift]` emitted at once, then A → `[LShift, A]`, `[0]` | `activate_on_keypress` setting (pure-mod only — early mod emission) |
| `test_osm_combined_modifiers_with_activate_on_keypress` | accumulate + early activation | `activate_on_keypress=true`; two OSM then A → `[LShift]`, `[LShift\|LCtrl]`, `[LShift\|LCtrl, A]`, `[0]` | accumulation under `activate_on_keypress` (incremental mod reports) |
| `test_osl_basic_single_behavior` | OSL activates layer for next key only | `osl!(1)`; tap OSL, tap col2 → C (layer-1 key), then `[0]` | layer-shape SK (3d): one-shot layer for next key |
| `test_osl_held_behavior` | held across key press; layer stays until release | press OSL, press col2 (→C), release, release OSL → `[C]`, `[0]` | layer held-promotion |
| `test_osl_timeout` | OSL expires; next key on base layer | `OneShotConfig.timeout=100ms`; col2 at 150ms → A (layer 0) | shared global timeout on layer shape |
| `test_osl_multiple_keys` | layer applies only to the next key | OSL, col2→C (layer 1), col3→B (layer 0) | layer single-consume |
| `test_osm_then_osl` | OSM + OSL combine; mod applies to layer-switched key | OSM(LShift), OSL(1), col2 → **`[0, C]`** (C from layer 1). NOTE: report shows **no LShift modifier** — see discrepancy D1 | combined mod-shape + layer-shape ordering |
| `test_osl_then_osm` | OSL + OSM combine | OSL(1), then col0 OSM resolves (layer1 OSM is `LShift\|LCtrl`; col0 layer-1 is OSM(LShift\|LCtrl)), col2 → `[LShift\|LCtrl, A]`. NOTE: emits **both** Shift+Ctrl, not just Shift | layer-then-mod combination; mod set comes from the layer-active OSM |
| `test_osm_and_osl_timeout` | both time out independently | timeout=100ms; col2 at 200ms → `[A]` (layer 0, no mod) | independent expiry of mod-shape and layer-shape under shared timeout |
| `test_osm_chain_mode_basic` | `quick_release=false`: mod held until key RELEASE | `quick_release=false`; tap A → `[LShift, A]`, release A → `[0]` | chain mode: terminating-key holds mod until its release |
| `test_osm_chain_mode_multiple_keys` | chain mode: only first key modified | `quick_release=false`; A → `[LShift,A]`,`[0]`; B → `[0,B]`,`[0]` | chain single-consume |
| `test_osm_chain_mode_activate_on_keypress` | chain + early activation | `activate_on_keypress=true, quick_release=false`; `[LShift]`,`[LShift,A]`,`[0]` | chain mode under `activate_on_keypress` |
| `test_osm_quick_release_basic` | `quick_release=true`: mod released mid key-press | `quick_release=true`; press A → `[LShift,A]`, then **`[0,A]`** (mod dropped while key still held), release → `[0]` | quick-release mode: mod cleared as soon as terminating key registers |
| `test_osm_quick_release_multiple_keys` | quick-release single-consume | `quick_release=true`; A → `[LShift,A]`,`[0,A]`,`[0]`; B → `[0,B]`,`[0]` | quick-release single-consume |
| `test_osm_quick_release_combined_modifiers` | quick-release with accumulated mods | `quick_release=true`; OSM(LShift)+OSM(LCtrl)+A → `[LShift\|LCtrl,A]`,`[0,A]`,`[0]` | quick-release + accumulation |
| `test_osm_quick_release_with_wm` | OSM mod released, WM mod persists | `quick_release=true`; OSM(LShift)+OSM(LCtrl)+`WM(B,LGui)` → `[LShift\|LCtrl\|LGui,B]`, then **`[LGui,B]`** (only OSM mods dropped; WM's LGui stays), `[0]` | quick-release drops only SK-owned mods, leaves WM/other mods intact |
| `test_osm_quick_release_activate_on_keypress` | quick-release + early activation | `activate_on_keypress=true, quick_release=true`; `[LShift]`,`[LShift,A]`,`[0,A]`,`[0]` | quick-release under `activate_on_keypress` |
| `test_osm_quick_release_combined_activate_on_keypress` | quick-release + accumulation + early activation | both flags true; `[LShift]`,`[LShift\|LCtrl]`,`[LShift\|LCtrl,A]`,`[0,A]`,`[0]` | full combination: accumulation + early activation + quick-release |

**Removed upstream (noted in the file, not a discrepancy):** the plan's
`test_osm_quick_release_rolling` does **not** exist — it was intentionally deleted
(comment at `keyboard_one_shot_test.rs:661-662`: "OSM + morse/tap-hold interaction
has a known bug where the OSM deadline loop times out before the tap resolves").
This is why the OSM/OSL file holds 25 tests, not 26. The 25 present all match the
plan's list.

---

## Step 2 — SK test catalogue (11 rows)

For each row, "axis preserved?" states whether the SK engine must keep the axis after
the merge. Rows that **prove the tap-key shape** require `key != No` semantics
(SK actually emits a HID key, not just a modifier) and are flagged **[TAP-KEY PROOF]**.

| test name | behavior axis | syntax+config used | maps to (new SK shape/setting) — preserved? |
|---|---|---|---|
| `test_sk_basic_flow_press_twice` | press sends key+mod; release holds mod; re-press repeats; layer exit cleans up | default keymap `sk!(Tab,LAlt,0,0,true)`; MO↓, SK↓→`[LAlt,Tab]`, SK↑→`[LAlt]`, SK↓→`[LAlt,Tab]`, SK↑→`[LAlt]`, MO↑→`[0]` | **[TAP-KEY PROOF]** tap-key core (key Tab + mod LAlt). Preserved — must keep `key != No` |
| `test_sk_layer_change_cleanup` | `exit_on_layer_change=true` → cleanup on MO release | `sk!(...,true)`; MO↑ produces `[0]` cleanup report | maps to new `release_on_layer_change`. **Behavior change:** new default is `false` (see Accepted changes); this test pins the `=true` path |
| `test_sk_shift_does_not_release_sk` | a real modifier press does NOT release SK; they stack | press LShift (col4 transparent→LShift) between SK presses → `[LCtrl\|LShift,...]`; SK stays active | foreign-key rule **excludes bare modifiers**: pressing a modifier stacks, does not terminate SK. Preserved |
| `test_sk_rapid_three_presses` | three rapid presses each send key+mod | `sk!(Tab,LAlt,...)`; 3×(SK↓→`[LAlt,Tab]`, SK↑→`[LAlt]`) | **[TAP-KEY PROOF]** repeated tap-key emission. Preserved |
| `test_sk_combined_modifiers` | SK with `LCtrl\|LShift` sends both | col2 `sk!(Tab, LCtrl\|LShift, ...)` → `[LCtrl\|LShift, Tab]` | **[TAP-KEY PROOF]** multi-mod tap-key. Preserved |
| `test_sk_timeout` | auto-release after global timeout; next key clean | `StickyKeyConfig.timeout=100ms`; SK↑ then 150ms wait → `[0]`; later C clean | shared global timeout on tap-key SK. Preserved |
| `test_sk_timeout_resets_on_press` | timeout resets on each press | timeout=100ms; SK#1↑ (T1), SK#2 at 50ms cancels T1, SK#2↑ (T2), 150ms→fire | timeout-reset-on-press. Preserved |
| `test_sk_max_repeat` | deactivates silently after `max_repeat=2` (3rd press deactivates) | `KEYMAP_MAX_REPEAT` `sk!(Tab,LAlt,2,0,false)`; press#3 → `[0]`, then A clean | `max_repeat` cycling. Preserved |
| `test_sk_per_key_timeout_overrides_global` | per-key `timeout_ms` overrides global | `KEYMAP_PER_KEY_TIMEOUT` `sk!(Tab,LAlt,0,50,false)`, global=100ms; releases at 50ms | **CAPABILITY DEFERRED.** Per-key timeout override is removed this round. This test must be **re-expressed or retired**: convert to a global-timeout assertion (drop the 50ms positional, assert release at the global 100ms boundary) **or delete with justification**. Flagged here per plan Step 2. |
| `test_sk_exits_on_layer_change` | `exit_on_layer_change=true` | duplicates Test 2's exit=true path (default keymap) → `[LAlt,Tab]`,`[LAlt]`,`[0]` | `release_on_layer_change=true` path. Preserved (explicit) |
| `test_sk_survives_layer_change` | `exit_on_layer_change=false` survives; released only by a key press | `KEYMAP_NO_EXIT` `sk!(...,false)`; MO↑ no report; later A↓ releases SK then sends A → `[0]`,`[0,A]`,`[0]` | new **default** `release_on_layer_change=false`. Preserved as the new default behavior |

---

## Step 3 — New tests required by the spec (do NOT exist yet; author in Stage 2)

| test name (proposed) | behavior axis | syntax+config | maps to (proof) |
|---|---|---|---|
| `test_sk_puremod_terminating_key` | pure-mod SK then a normal key emits mod+key, then mod clears | `sk!` with **no tap key** (pure-mod, e.g. `SK(LGui)`); press P → `[LGui, P]`, then `[0, P]`/`[0]` | **Core 3b proof.** SK(LGui) then P must emit Gui+P. Today's SK engine gets this wrong; today's OSM (`test_osm_basic_single_behavior`) gets it right. This regression test pins the absorbed OSM behavior. |
| `test_sk_puremod_cross_tap_accumulation` | two pure-mod SK taps accumulate onto one key | `SK(LCtrl)` then `SK(LShift)` then P → `[LCtrl\|LShift, P]` | **3c proof.** Mirrors `test_osm_combined_modifiers` but via the SK engine. Pins cross-tap mod accumulation for pure-mod SKs. |

---

## Step 4 — Accepted behavior changes (deltas — NOT regressions)

Reviewers must not mistake these intentional changes for regressions:

1. **Alt-tab SKs gain a default 1s timeout.** Previously a tap-key SK could hold its
   modifier indefinitely (effectively `Duration::MAX` / no timeout); after the merge
   the shared one-shot timeout (default `1000ms`) applies. Behavioral effect: a stuck
   Alt auto-clears after 1s of inactivity.
2. **Default `release_on_layer_change=false`.** Several existing SK tests used the
   default keymap with `exit_on_layer_change=true` (e.g. `test_sk_basic_flow_press_twice`,
   `test_sk_layer_change_cleanup`, `test_sk_exits_on_layer_change`). The new default is
   `false` (SK survives a layer change), matching `test_sk_survives_layer_change`. Tests
   that assert the `=true` cleanup path must opt in explicitly.
3. **`per-key timeout_ms` and the 5-positional `SK(...)` tail are removed.** The current
   macro is `sk!(key, mods, max_repeat, per_key_timeout_ms, exit_on_layer_change)`. The
   per-key timeout positional is dropped (capability deferred), so the positional tail
   shrinks. `test_sk_per_key_timeout_overrides_global` is directly affected (Step 2).

---

## Discrepancies found between the plan and the actual test files

- **D0 — `test_osm_quick_release_rolling` absent (expected).** The plan's preamble said
  "25 tests" but its bullet list and the actual file agree on 25; the 26th
  (`..._rolling`) was deleted upstream with an explanatory comment. No action needed
  beyond noting it. All 25 named tests in the plan exist with matching names.
- **D1 — `test_osm_then_osl` does NOT emit the OSM modifier.** The plan describes it as
  "OSM+OSL combine, mod applies to layer-switched key." The **actual assertion is
  `[0, C]` — no LShift modifier on the layer-switched key C.** The OSM appears to be
  consumed/dropped by the intervening OSL activation rather than carried onto C. The
  catalogue row reflects the real assertion. This is a meaningful parity detail for the
  merge: the SK engine must reproduce this "OSM-then-OSL drops the mod" outcome, or the
  behavior must be explicitly re-decided. **Flagged for design review.**
- **D2 — `test_osl_then_osm` emits `LShift|LCtrl`, not just `LShift`.** The plan row
  says only "OSL+OSM combine." In reality, after OSL(1) the col-0 key resolves to the
  **layer-1** OSM which is `OSM(LShift|LCtrl)`, so the final key A carries **both**
  modifiers (`[LShift|LCtrl, A]`). The catalogue row captures the real mod set.
- **D3 — `test_sk_exits_on_layer_change` is an intentional duplicate of
  `test_sk_layer_change_cleanup`.** Both pin the `exit=true` MO-release cleanup on the
  default keymap; the file's own doc comment acknowledges this ("This is the same as
  Test 2"). Not a problem, but flagged so a reviewer doesn't think one is redundant by
  mistake — both should be migrated to the explicit `release_on_layer_change=true` opt-in.

No test-count or test-name mismatches otherwise: 25 OSM/OSL + 11 SK = 36 existing tests,
matching the plan's "36 existing tests" total.
