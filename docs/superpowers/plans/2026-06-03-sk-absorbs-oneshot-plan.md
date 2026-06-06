# Sticky Key Absorbs One-Shot (OSM + OSL) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a single Sticky Key (SK) engine and a single `Action::StickyKey` path fully absorb one-shot modifier (OSM) and one-shot layer (OSL), replacing the `OSM(...)`/`OSL(...)` keymap syntax and the three one-shot/sticky config tables with one `SK(...)` surface and one `[behavior.sticky_key]` table. Behavior is selected by the **shape** of the SK action (pure-mod / tap-key / layer), not by which legacy syntax was used.

**Architecture:** One latch state replaces `OneShotState<ModifierCombination>` (OSM), `OneShotState<u8>` (OSL), and the `StickyKeyState` enum. The engine reads the action's shape — `key == No` → pure-mod (OSM behavior, applies mod *through* the terminating key, accumulates), `key != No` → tap-key (alt-tab, releases clean), layer payload → one-shot layer (OSL behavior). Timeout moves entirely onto the existing non-blocking run-loop deadline race (`keyboard.rs:144-185`); the blocking inline `select(timeout, …)` blocks in `oneshot.rs` are deleted. This is a **deliberate, non-backward-compatible** replacement: user-facing syntax, config table names, defaults, and the wire/struct shape all change.

**Tech Stack:** Rust `#![no_std]` firmware (RMK fork). Config: `rmk-config` (serde + pest grammar) → `rmk-config/src/resolved` → `rmk-macro` codegen → `rmk` runtime structs. Wire: `rmk-types` (postcard via `MaxSize`/`Serialize`/`Deserialize`). Tests: `cargo nextest` with `embassy-time` MockDriver (virtual time, per-test process isolation required).

**Test command (memorize — every gate uses it):**
```sh
# from rmk-fork/rmk/
cargo nextest run --no-default-features --features=split,vial,storage,async_matrix,_ble
# full feature matrix (run before declaring the whole feature done):
sh scripts/test_all.sh   # from repo root
```
`cargo test` will **abort at startup** by design (`rmk/tests/common/mod.rs:30-43` `require_nextest`). Always use nextest.

**Repo / branch:** All work happens in `/mnt/c/RandomProjects/GitHubRepoProjects/rmk-fork` on branch `feat/osm-sticky-key-merge`. This is consumed by RMKSofleV2 via `[patch.crates-io]`. **Do not push toward PR #859.** The branch merges into the sticky-mod PR *only after* the full local suite passes **and** the user confirms it works on real hardware.

---

## Decision Points (resolve these explicitly — do not silently pick)

These are the spec's four open questions (Section 5 / "Open questions for the implementation plan"). Each is wired into a specific stage below. Where a stage reaches one, **STOP, state the trade-off, record the choice in this plan file (check the box + write the decision inline), and only then proceed.** Recommendations are given but are not final until confirmed.

- **DP-1 — Action payload encoding (resolve in Stage 2, Task 2.1).** `StickyKeyAction` must carry an optional layer (OSL) which today's all-concrete struct cannot. Two encodings:
  - *(a) Tagged enum:* `StickyKeyAction` becomes `enum { Mods { keep, key, max_repeat }, Layer { layer } }`.
  - *(b) Added optional field:* keep the struct, add `layer: Option<u8>`; `Some` = layer shape, `None` = mod/tap-key shape with `key == No` distinguishing pure-mod from tap-key.
  - **Recommendation:** (b) added `layer: Option<u8>` — smaller diff to the existing struct, derives (`MaxSize`/`Serialize`/`Deserialize`/`Schema`) carry over unchanged, and the engine already needs the `key == No` branch for the terminating-key rule, so the three-way match is `(layer, key)` → cheap. Revisit if the engine dispatch reads cleaner as an enum once the latch is merged.
  - **DECISION (2026-06-04, confirmed by user):** **(b) — add `layer: Option<u8>`.** Grounding: every action-parameter type in rmk-types is a plain struct (`Combo`, `Morse`, `Fork`, `EncoderAction`, `StickyKeyAction`); none is a tagged enum — only the top-level `Action` is. Keeping `StickyKeyAction` a struct matches that convention and `MorseProfile`'s `Option<...>` sub-setting pattern. The engine currently reads `params.{key,keep,...}` as plain field accesses; adding one optional field keeps those and adds a single `(layer, key)` dispatch, whereas an enum would force every access site to a `match` (larger, riskier diff). Both encodings break the postcard wire identically (DP-4), so the struct option is strictly the lower-churn / lower-bug-risk path. Field set: `{ key: KeyCode, keep: ModifierCombination, layer: Option<u8> }` — `Some(n)` = layer shape; `None` + `key == No` = pure-mod; `None` + `key != No` = tap-key.

- **DP-2 — Home of the unified latch (resolve in Stage 2, Task 2.1).** Fold `oneshot.rs` into `sticky_key.rs`, or create a new shared module (e.g. `keyboard/latch.rs`).
  - **Recommendation:** fold into `sticky_key.rs` and delete `oneshot.rs`. The spec's file map predicts `oneshot.rs` "likely shrinks to nothing or merges into `sticky_key.rs`." A new module name would orphan the established `sticky_key` mod path used across `keyboard.rs`.
  - **DECISION (2026-06-06, confirmed by user):** **Fold into `sticky_key.rs`; delete `oneshot.rs`.** The unified latch lives in `sticky_key.rs` (Task 2.1 declares the latch state there). `oneshot.rs` is deleted as its logic is ported out — OSM removed in Task 2.4, OSL removed and the file + `mod oneshot;`/`use` deleted in Task 3.1. Grounding: the surviving public surface is already `sticky_key`-named (`Action::StickyKey`, `sticky_key_state` field, `sticky_key_config`, `process_action_sticky_key`, `release_sticky_key_if_active`) and `keyboard.rs` references that path throughout; keeping it and dropping `oneshot` is the lowest-churn path and matches the user-facing `SK(...)` naming. A neutral `keyboard/latch.rs` would force every `sticky_key::` site and the `mod`/`use` lines to churn to `latch::` for zero behavioral gain, and would add a third module name to a feature whose whole point is collapsing to one engine.

- **DP-3 — Vial one-shot-timeout runtime path (resolve in Stage 1, Task 1.4).** The unified `timeout` either keeps a Vial runtime-set path (`SettingKey::OneShotTimeout = 0x06`, handlers at `rmk/src/host/via/vial.rs:127-130` and `184-186`, storage `FlashOperationMessage::OneShotTimeout` at `rmk/src/storage/mod.rs:145`) or drops it with the OSM keycodes.
  - **Recommendation:** **keep** the Vial setting wire-compatible but re-point it at the unified `sticky_key` timeout (rename the internal `one_shot_timeout` storage field / accessor to `sticky_key_timeout`, leave `SettingKey` numeric value `0x06` and the protocol bytes unchanged). Dropping a Vial `SettingKey` is itself a Vial-protocol break; this round we are already breaking the keymap wire (DP-4) and should not stack a second protocol break unless the user wants it. Confirm with user.
  - **DECISION (2026-06-04, confirmed by user):** **KEEP, re-pointed.** Leave `SettingKey::OneShotTimeout = 0x06` and its protocol bytes unchanged; rename the internal storage field/accessor from `one_shot_timeout` → `sticky_key_timeout` so Vial still live-sets the unified timeout. Zero Vial-protocol break (the keymap-wire break in DP-4 stays the only one this round). Chosen explicitly for minimum breakage.

- **DP-4 — Wire/Vial/storage migration impact (resolve in Stage 5, post-engine).** The `StickyKeyAction` struct/postcard change plus removal of `Action::OneShotModifier`/`OneShotLayer` variants is a wire-order break. Whether it invalidates keymaps stored in flash and Vial state — and what migration is needed (reflash? Vial re-sync? storage schema bump?) — is **TBD after the engine works**, likely only visible during hardware testing. **Do not assume harmless.** Stage 5 has an explicit evaluation task; the finding must be recorded before any move toward PR #859.

---

## File Map (re-verified against `feat/osm-sticky-key-merge`, 2026-06-03)

Line numbers below are current as of this plan. Re-confirm with a `grep`/Read immediately before editing each file — surrounding edits in earlier stages will shift them.

**Config — TOML structs & resolve & codegen:**
- `rmk-config/src/lib.rs` — `BehaviorConfig` (570-580), `OneShotConfig` (614-616, `timeout`), `OneShotModifiersConfig` (621-624, `activate_on_keypress`, `quick_release`), `StickyKeyConfig` (629-632, `timeout`). TOML tables `[behavior.one_shot]`, `[behavior.one_shot_modifiers]`, `[behavior.sticky_key]`.
- `rmk-config/src/resolved/behavior.rs` — `one_shot_timeout_ms`, `one_shot_modifiers`, `sticky_key_timeout_ms` (4-17); extraction at 105-110 and 205.
- `rmk-config/src/keymap.pest` — `osm_action` (58), `osl_action` (73), `sk_action` (112-120), `key_action` integration (127).
- `rmk-config/src/layout.rs` — pest AST match arms: `osm_action` (391-397), `sk_action` (399-402), `osl_action` (423-428).
- `rmk-macro/src/codegen/behavior.rs` — `expand_one_shot` (25-39), `expand_one_shot_modifiers` (41-65), `expand_sticky_key` (67-79).
- `rmk-macro/src/codegen/action_parser.rs` — `parse_key` (152); `osl(` arm (201-206), `osm(` arm (207-225), `sk(` arm (226-289); `parse_modifiers` helper (51-85).

**Runtime config:**
- `rmk/src/config/behavior.rs` — `BehaviorConfig` (11-22), `OneShotConfig` (62-75, default 1s), `OneShotModifiersConfig` (77-83), `StickyKeyConfig` (85-97, default `Duration::MAX`).
- `rmk/src/keymap.rs` — `one_shot_timeout()` (511), `sticky_key_timeout()` (515), `set_one_shot_timeout()` (561).

**Macros (declarative):**
- `rmk/src/layout_macro.rs` — `osl!` (328-332), `osm!` (352-356), `sk!` (367-379).

**Engine:**
- `rmk/src/keyboard/sticky_key.rs` — `StickyKeyState` enum (24-36: `None | Active { mods, repeat_count, max_repeat, exit_on_layer_change, deadline }`); helpers `value`/`is_active`/`deadline`/`exit_on_layer_change` (38-66); `process_action_sticky_key` (69-125, repeat-count increment 90-102); `release_sticky_key_if_active` (127-133).
- `rmk/src/keyboard/oneshot.rs` — `OneShotState<T>` enum (10-20: `Initial/Single/Held/None`); `process_action_osm` (33-114, accumulation `cur | new` at 42/59/62, inline `select` 75-93, `unprocessed_events.retain` 49); `process_action_osl` (116-161, activate 119-133, inline `select` 139-152, `unprocessed_events.push` 148, deactivate via `update_osl` 184-193); `update_osm` (165-182).
- `rmk/src/keyboard.rs` — mod decls `oneshot` (44) / `sticky_key` (47), `use` imports (31-32); state fields `osl_state` (217), `osm_state` (220), `sticky_key_state` (223); `run()` loop + deadline race (144-185); action dispatch `OneShotLayer`/`OneShotModifier`/`StickyKey` (1316-1328); foreign-key release (1220-1230); layer-change release calls (`exit_on_layer_change()` at 1241, 1250, 1268, 1276, 1600); `resolve_explicit_modifiers` (1380-1403); `unprocessed_events` consumer (148-150) and **non-OSM producer** Clear Peer BLE (1683, `#[cfg(feature = "split")]`).

**Wire / Via / storage:**
- `rmk-types/src/action/mod.rs` — `StickyKeyAction` struct (34-50: `key`, `keep`, `max_repeat`, `timeout_ms`, `exit_on_layer_change`); `Action` variants `OneShotLayer(u8)` (83), `OneShotModifier(ModifierCombination)` (85), `OneShotKey(KeyCode)` (87), `StickyKey(StickyKeyAction)` (98). Derives `Serialize, Deserialize, MaxSize, defmt::Format, Schema`.
- `rmk/src/host/via/keycode_convert.rs` — `to_via_keycode` OSL (61-64) / OSM (65-69); `from_via_keycode` OSL (188-192) / OSM (193-197); unit tests (280, 287).
- `rmk/src/storage/mod.rs` — `BehaviorConfig` persisted struct (301-316, `one_shot_timeout: u16` at 310); serialize (336); deserialize (514); `FlashOperationMessage::OneShotTimeout(u16)` (145); handler (803-804).
- `rmk-types/src/protocol/vial.rs` — `SettingKey::OneShotTimeout = 0x06` (101).
- `rmk/src/host/via/vial.rs` — `GetBehaviorSetting` OneShotTimeout (127-130); `SetBehaviorSetting` (184-186).

**Tests / docs:**
- `rmk/tests/keyboard_one_shot_test.rs` — 25 tests (catalogued in Stage 0).
- `rmk/tests/keyboard_sticky_key_test.rs` — 11 tests (catalogued in Stage 0).
- `rmk/tests/common/mod.rs` — `require_nextest` (30-43), `run_key_sequence_test`; `rmk/tests/common/test_macro.rs` — `key_sequence_test!` (10).
- User docs — keymap config reference + `[behavior.sticky_key]` section (Stage 4 finds exact path).

---

## Stage 0 — Characterize (the capability oracle / parity contract)

**Goal:** Produce a written behavior catalogue — one row per OSM/OSL/SK axis the 36 existing tests pin. This list is the parity checklist every later stage is graded against. No code changes.

### Task 0.1: Write the parity catalogue document

**Files:**
- Create: `docs/superpowers/plans/sk-oneshot-parity-catalogue.md`

- [ ] **Step 1: Catalogue OSM/OSL tests.** Open `rmk/tests/keyboard_one_shot_test.rs` and for each of the 25 tests write a row: `test name | behavior axis | syntax+config used | which new SK shape/setting it maps to`. The 25 (verified) are:

  - `test_osm_basic_single_behavior` (76) — OSM applies mod to next key then releases → pure-mod SK terminating-key.
  - `test_osm_timeout` (108) — OSM expires after timeout; next key clean → shared `timeout`.
  - `test_osm_held_behavior` (151) — held past key press, mod stays until OSM release → pure-mod held-promotion.
  - `test_osm_multiple_keys` (185) — applies only to next key → pure-mod single-consume.
  - `test_osm_rolling_with_tap_hold` (224) — OSM release before key release still applies → pure-mod ordering.
  - `test_osm_combined_modifiers` (255) — two OSM presses accumulate (LShift+LCtrl) → pure-mod accumulation (3c).
  - `test_osm_multiple_osm_with_wm` (291) — multiple OSM + `wm!` accumulate → accumulation + WM interaction.
  - `test_osm_activate_on_keypress` (329) — mod sent immediately on press when enabled → `activate_on_keypress` (pure-mod only).
  - `test_osm_combined_modifiers_with_activate_on_keypress` (366) — accumulate + early activation.
  - `test_osl_basic_single_behavior` (394) — OSL activates layer for next key only → layer shape (3d).
  - `test_osl_held_behavior` (411) — held across key press, layer stays until release → layer held-promotion.
  - `test_osl_timeout` (428) — OSL expires; next key on base layer → shared `timeout` on layer shape.
  - `test_osl_multiple_keys` (456) — applies only to next key → layer single-consume.
  - `test_osm_then_osl` (477) — OSM+OSL combine, mod applies to layer-switched key.
  - `test_osl_then_osm` (496) — OSL+OSM combine.
  - `test_osm_and_osl_timeout` (515) — both time out independently.
  - `test_osm_chain_mode_basic` (546) — `quick_release=false`: mod held until key release.
  - `test_osm_chain_mode_multiple_keys` (567) — chain mode, only first key modified.
  - `test_osm_chain_mode_activate_on_keypress` (592) — chain + early activation.
  - `test_osm_quick_release_basic` (616) — `quick_release=true`: mod released mid key-press.
  - `test_osm_quick_release_multiple_keys` (637).
  - `test_osm_quick_release_combined_modifiers` (665).
  - `test_osm_quick_release_with_wm` (688) — OSM mod released, WM mod persists.
  - `test_osm_quick_release_activate_on_keypress` (711).
  - `test_osm_quick_release_combined_activate_on_keypress` (734).

- [ ] **Step 2: Catalogue SK tests.** Open `rmk/tests/keyboard_sticky_key_test.rs` and add the 11 (verified) rows. Note for each whether the axis is preserved, and which axes prove the **tap-key** shape (so they must keep `key != No` semantics):

  - `test_sk_basic_flow_press_twice` (131) — press sends key+mod; release holds; re-press repeats; layer exit cleans up → tap-key core.
  - `test_sk_layer_change_cleanup` (162) — `exit_on_layer_change=true` cleanup on MO release → `release_on_layer_change`.
  - `test_sk_shift_does_not_release_sk` (196) — a real modifier press does NOT release SK; they stack → foreign-key rule excludes modifiers.
  - `test_sk_rapid_three_presses` (228) — three rapid presses each send key+mod.
  - `test_sk_combined_modifiers` (263) — SK with `LCtrl|LShift` sends both.
  - `test_sk_timeout` (293) — auto-release after global timeout; next key clean.
  - `test_sk_timeout_resets_on_press` (332) — timeout resets each press.
  - `test_sk_max_repeat` (375) — deactivates silently after `max_repeat=2` (3rd press deactivates) → `max_repeat` cycling.
  - `test_sk_per_key_timeout_overrides_global` (414) — per-key `timeout_ms` overrides global. **NOTE:** per-key timeout override is *removed* this round (Section 4 deferred). This test must be **re-expressed or retired** — flag it in the catalogue as "capability deferred; convert to global-timeout assertion or delete with justification."
  - `test_sk_exits_on_layer_change` (444) — `exit_on_layer_change=true`.
  - `test_sk_survives_layer_change` (478) — `exit_on_layer_change=false` survives; released only by key press → new default `release_on_layer_change=false`.

- [ ] **Step 3: Mark the two known new tests required by the spec (Section 7).** Add rows for tests that do **not** exist yet and must be authored in Stage 2:
  - *pure-mod terminating-key regression* — `SK(LGui)` then `P` emits `Gui+P` (today's SK engine gets this wrong; today's OSM gets it right). This is the core 3b proof.
  - *cross-tap accumulation on pure-mod* — `SK(LCtrl)` then `SK(LShift)` then `P` emits `Ctrl+Shift+P` (3c proof).

- [ ] **Step 4: Mark capability deltas (accepted breaks) explicitly.** Add a short "Accepted behavior changes" section so reviewers don't mistake them for regressions:
  - alt-tab SKs gain a default 1s timeout (previously none / `Duration::MAX`).
  - default `release_on_layer_change=false` (was effectively `exit_on_layer_change=true` in several SK tests via the keymap).
  - per-key `timeout_ms` and the 5-positional `SK(...)` tail are removed.

- [ ] **Step 5: Commit.**
```bash
cd /mnt/c/RandomProjects/GitHubRepoProjects/rmk-fork
git add docs/superpowers/plans/sk-oneshot-parity-catalogue.md
git commit -m "docs: characterize OSM/OSL/SK behavior parity catalogue (Stage 0)"
```

---

## Stage 1 — Config + parser (collapse three tables → one; add `SK(LGui)`/`SK(MO(n))`; remove `OSM`/`OSL` and the 5-positional tail)

**Goal:** The build accepts the new `[behavior.sticky_key]` table (with `activate_on_keypress`, `quick_release`, `max_repeat`, `release_on_layer_change`, `timeout`) and the new `SK(...)` parse forms; it **rejects** `OSM(...)`/`OSL(...)` and the legacy 5-positional `SK(...)` with a clear build error. Keymaps/tests are rewritten to the new syntax.

**Gate:** Rewritten config/parse tests green. (Engine still references old state — it will be migrated in Stage 2; keep it compiling by leaving the runtime structs in place but feeding them from the new resolved values where needed, or stub as noted per task.)

> **Ordering note:** Stage 1 changes the wire shape (`StickyKeyAction` gains a layer payload, `OneShotModifier`/`OneShotLayer` variants are removed). That touches the engine's `match` arms in `keyboard.rs:1316-1328`. To keep the crate compiling between Stage 1 and Stage 2, this stage **adds** the new payload shape and parse paths and makes the old `OneShotModifier`/`OneShotLayer` dispatch arms forward to the existing OSM/OSL engine functions *temporarily* (the producers are gone, so they're dead, but they keep types resolved). Stage 2 deletes them. If you prefer, do DP-1 here and thread it forward — but **resolve DP-1 before writing the wire struct (Task 2.1 references it; pull it earlier if needed).**

### Task 1.1: Unified runtime `StickyKeyConfig`

**Files:**
- Modify: `rmk/src/config/behavior.rs:85-97` (and `BehaviorConfig` 11-22)

- [x] **Step 1: Re-read the file** to confirm current line numbers for `OneShotConfig`, `OneShotModifiersConfig`, `StickyKeyConfig`, and `BehaviorConfig`.

- [x] **Step 2: Replace the three config structs with one.** New `StickyKeyConfig`:
```rust
/// Unified sticky-key configuration. Absorbs the former one_shot, one_shot_modifiers,
/// and sticky_key tables. `activate_on_keypress`/`quick_release` are honored only for
/// the pure-modifier SK shape (key == No); see docs.
#[derive(Clone, Copy, Debug)]
pub struct StickyKeyConfig {
    /// Applies to every SK shape. Default 1s.
    pub timeout: Duration,
    /// Honored only by pure-mod SK. Default false.
    pub activate_on_keypress: bool,
    /// Honored only by pure-mod SK. Default false.
    pub quick_release: bool,
    /// 0 = infinite; governs tap-key cycling. Default 0.
    pub max_repeat: u16,
    /// true = a layer change releases the SK. Default false (survives).
    pub release_on_layer_change: bool,
}

impl Default for StickyKeyConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(1),
            activate_on_keypress: false,
            quick_release: false,
            max_repeat: 0,
            release_on_layer_change: false,
        }
    }
}
```

- [x] **Step 3: Update `BehaviorConfig`.** Remove the `one_shot: OneShotConfig` and `one_shot_modifiers: OneShotModifiersConfig` fields; keep only `sticky_key: StickyKeyConfig`. Delete `OneShotConfig` and `OneShotModifiersConfig` struct defs. Fix the `Default` impl of `BehaviorConfig` accordingly.

- [x] **Step 4: Build the config crate.** Run: `cargo build -p rmk --no-default-features --features=split,vial,storage,async_matrix,_ble` and fix any references that read `behavior.one_shot*` (you'll find them in `keymap.rs`, `storage/mod.rs`, the engine — expect failures; resolve only the config-crate-local ones now, defer engine ones to Stage 2 by leaving TODO and a temporary shim if needed). Expected: incremental compile errors that map the blast radius.

- [x] **Step 5: Commit.**
```bash
git add rmk/src/config/behavior.rs
git commit -m "feat(config): collapse one_shot/one_shot_modifiers/sticky_key into unified StickyKeyConfig"
```

### Task 1.2: Unified TOML table + resolve + codegen

**Files:**
- Modify: `rmk-config/src/lib.rs:614-632` (TOML structs), `rmk-config/src/resolved/behavior.rs:4-17,105-110,205`, `rmk-macro/src/codegen/behavior.rs:25-79`

- [ ] **Step 1: TOML struct (`rmk-config/src/lib.rs`).** Delete `OneShotConfig` (614-616) and `OneShotModifiersConfig` (621-624). Replace `StickyKeyConfig` (629-632) with the full surface:
```rust
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StickyKeyConfig {
    pub timeout: Option<DurationMillis>,
    pub activate_on_keypress: Option<bool>,
    pub quick_release: Option<bool>,
    pub max_repeat: Option<u16>,
    pub release_on_layer_change: Option<bool>,
}
```
Remove the `one_shot` and `one_shot_modifiers` fields from the parent `BehaviorConfig` (570-580); keep `sticky_key`.

- [ ] **Step 2: Resolved struct (`rmk-config/src/resolved/behavior.rs`).** Replace `one_shot_timeout_ms` / `one_shot_modifiers` / `sticky_key_timeout_ms` (4-17) with a single resolved shape:
```rust
pub sticky_key_timeout_ms: Option<u64>,
pub sticky_key_activate_on_keypress: Option<bool>,
pub sticky_key_quick_release: Option<bool>,
pub sticky_key_max_repeat: Option<u16>,
pub sticky_key_release_on_layer_change: Option<bool>,
```
Update extraction (was 105-110 one_shot, 205 sticky_key) to read all five fields off `[behavior.sticky_key]`.

- [ ] **Step 3: Codegen (`rmk-macro/src/codegen/behavior.rs`).** Delete `expand_one_shot` (25-39) and `expand_one_shot_modifiers` (41-65). Replace `expand_sticky_key` (67-79) so it emits the full `StickyKeyConfig { timeout, activate_on_keypress, quick_release, max_repeat, release_on_layer_change }` using the Stage-1.1 defaults (1s / false / false / 0 / false) for any `None`. Update the `BehaviorConfig` assembly site that called the three deleted expanders.

- [ ] **Step 4: Build both crates.** Run: `cargo build -p rmk-config && cargo build -p rmk-macro`. Expected: PASS.

- [ ] **Step 5: Commit.**
```bash
git add rmk-config/src/lib.rs rmk-config/src/resolved/behavior.rs rmk-macro/src/codegen/behavior.rs
git commit -m "feat(config): single [behavior.sticky_key] TOML table, resolve, and codegen"
```

### Task 1.3: Parser — add `SK(LGui)`/`SK(MO(n))`, remove `OSM`/`OSL` and 5-positional tail

**Files:**
- Modify: `rmk-config/src/keymap.pest:58,73,112-120,127`, `rmk-config/src/layout.rs:391-428`, `rmk-macro/src/codegen/action_parser.rs:152,201-289`, `rmk/src/layout_macro.rs:328-379`

- [ ] **Step 1: Grammar (`keymap.pest`).** Delete `osm_action` (58) and `osl_action` (73). Rewrite `sk_action` (112-120) to accept the three bare shapes:
```pest
// SK(key, [mods])  | SK(modifier)  | SK(MO(n))
sk_action = {
    ^"SK" ~ "(" ~ (
        layer_action                              // SK(MO(n)) — layer shape
      | (keycode_name ~ "," ~ modifier_keep_list) // SK(key, [mods]) — tap-key shape
      | modifier_combination                      // SK(LGui) — pure-mod shape
    ) ~ ")"
}
```
Remove `osm_action`/`osl_action` from the `key_action` rule (127).

- [ ] **Step 2: pest AST (`layout.rs`).** Delete the `Rule::osm_action` (391-397) and `Rule::osl_action` (423-428) match arms. Keep the `Rule::sk_action` arm (399-402) — it forwards the raw string to codegen — but ensure it no longer assumes the 5-positional shape downstream.

- [ ] **Step 3: codegen parse (`action_parser.rs`).** Delete the `osl(` arm (201-206) and `osm(` arm (207-225). Rewrite the `sk(` arm (226-289) to dispatch on the inner text:
  - inner starts with `MO(` → emit `::rmk::sk_layer!(n)`.
  - inner contains `[` → tap-key: parse `key` + `[mods]` (reuse existing bracket parse and `parse_modifiers`); emit `::rmk::sk!(key, mods)`.
  - else → pure-mod: `parse_modifiers(inner)`; emit `::rmk::sk_mod!(mods)`.
  - If the inner text still contains extra positional args after `]` (the legacy tail), `panic!` with a clear migration message: `"❌ keyboard.toml: the 5-positional SK(...) form is removed; use SK(key, [mods]). max_repeat/timeout/release_on_layer_change now live in [behavior.sticky_key]."`

- [ ] **Step 4: declarative macros (`layout_macro.rs`).** Delete `osl!` (328-332) and `osm!` (352-356). Replace `sk!` (367-379) with three macros matching the new payload (uses DP-1 — pull DP-1 decision here if encoding the layer payload). Example assuming DP-1 recommendation (b), `layer: Option<u8>`:
```rust
#[macro_export]
macro_rules! sk {                       // tap-key shape
    ($key:ident, $keep:expr) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::StickyKey(
            $crate::types::action::StickyKeyAction {
                key: $crate::types::keycode::KeyCode::Hid($crate::types::keycode::HidKeyCode::$key),
                keep: $keep,
                layer: None,
            },
        ))
    };
}
#[macro_export]
macro_rules! sk_mod {                   // pure-mod shape (key == No)
    ($m:expr) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::StickyKey(
            $crate::types::action::StickyKeyAction {
                key: $crate::types::keycode::KeyCode::No,
                keep: $m,
                layer: None,
            },
        ))
    };
}
#[macro_export]
macro_rules! sk_layer {                 // layer shape (OSL)
    ($n:literal) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::StickyKey(
            $crate::types::action::StickyKeyAction {
                key: $crate::types::keycode::KeyCode::No,
                keep: $crate::types::modifier::ModifierCombination::new(),
                layer: Some($n),
            },
        ))
    };
}
```
> The exact `StickyKeyAction` field set depends on **DP-1**. If DP-1 picks the tagged-enum encoding, these three macros emit the three enum variants instead. **Do not write this task until DP-1 is recorded** (it forces the field names used in Stage 2 too).

- [ ] **Step 5: Build.** `cargo build -p rmk-config -p rmk-macro -p rmk`. Expect engine-side errors only (handled Stage 2) — config/parse/macro crates must build clean.

- [ ] **Step 6: Commit.**
```bash
git add rmk-config/src/keymap.pest rmk-config/src/layout.rs rmk-macro/src/codegen/action_parser.rs rmk/src/layout_macro.rs
git commit -m "feat(parser): SK(LGui)/SK(MO(n)) shapes; drop OSM/OSL keywords and 5-positional SK tail"
```

### Task 1.4: Wire `StickyKeyAction` + remove OSM/OSL variants + Vial decision (DP-3)

**Files:**
- Modify: `rmk-types/src/action/mod.rs:34-50,83-98`, `rmk/src/host/via/keycode_convert.rs:61-69,188-197,280-293`, `rmk-types/src/protocol/vial.rs:101`, `rmk/src/host/via/vial.rs:127-130,184-186`, `rmk/src/storage/mod.rs:145,310,336,514,803-804`

- [ ] **Step 1: `StickyKeyAction` (rmk-types).** Apply the DP-1 encoding. For recommendation (b): replace `max_repeat`/`timeout_ms`/`exit_on_layer_change` fields (45-49) with `layer: Option<u8>`, keeping `key` and `keep`:
```rust
pub struct StickyKeyAction {
    pub key: KeyCode,                 // No = pure-mod or layer shape
    pub keep: ModifierCombination,    // unused for layer shape
    pub layer: Option<u8>,            // Some = one-shot layer (OSL) shape
}
```
Keep all derives (`Serialize, Deserialize, MaxSize, defmt::Format, Schema`). Delete the `Action::OneShotLayer` (83) and `Action::OneShotModifier` (85) variants. **Leave `Action::OneShotKey` (87) untouched** (OSK is an explicit non-goal this round — stays a no-op warning).

- [ ] **Step 2: Via keycode_convert.** Delete the OSL/OSM arms in `to_via_keycode` (61-69) and `from_via_keycode` (188-197), and delete/replace the OSL(3)/OSM tests (280, 287). (These map OSM/OSL ranges `0x5280-0x52BF`, which no longer have producers.)

- [ ] **Step 3: DP-3 — Vial one-shot-timeout path.** **STOP. Record the DP-3 decision.** Then:
  - *If keeping (recommended):* rename the storage field `one_shot_timeout` → `sticky_key_timeout` and the keymap accessor `set_one_shot_timeout`/`one_shot_timeout` (`keymap.rs:511,561`) to `sticky_key_timeout`/`set_sticky_key_timeout`, but **leave `SettingKey::OneShotTimeout = 0x06` numeric value and the Vial byte layout unchanged** (rename the enum variant label only if desired; the wire value must not move). Re-point the `vial.rs` Get/Set handlers (127-130, 184-186) at the unified timeout.
  - *If dropping:* delete `SettingKey::OneShotTimeout`, the two `vial.rs` handlers, `FlashOperationMessage::OneShotTimeout` (storage 145, handler 803-804), and the persisted field (310, 336, 514). **This is a second Vial-protocol break — flag it loudly in DP-4's evaluation.**

- [ ] **Step 4: Storage (`storage/mod.rs`).** Per the DP-3 choice, update the persisted `BehaviorConfig` field (310), serialize (336), deserialize (514) to read the unified `sticky_key.timeout`. (Today's deserialize at 514 writes `behavior_config.one_shot.timeout`; re-point to `behavior_config.sticky_key.timeout`.)

- [ ] **Step 5: Build + snapshot check.** `cargo build -p rmk-types -p rmk`. The wire-format change will likely break a postcard/Schema **snapshot test** (the branch has regenerated snapshots before — see commits `3de61454`, `3d8d5723`). If a snapshot test fails, regenerate it deliberately (do not hand-edit) and **note in the commit that the wire format changed** — this is the DP-4 break surfacing early. Run the snapshot regen exactly as the existing CI/scripts do (look for `insta` or a `*_snapshot` test + `cargo insta review` / `INSTA_UPDATE`).

- [ ] **Step 6: Commit.**
```bash
git add rmk-types/src/action/mod.rs rmk/src/host/via/keycode_convert.rs rmk-types/src/protocol/vial.rs rmk/src/host/via/vial.rs rmk/src/storage/mod.rs
git commit -m "feat(wire): StickyKeyAction carries layer payload; remove OneShotModifier/OneShotLayer variants"
```

### Task 1.5: Rewrite config/parse-facing tests to the new syntax

**Files:**
- Modify: `rmk/tests/keyboard_one_shot_test.rs`, `rmk/tests/keyboard_sticky_key_test.rs` (syntax/config only this stage — behavior assertions stay; they'll be the Stage 2/3 gates)

- [ ] **Step 1: Mechanical syntax migration.** In both test files, rewrite keymap macros and configs:
  - `osm!(mods)` → `sk_mod!(mods)`
  - `osl!(n)` → `sk_layer!(n)`
  - `sk!(key, mods, max_repeat, timeout_ms, exit)` → `sk!(key, mods)` (drop the tail; move `max_repeat`/`release_on_layer_change` intent into the `StickyKeyConfig` the test builds)
  - `OneShotConfig { timeout }` / `OneShotModifiersConfig { activate_on_keypress, quick_release }` / `StickyKeyConfig { timeout }` → one `StickyKeyConfig { timeout, activate_on_keypress, quick_release, max_repeat, release_on_layer_change }`.
  - `test_sk_per_key_timeout_overrides_global` (414): per the Stage 0 flag, **delete** it (capability deferred) and add a one-line comment in the file `// per-key timeout removed this round (deferred, spec Section 4); see parity catalogue`.

- [ ] **Step 2: Adjust accepted-break expectations.** Tests that relied on alt-tab having *no* timeout, or SK defaulting to `exit_on_layer_change=true`, must set the config explicitly (`release_on_layer_change: true` where the old test assumed exit-on-change). Use the Stage 0 "Accepted behavior changes" list as the checklist.

- [ ] **Step 3: Compile the test crate only (do not expect green yet).** `cargo nextest run --no-default-features --features=split,vial,storage,async_matrix,_ble --no-run`. Expected: compiles (engine still old → behavior tests may fail at runtime, that's Stage 2/3). If it does not compile, the parser/macro/wire work from 1.1-1.4 has a gap — fix before proceeding.

- [ ] **Step 4: Commit.**
```bash
git add rmk/tests/keyboard_one_shot_test.rs rmk/tests/keyboard_sticky_key_test.rs
git commit -m "test: migrate one-shot/sticky tests to SK(...) syntax and unified config (Stage 1)"
```

**Stage 1 Gate:** Config/parse/macro/wire crates build; test crate compiles; `OSM(...)`/`OSL(...)`/5-positional-`SK(...)` now produce build errors. Behavior tests not yet green (engine pending). Run `cargo build` across the workspace to confirm only the engine `keyboard.rs`/`oneshot.rs`/`sticky_key.rs` arms remain to migrate.

**STAGE 1 COMPLETE (2026-06-05).** Commits: cbb75169 (1.1) · 9ae4008c (1.2) · a24b198a (1.3) · 28416fa5 (1.4) · cefc6e1b (1.5) · f19c4734 (plan DPs). Both per-task reviews (spec + code-quality) passed for every task. Gate status:
- ✅ rmk-config, rmk-macro, rmk-types build clean (rmk-types under `--features host`; snapshots regenerated for the wire-format change — base+bulk Action-carrying endpoints only).
- ✅ Exactly 10 remaining `rmk` lib errors, ALL in engine files (`keymap.rs` ×2, `keyboard/oneshot.rs` ×2, `keyboard/sticky_key.rs` ×4, `keyboard.rs` ×2) under the CI feature set `--no-default-features --features=split,vial,storage,async_matrix,_ble`. These are the Stage 2 migration targets.
- ⚠️ **CARRY-FORWARD D2 — "test crate compiles" deferred to the Stage 2 gate.** The plan assumed the engine still compiled through Stage 1, but Tasks 1.1–1.4 removed the symbols the old engine depends on, so the `rmk` lib (and therefore the test targets) cannot compile until Stage 2. The Task 1.5 test migration was verified at the symbol level only (grep-clean of `osm!`/`osl!`/`OneShotConfig`/`OneShotModifiersConfig`/`one_shot_modifiers`/>2-arg `sk!`; API-surface review; semantic per-key→global remap audited). **Stage 2 gate must compile + run both migrated test files** (`keyboard_one_shot_test.rs`, `keyboard_sticky_key_test.rs`) — that is where the migration is actually validated.
- DP-3 applied (Vial `SettingKey::OneShotTimeout = 0x06` kept byte-identical; internal `one_shot_timeout`→`sticky_key_timeout` rename across keymap/context/vial/storage).

---

## Stage 2 — Engine: shape dispatch + absorb OSM

**Goal:** One latch state; pure-mod path with terminating-key application (3b), accumulation (3c), and shape-gated `activate_on_keypress`/`quick_release` (3a). Delete the inline `select` timeout blocks. Remove the **OSM/OSL producers** of `unprocessed_events` (but **keep** the queue + consumer — the Clear Peer BLE producer at `keyboard.rs:1683` remains; spec Risk #4 audit = NOT sole producers).

**Gate:** all OSM-behavior tests green against the new syntax; all (non-deferred) SK tests green. New 3b + 3c regression tests green.

### Task 2.1: Define the unified latch (resolves DP-1 + DP-2)

**Files:**
- Modify: `rmk/src/keyboard/sticky_key.rs:24-66` (latch state + helpers)
- Decision: DP-1 (payload encoding — must already be recorded from Task 1.4), DP-2 (latch home)

- [x] **Step 1: STOP — record DP-2.** Write the decision (recommended: fold `oneshot.rs` into `sticky_key.rs`, delete `oneshot.rs`) into the Decision Points section above.

- [x] **Step 2: Replace `StickyKeyState`** (enum `None | Active{...}` at 24-36) with the unified latch carrying everything the spec lists (Section 3e): `mods`, optional `key`, optional `layer`, `phase` (Pressed/Latched/Held), `repeat_count`, `deadline: Option<Instant>`. Suggested shape:
```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum SkPhase {
    #[default]
    Pressed,   // SK pressed, not yet consumed
    Latched,   // armed, waiting for the next (foreign) key
    Held,      // promoted to held (key released after another key was used)
}

#[derive(Clone, Copy, Default)]
pub(crate) enum StickyKeyState {
    #[default]
    None,
    Active {
        mods: ModifierCombination,
        key: KeyCode,              // No = pure-mod / layer shape
        layer: Option<u8>,         // Some = OSL shape
        phase: SkPhase,
        repeat_count: u16,
        deadline: Option<Instant>,
    },
}
```
- [x] **Step 3: Re-implement the helper methods** (`value`/`is_active`/`deadline` at 38-66, plus new shape predicates `is_pure_mod()` = `key == No && layer.is_none()`, `is_tap_key()` = `key != No`, `is_layer()` = `layer.is_some()`). `value()` returns the held mods for `resolve_explicit_modifiers`. Replace `exit_on_layer_change()` (it was per-action; now read `release_on_layer_change` from config — the helper becomes a config read in `keyboard.rs`, see Task 2.4).

- [x] **Step 4: Build.** `cargo build -p rmk ...` — expect failures only in the `process_*`/dispatch sites (next tasks). Commit the state shape alone:
```bash
git add rmk/src/keyboard/sticky_key.rs
git commit -m "feat(engine): unified SK latch carrying mods/key/layer/phase/repeat/deadline (DP-1, DP-2)"
```

**TASK 2.1 COMPLETE (2026-06-06).** Commits: `1056db2d` (DP-2 plan record) · `f329ca4e` (unified SK latch — sticky_key.rs only) · `d2a17499` (sentinel fix, see below). Both reviews passed (spec ✅; code-quality ✅ after fixes).
- Latch implemented exactly as specced; `is_pure_mod` requires BOTH `key == Hid(No)` AND `layer.is_none()`. The `"No"` sentinel is `KeyCode::Hid(HidKeyCode::No)` — `KeyCode` has **no bare `No` variant**.
- Code-quality review surfaced a **Stage-1 sentinel defect**: `sk_mod!`/`sk_layer!` in `layout_macro.rs` (and doc prose in `rmk-types/src/action/mod.rs`) emitted the non-existent `KeyCode::No`. Fixed now (commit `d2a17499`) since Task 2.1's predicates define that sentinel contract. This removes one class of the test-crate-compile errors that Task 2.2 must clear (carry-forward D2).
- `is_tap_key` defined as `is_active() && !is_pure_mod() && !is_layer()` (sentinel-independent, per reviewer).
- A `phase()` accessor was **declined** as speculative (YAGNI); Task 2.2 adds accessors when it writes the engine body (same file, zero extra churn).
- Build blast radius after 2.1: exactly the expected consumer-site errors (`process_action_sticky_key` body, `keyboard.rs` `.exit_on_layer_change()`/`Action::OneShot*` arms, `keymap.rs` `one_shot_modifiers`, `oneshot.rs` `one_shot_timeout()`) — all Task 2.2/2.4/3.1 targets. No errors in the new latch/helpers.

**STAGE 2 EXECUTION DECISION (2026-06-06, confirmed by user): combine Tasks 2.2 + 2.3 + 2.4 into one engine-migration work unit.** Rationale: the `rmk` lib does not compile after Stage 1 (carry-forward D2), and the blocking errors are split across 2.2 (`process_action_sticky_key` body), 2.4 (`keyboard.rs` `Action::OneShot*` arms, `.exit_on_layer_change()` calls; `keymap.rs` `one_shot_modifiers`) and 2.4/3.1 (`oneshot.rs` `one_shot_timeout()`). The three tasks all edit the same function and the same `keyboard.rs` sites, and no test can run until all three land — so the per-task TDD gates in 2.2/2.3 are not individually satisfiable (the plan's line-160 assumption that Stage 1 kept the lib compiling failed). They are executed by one implementer to a compiling, fully test-green state (OSM behavior + non-deferred SK + new 3b/3c regressions), followed by a single two-stage review over the combined diff. OSL behavior tests remain failing until Stage 3 (expected). Task sub-sections 2.2/2.3/2.4 below retain their full specs as the combined work unit's checklist.

### Task 2.2: Pure-mod path — accumulation, activate_on_keypress, quick_release, terminating-key application

**Files:**
- Modify: `rmk/src/keyboard/sticky_key.rs` (`process_action_sticky_key`, was 69-125), fold in OSM logic from `oneshot.rs:33-114`
- Modify: `rmk/src/keyboard.rs:1380-1403` (`resolve_explicit_modifiers`), `1220-1230` (foreign-key hook)

- [ ] **Step 1: Write the failing regression test first (3b — terminating-key).** Add to `rmk/tests/keyboard_sticky_key_test.rs`:
```rust
#[test]
fn test_sk_pure_mod_applies_to_terminating_key() {
    // SK(LGui) then P must emit Gui+P (OSM-via-SK; today's SK engine drops the mod).
    key_sequence_test! {
        keyboard: create_test_keyboard_with_config(/* default StickyKeyConfig */),
        sequence: [
            // press+release the SK(LGui) key, then press+release P
            [SK_GUI_ROW, SK_GUI_COL, true,  10], [SK_GUI_ROW, SK_GUI_COL, false, 10],
            [P_ROW, P_COL, true, 10],            [P_ROW, P_COL, false, 10],
        ],
        expected_reports: [
            [KC_LGUI, [kc_to_u8!(P), 0,0,0,0,0]],  // P sent WITH Gui
            [0, [0,0,0,0,0,0]],
        ]
    };
}
```
(Adapt row/col + keymap to the file's existing harness; mirror an existing OSM test's scaffolding.)

- [ ] **Step 2: Run it — verify it fails.** `cargo nextest run ... -E 'test(test_sk_pure_mod_applies_to_terminating_key)'`. Expected: FAIL (mod not applied / wrong report).

- [ ] **Step 3: Implement pure-mod transmission.** In `process_action_sticky_key`, branch on shape. For pure-mod (`key == No && layer.is_none()`):
  - On press: accumulate into the latch (`mods |= params.keep`) — port `cur | new` from `oneshot.rs:42/59/62` (this is 3c). Honor `activate_on_keypress` (config): if true, the mod is emitted immediately (set phase/flag so `resolve_explicit_modifiers` includes it now); if false, defer to the terminating key.
  - Set `deadline` from `config.timeout` (replaces the inline `select`).
  - Port the OSM state transitions (`Initial/Single/Held`) from `update_osm` (`oneshot.rs:165-182`) into the `SkPhase` transitions.

- [ ] **Step 4: Terminating-key application (3b) in `resolve_explicit_modifiers` + foreign-key hook.** In `resolve_explicit_modifiers` (`keyboard.rs:1380-1403`) the latch `value()` already contributes held mods. The new work: for a **pure-mod** active latch, the held mod must remain applied **through** the terminating key's report, then release on that key's press or release per `quick_release`. Port OSM's "decorate the next key then release" from the OSM path. In the foreign-key hook (`keyboard.rs:1220-1230`), branch: pure-mod → do **not** release before the foreign key (apply mod, release after per `quick_release`); tap-key → release first (unchanged, clean foreign key).

- [ ] **Step 5: Run the 3b test + the migrated OSM suite.** `cargo nextest run --no-default-features --features=split,vial,storage,async_matrix,_ble`. Iterate until all `test_osm_*` (basic, held, multiple_keys, rolling, chain_mode_*, quick_release_*) and the new 3b test pass. Use `superpowers:systematic-debugging` on any failure — the parity catalogue says exactly which axis each test pins.

- [ ] **Step 6: Add + pass the accumulation regression (3c).**
```rust
#[test]
fn test_sk_pure_mod_accumulates_across_taps() {
    // SK(LCtrl) then SK(LShift) then P -> Ctrl+Shift+P
    // ... harness ...
    expected_reports: [ [KC_LCTRL | KC_LSHIFT, [kc_to_u8!(P),0,0,0,0,0]], [0,[0,0,0,0,0,0]] ]
}
```
Run; verify green (Step 3 already ported accumulation).

- [ ] **Step 7: Commit.**
```bash
git add rmk/src/keyboard/sticky_key.rs rmk/src/keyboard.rs rmk/tests/keyboard_sticky_key_test.rs
git commit -m "feat(engine): pure-mod SK shape — accumulation, activate_on_keypress/quick_release, terminating-key application (3a/3b/3c)"
```

### Task 2.3: Tap-key path parity (preserve alt-tab) + timeout on run-loop deadline

**Files:**
- Modify: `rmk/src/keyboard/sticky_key.rs` (tap-key branch), `rmk/src/keyboard.rs:144-185` (deadline race already races `sticky_key_state.deadline()`)

- [ ] **Step 1: Implement tap-key branch.** For `key != No`: always immediate transmission (ignore `activate_on_keypress`/`quick_release`), send `keep` mods + `key` on each press, `repeat_count += 1`, deactivate silently when `max_repeat > 0 && repeat_count > max_repeat` (port the existing `sticky_key.rs:90-102` logic), refresh `deadline` from `config.timeout` each press. On a foreign key, release **without** applying the mod (existing behavior).

- [ ] **Step 2: Confirm the deadline race already drives SK timeout.** `keyboard.rs:158-177` already races `self.sticky_key_state.deadline()` and calls `release_sticky_key_if_active()` on expiry. Verify the unified latch's `deadline()` helper returns the right `Option<Instant>` for all shapes (pure-mod, tap-key, layer). No new race machinery — this is the single timeout mechanism the spec wants (Section 3e).

- [ ] **Step 3: Run the SK suite.** `cargo nextest run ...`. Iterate until `test_sk_basic_flow_press_twice`, `test_sk_shift_does_not_release_sk`, `test_sk_rapid_three_presses`, `test_sk_combined_modifiers`, `test_sk_timeout`, `test_sk_timeout_resets_on_press`, `test_sk_max_repeat` pass. (`test_sk_exits_on_layer_change`/`test_sk_survives_layer_change` finish in Task 2.4.)

- [ ] **Step 4: Commit.**
```bash
git add rmk/src/keyboard/sticky_key.rs rmk/src/keyboard.rs
git commit -m "feat(engine): tap-key SK shape parity (alt-tab cycling, max_repeat) on unified latch"
```

### Task 2.4: Delete inline `select`, remove OSM/OSL `unprocessed_events` producers, retire OSM dispatch

**Files:**
- Modify: `rmk/src/keyboard/oneshot.rs` (delete OSM logic + inline `select` 75-93), `rmk/src/keyboard.rs:1316-1328` (dispatch), `1380-1403`, `1241/1250/1268/1276/1600` (layer-change release → config `release_on_layer_change`)

- [ ] **Step 1: Audit `unprocessed_events` (Risk #4) — record the finding.** Verified producers: `oneshot.rs:89` (OSM), `oneshot.rs:148` (OSL), **`keyboard.rs:1683` (Clear Peer BLE, `#[cfg(feature="split")]`)**. Consumer: `keyboard.rs:148-150`. **Conclusion: OSM/OSL are NOT the sole producers — the queue and consumer must STAY for Clear Peer.** Only delete the OSM/OSL push (89, 148) and the OSM `retain` (49). Write this conclusion as a code comment near the consumer so a future reader doesn't re-delete the queue.

- [ ] **Step 2: Delete OSM from `oneshot.rs`.** Remove `process_action_osm` (33-114) including the inline `select(timeout, …)` (75-93) and the `retain` (49), and `update_osm` (165-182). (OSL removal is Stage 3; if folding `oneshot.rs` into `sticky_key.rs` per DP-2, keep OSL temporarily here or move it — your call, but keep it compiling.)

- [ ] **Step 3: Retire the OSM dispatch arm.** In `keyboard.rs:1316-1328`, delete the `Action::OneShotModifier(m)` arm (now an unreachable/removed variant) and the cross-wise `update_osm`/`update_osl` calls tied to it. Remove the `osm_state` field (220) and its `use`/init. `resolve_explicit_modifiers` (1380-1403) now reads only the unified latch (the `osm_state` branch at ~1384-1389 is deleted).

- [ ] **Step 4: Layer-change release → config.** The five `sticky_key_state.exit_on_layer_change()` call sites (1241, 1250, 1268, 1276, 1600) must now read `config.sticky_key.release_on_layer_change` instead of a per-action field (which no longer exists). Replace each `if self.sticky_key_state.exit_on_layer_change()` with `if self.keymap...sticky_key_config().release_on_layer_change` (use the actual config accessor; add one to `keymap.rs` if absent).

- [ ] **Step 5: Run the full suite.** `cargo nextest run --no-default-features --features=split,vial,storage,async_matrix,_ble`. Iterate until **every** OSM-behavior and SK test is green (OSL tests will still fail until Stage 3 — that's expected; note which). Run `cargo clippy --no-default-features --features=... ` and clear warnings in touched files.

- [ ] **Step 6: Commit.**
```bash
git add rmk/src/keyboard/oneshot.rs rmk/src/keyboard.rs rmk/src/keymap.rs
git commit -m "refactor(engine): retire OSM path + inline select; remove OSM/OSL unprocessed_events producers (keep queue for Clear Peer)"
```

**Stage 2 Gate:** All OSM-behavior tests + all (non-deferred) SK tests + the 3b/3c regressions green. Inline `select` gone. `unprocessed_events` queue retained (Clear Peer), OSM/OSL producers removed. Clippy clean in touched files.

**STAGE 2 COMPLETE (2026-06-06).** Executed as one combined work unit (see execution decision above). Commits: `e0a67bfb` (pure-mod 3a/3b/3c) · `e9a0c092` (retire OSM path + inline select; remove OSM/OSL `unprocessed_events` producers, keep queue for Clear Peer) · `35e62c77` (code-quality review fix). Both review gates passed: **spec ✅** (independently verified by code read + test run) and **code-quality ✅ APPROVED**.
- **nextest: 466 run, 461 passed, 5 failed.** The 5 failures are EXACTLY the deferred OSL behavior tests — `test_osl_basic_single_behavior`, `test_osl_held_behavior`, `test_osl_multiple_keys`, `test_osl_then_osm`, `test_osm_then_osl` (Stage 3). All 25 OSM tests, all non-deferred SK tests, and both new regressions (`test_sk_puremod_terminating_key` 3b, `test_sk_puremod_cross_tap_accumulation` 3c) are green. Note: `test_osl_timeout`/`test_osm_and_osl_timeout` pass only because their expected base-layer/no-mod output coincides with the no-OSL-layer result.
- Engine: `process_action_sticky_key` dispatches by shape → `process_sticky_pure_mod` (OSM port: accumulation, `activate_on_keypress`/`quick_release`, terminating-key application via `update_sticky_key` foreign-key hook + `resolve_explicit_modifiers`) and `process_sticky_tap_key` (alt-tab cycling, `max_repeat`). Single timeout = run-loop deadline race.
- OSM fully retired: `process_action_osm`/`update_osm` deleted from `oneshot.rs`; `Action::OneShotModifier`/`OneShotLayer` dispatch arms + `osm_state` field + its `resolve_explicit_modifiers` branch removed; `one_shot_modifiers_config()` → `sticky_key_config()` in `keymap.rs`; five `exit_on_layer_change()` sites read `config.release_on_layer_change`.
- OSL kept compiling for Stage 3 (`process_action_osl`/`update_osl`/`OneShotState` retained; `#[allow(dead_code)]`+TODO on the now-uncalled `process_action_osl`; layer branch in `process_action_sticky_key` is an early-return stub).
- **Scope note:** `keyboard_combo_test.rs` was also migrated off the removed OSM API (Stage 1 left it broken; mechanical `osm!`→`sk_mod!` + config rename, no assertion changes) — needed for the suite to compile.
- **Code-quality review adjudication (controller):** #1 *Held pure-mod spurious timeout* — ACCEPTED & FIXED in `35e62c77` (clear deadline on Held promotion; restores OSM's no-timeout-while-held parity, a real divergence not in the accepted-changes list). #2 *cross-shape latch contamination* — behavior DEFERRED (unspecified concurrent-mixed-shape case; no test/spec); documented with a single-latch-assumption comment. #3 *`repeat_count` u16 overflow* — pre-existing, carried over unchanged; left per surgical-changes rule (noted only).

---

## Stage 3 — Engine: absorb OSL

**Goal:** `SK(MO(n))` activates layer `n` as one-shot on the shared latch + shared deadline/foreign-key plumbing, reusing OSL's activate/deactivate logic. `release_on_layer_change` reads the config.

**Gate:** all OSL-behavior tests green; full suite + `cargo clippy` clean.

**STAGE 3 DESIGN DECISION (2026-06-06, confirmed by user): D1/D2 — PRESERVE existing OSM+OSL combination behavior exactly.** The OSL layer shape lives on the SAME single mutually-exclusive `sticky_key_state` latch (DP-1), not a separate field and not a combined mod+layer latch. Rule: a newly-pressed SK shape that lands on an active latch of a *different* shape REPLACES it — dropping the latched mod (→ D1: `test_osm_then_osl` emits `[0, C]`, no LShift) and/or deactivating the latched layer before applying the new shape (→ D2: `test_osl_then_osm` emits `[LShift|LCtrl, A]` because col0 resolves on the still-active layer 1 to `OSM(LShift|LCtrl)`, then that OSM replaces the OSL latch and deactivates layer 1). Same-shape mod+mod still accumulates (Stage 2 cross-tap behavior, unchanged). This is a pure behavior-preserving refactor: D1/D2 are NOT accepted behavior changes — the 5 OSL tests stay as-written and are the grading contract. Rationale: matches the parity catalogue's "the SK engine must reproduce this outcome" note (D1), keeps the one-engine/one-latch model from DP-1, and is the lowest-risk path to green. A combined-latch "fix" was rejected as scope creep with no anchoring test.

### Task 3.1: Layer shape on the unified latch

**Files:**
- Modify: `rmk/src/keyboard/sticky_key.rs` (layer branch), `rmk/src/keyboard.rs:1316-1328` (dispatch), `rmk/src/keyboard/oneshot.rs` (port OSL activate/deactivate 119-133/184-193, then delete)

- [ ] **Step 1: Run the migrated OSL tests — confirm current failure.** `cargo nextest run ... -E 'test(/osl/) or test(/osm_then_osl/) or test(/osl_then_osm/)'`. Expected: FAIL (no layer handling yet).

- [ ] **Step 2: Implement the layer branch.** In `process_action_sticky_key`, for `layer.is_some()`:
  - On press: activate the layer (`self.keymap.activate_layer(n)`) — port from `oneshot.rs:119-133` (including deactivating a previously-latched OSL layer if any).
  - Arm the latch (phase transitions mirror OSL's `update_osl` at 184-193: deactivate on the Single→consume transition).
  - Set `deadline` from `config.timeout`.
  - On the terminating (foreign) key and on timeout: deactivate the layer, clear the latch. Reuse `release_sticky_key_if_active` so the deadline race (Task 2.3 Step 2) covers layer expiry too.

- [ ] **Step 3: Dispatch.** `keyboard.rs` already routes all `Action::StickyKey` to `process_action_sticky_key` (1326-1328). Delete the `Action::OneShotLayer(l)` arm (1316-1320) and `osl_state` field (217) + its `use`/init + `update_osl`.

- [ ] **Step 4: Delete OSL from `oneshot.rs`.** Remove `process_action_osl` (116-161) including inline `select` (139-152), the `unprocessed_events.push` (148), and `update_osl` (184-193). If `oneshot.rs` is now empty, delete the file and its `mod oneshot;` decl (`keyboard.rs:44`) + `use` (31) per DP-2.

- [ ] **Step 5: Run OSL suite + full suite.** `cargo nextest run --no-default-features --features=split,vial,storage,async_matrix,_ble`. Iterate until `test_osl_*`, `test_osm_then_osl`, `test_osl_then_osm`, `test_osm_and_osl_timeout` all pass **and** nothing earlier regressed.

- [ ] **Step 6: Commit.**
```bash
git add rmk/src/keyboard/sticky_key.rs rmk/src/keyboard.rs rmk/src/keyboard/oneshot.rs
git commit -m "feat(engine): absorb OSL — SK(MO(n)) layer shape on unified latch; delete oneshot.rs"
```

### Task 3.2: Full-suite + clippy gate

**Files:** none (verification task)

- [ ] **Step 1: Full feature matrix.** From repo root: `sh scripts/test_all.sh`. Expected: all green. (If the script enumerates feature combos, every combo must pass — the wire/snapshot changes from Stage 1 may surface here.)

- [ ] **Step 2: Clippy across the workspace.** `cargo clippy --workspace --no-default-features --features=split,vial,storage,async_matrix,_ble -- -D warnings` (match the project's lint invocation if different). Fix warnings in touched files only (per surgical-changes rule).

- [ ] **Step 3: Confirm OSK untouched.** Grep `OneShotKey` — confirm it remains a no-op warning (non-goal this round). No code change; just verify it wasn't accidentally altered.

- [ ] **Step 4: Commit any lint fixes.**
```bash
git add -A
git commit -m "chore: clippy clean + full-suite green after OSM/OSL absorption (Stage 3 gate)"
```

**Stage 3 Gate:** Full suite + full feature matrix + clippy all green. `oneshot.rs` gone (or empty + removed). OSK untouched.

---

## Stage 4 — Docs (Section 6 requirement)

**Goal:** Document the pure-mod vs tap-key shape distinction — specifically that `activate_on_keypress` and `quick_release` are honored **only for pure-mod SKs and silently ignored for tap-key SKs** — prominently in the keymap config reference and the `[behavior.sticky_key]` section. Include the rationale (a tap-key has nothing to defer) and the three-shape table from the spec Overview.

### Task 4.1: Write the docs

**Files:**
- Modify: user docs — locate exact files first (likely `docs/` keymap config reference + a behavior/config page). Grep the repo's docs tree for the old `OSM`/`OSL`/`one_shot` documentation and the `[behavior.sticky_key]`/`[behavior.one_shot*]` sections.

- [ ] **Step 1: Find the doc pages.** `grep -rn "OSM\|OSL\|one_shot\|sticky_key" docs/ *.md` (in rmk-fork). Identify the keymap-action reference and the behavior-config reference pages.

- [ ] **Step 2: Replace OSM/OSL syntax docs with the SK shapes.** Document `SK(LGui)` (pure-mod = old OSM), `SK(MO(n))` (layer = old OSL), `SK(key, [mods])` (tap-key = alt-tab). Add the migration table from spec Section 1.

- [ ] **Step 3: Replace the three config tables' docs with the single `[behavior.sticky_key]`.** Document all five keys (`timeout`, `activate_on_keypress`, `quick_release`, `max_repeat`, `release_on_layer_change`) with defaults (1s / false / false / 0 / false).

- [ ] **Step 4: Add the shape-magic note prominently.** A callout/warning block stating: `activate_on_keypress` and `quick_release` apply **only to pure-mod SKs** (`SK(LGui)`); they are **silently ignored for tap-key SKs** (`SK(Tab, [LAlt])`) because a tap-key has nothing to defer. Include the three-shape table.

- [ ] **Step 5: Note the accepted breaks.** Document that `OSM(...)`/`OSL(...)` and the 5-positional `SK(...)` form are removed (build errors), that alt-tab SKs now have a 1s default timeout, and that `exit_on_layer_change` is renamed `release_on_layer_change` (default false).

- [ ] **Step 6: Commit.**
```bash
git add docs/
git commit -m "docs: SK shapes + unified [behavior.sticky_key]; pure-mod vs tap-key magic-field rule (Section 6)"
```

**Stage 4 Gate:** Docs reviewed (the user reviews; surface the diff). The "magic field" rule is explicit and the three-shape table is present.

---

## Stage 5 — Local verification, hardware testing, and wire/Vial migration evaluation (DP-4)

**Goal:** Run the complete local suite and capture the **DP-4** wire/Vial/storage migration finding before any move toward PR #859. **This stage does not push to the PR.** The user runs hardware testing personally.

### Task 5.1: Full local verification

**Files:** none

- [ ] **Step 1: Full suite, exact command.** From `rmk-fork/rmk/`: `cargo nextest run --no-default-features --features=split,vial,storage,async_matrix,_ble`. From repo root: `sh scripts/test_all.sh`. Capture output. Per `superpowers:verification-before-completion`, paste the real pass/fail counts — no "should pass."

- [ ] **Step 2: Parity audit against Stage 0 catalogue.** Walk the Stage 0 catalogue row by row; confirm each behavior axis has a green test on the new surface (or is explicitly recorded as a deferred capability — only the per-key timeout). List any axis with no covering test and add a test if found missing.

- [ ] **Step 3: Build the consumer firmware.** In RMKSofleV2, the `[patch.crates-io]` points at this fork. Build both layouts to confirm the new syntax/wire compiles end-to-end against a real keymap: `cargo make uf2` (from `/mnt/c/RandomProjects/GitHubRepoProjects/RMKSofleV2`). **The Sofle keymaps use `OSM(...)`/`OSL(...)`? If so they must be rewritten to `SK(...)` first** — grep the `keyboard_*.toml` files and migrate. Expected: 4 `.uf2` files build.

### Task 5.2: DP-4 — evaluate wire/Vial/storage migration impact

**Files:**
- Append findings to: `docs/superpowers/plans/2026-06-03-sk-absorbs-oneshot-plan.md` (this file) or a sibling `sk-oneshot-migration-findings.md`

- [ ] **Step 1: Determine the storage blast radius.** The `StickyKeyAction` struct + `Action` enum changed (postcard wire order; removed variants). Determine whether keymaps stored in flash from a *pre-change* firmware deserialize correctly under the new layout, or are corrupted. Inspect the storage schema/version handling in `rmk/src/storage/mod.rs` — is there a schema-version field that triggers a wipe-on-mismatch? Record: **reflash needed? storage schema bump needed?**

- [ ] **Step 2: Determine Vial state impact.** Per the DP-3 decision: if the Vial `SettingKey::OneShotTimeout` value was preserved, Vial sees no break on that setting; if dropped, Vial loses the setting. Also check whether removed OSM/OSL keycodes (`0x5280-0x52BF`) appear in any stored Vial keymap — if a user's Vial layout referenced them, what happens on load? Record: **Vial re-sync needed?**

- [ ] **Step 3: Write the finding.** Record concretely: (reflash needed Y/N, Vial re-sync Y/N, storage schema bump Y/N, any migration code required). This is the spec's explicit DP-4 requirement and **must exist before the work moves toward PR #859.**

- [ ] **Step 4: Hand off to hardware testing.** Stop here. Report to the user: full local suite results, the parity audit, the uf2 build result, and the DP-4 findings. **The user performs hardware testing.** Do not merge, do not push toward PR #859, do not run `finishing-a-development-branch` until the user confirms hardware works.

- [ ] **Step 5: Commit the findings.**
```bash
git add docs/superpowers/plans/
git commit -m "docs: DP-4 wire/Vial/storage migration findings; local verification complete (pre-hardware)"
```

**Stage 5 Gate:** Full local suite green (with real numbers), parity audit complete, consumer firmware builds, DP-4 findings recorded. **Awaiting user hardware confirmation before any PR movement.**

---

## Self-Review (run against the spec)

**Spec coverage:**
- Section 1 (syntax migration) → Stage 1 Tasks 1.3-1.5. ✔
- Section 2 (config consolidation) → Stage 1 Tasks 1.1-1.2. ✔
- Section 3a (shape dispatch / gated fields) → Stage 2 Task 2.2-2.3. ✔
- Section 3b (terminating-key) → Stage 2 Task 2.2 (+ regression test). ✔
- Section 3c (accumulation) → Stage 2 Task 2.2 (+ regression test). ✔
- Section 3d (absorb OSL) → Stage 3 Task 3.1. ✔
- Section 3e (shared latch/timeout/foreign-key/resolve sink) → Tasks 2.1, 2.3, 2.4. ✔
- Section 4 (deferred overrides/profiles) → respected: per-key timeout test retired (1.5), no profile machinery added; DP-1/config resolve to concrete values. ✔
- Section 5 (action payload) → DP-1 (Task 1.4/2.1); wire variant removal (1.4). ✔
- Section 6 (docs) → Stage 4. ✔
- Section 7 (staging/tests) → Stages 0-5 mirror the spec's Stage 0-4 + a verification stage. ✔
- Section 8 risks: #1 terminating-key (3b test), #2 timeout-shift (Stage 5 hardware watch), #3 wire break (DP-4), #4 unprocessed_events (audited — NOT sole producers, queue kept), #5 behavior loss (Stage 0 catalogue + parity audit). ✔
- All four open questions → DP-1 (2.1), DP-2 (2.1), DP-3 (1.4), DP-4 (5.2). ✔

**Decision-point integrity:** No decision is silently made — DP-1/2/3/4 each have an explicit STOP-and-record step with a stated recommendation that requires confirmation.

**Known re-verification need:** All file:line anchors were re-checked on 2026-06-03 against `feat/osm-sticky-key-merge`, but every editing task re-greps before touching, because earlier-stage edits shift later-stage lines. The most important corrected fact vs. the spec: **`unprocessed_events` has a third (Clear Peer BLE) producer at `keyboard.rs:1683`**, so the spec's "delete `unprocessed_events`" is downgraded to "remove only the OSM/OSL producers; keep the queue" (Task 2.4 Step 1).
