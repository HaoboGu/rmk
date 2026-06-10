# DP-4 — SK-absorbs-OneShot: wire / Vial / storage migration findings

**Date:** 2026-06-10
**Branch:** `feat/osm-sticky-key-merge`
**Scope:** Impact of removing `Action::OneShotModifier` / `Action::OneShotLayer` (postcard
discriminant shift) and the `0x5280–0x52BF` Vial keycodes, plus the
`one_shot_timeout → sticky_key_timeout` field rename.

## TL;DR verdict

| Question | Answer |
| --- | --- |
| Manual reflash-with-erase needed? | **No** — firmware self-erases on upgrade. |
| Vial re-sync / migration code needed? | **No** — stale keycodes degrade to `KeyAction::No`, no panic. |
| Storage schema version bump needed? | **No** — `BUILD_HASH` already serves this role and changes every build. |
| Can `from_via_keycode` panic on a stale keycode? | **No** — catch-all arm warns + returns `KeyAction::No`. |
| User-visible consequence | Vial dynamic keymap edits are wiped on the upgrade flash (one-time). |

**Net: this is a safe, zero-code migration in practice.** No defensive code, no schema
field, no documented reflash step is strictly required. The single caveat is the Vial
dynamic-keymap wipe, which is inherent to any RMK firmware update (not specific to this change).

## How the keymap is stored (the load-bearing fact)

The keymap **is** persisted to flash, per-key, and `KeyAction`/`Action` is serialized via
**postcard (discriminant/positional wire format)** — NOT as a Vial u16 keycode.

- Write: `FlashOperationMessage::KeymapKey { action }` → `StorageData::KeyAction(action)`
  → postcard store. `rmk/src/storage/mod.rs:734`, per-key keys at `:556-567`.
- Read at boot: `StorageData::KeyAction(action)` deserialized straight back into the keymap
  array. `rmk/src/host/storage.rs:73-111` (`:96`).
- `from_via_keycode` / `to_via_keycode` (`rmk/src/host/via/keycode_convert.rs:131`, `:5`)
  are **protocol-boundary adapters only** — they are NOT the storage encoder.

Consequence: removing `Action` variants **does** shift postcard discriminants, so old stored
bytes would deserialize to the wrong variant — *if they were ever read against the new layout.*
They are not, because of `BUILD_HASH` (below).

## Why the discriminant shift is harmless: BUILD_HASH

`rmk/build.rs:25-51` computes `BUILD_HASH = crc32(format!("{git_short_commit}_{now_nanos}"))`,
where `now_nanos` is the wall-clock build time. It is written into `constants.rs` and consumed
as `BUILD_HASH` (`rmk/src/storage/mod.rs:28`).

Boot gate (`check_enable`, `rmk/src/storage/mod.rs:634-642`):

```rust
if let Some(StorageData::StorageConfig(config)) = self.fetch_data(StorageKey::StorageConfig).await
    && config.enable
    && config.build_hash == BUILD_HASH { return true; }
false
```

On mismatch (`:458-485`): `flash.erase_all()` then `initialize_storage_with_config(...)` from
the **compiled-in** keymap + behavior defaults. No panic; on init error it stores
`enable: false` to avoid partial init. A regression test already exists:
`build_hash_mismatch_reinitializes_storage` (`:1009`).

Because `BUILD_HASH` embeds both the commit id **and** the build timestamp, the old firmware
(built on `main`) and the new firmware (built on the feat branch) will always have different
hashes. Flashing the new `.uf2` therefore always triggers erase + reinit, so the new firmware
**never reads old-layout postcard bytes**. The discriminant shift is masked by design.

> Edge note: within a single source tree, Cargo caches the build-script output (only
> `rerun-if-changed=build.rs` is declared), so two consecutive rebuilds *without* a change can
> reuse a `BUILD_HASH`. This does not affect the upgrade path — the two firmwares differ in
> source, and any `Action`-layout change forces recompilation. A `cargo clean` (already part of
> the layout cargo-make tasks) regenerates the hash regardless.

## Vial keycode removal (0x5280–0x52BF): no panic

Removed in commit `28416fa57`. The old mappings were
`0x5280..=0x529F → OneShotLayer`, `0x52A0..=0x52BF → OneShotModifier`. The range is now
unhandled and hits the catch-all in `from_via_keycode`
(`rmk/src/host/via/keycode_convert.rs:227-230`):

```rust
_ => {
    warn!("Via keycode {:#X} is not processed", via_keycode);
    KeyAction::No
}
```

- A stale `0x5280` from a live Vial message → `KeyAction::No` (inert) + warn. **No panic.**
- Stale keycodes are **never decoded at boot**: storage reads typed `KeyAction` via postcard,
  not via keycodes, so `from_via_keycode` is only on the live-protocol path
  (`rmk/src/host/via/mod.rs:105`, `vial.rs`).
- The new `Action::StickyKey` is **not** Vial-keycode-representable: `to_via_keycode` returns `0`
  + warn for it (`keycode_convert.rs:88-92`). So SK actions cannot round-trip through the Vial
  desktop app — but this fails safe (no panic; SK keys simply aren't editable/visible in Vial).
  StickyKey is authored via the `sk!`/`sk_mod!`/`sk_layer!` macros (compile-time TOML), not Vial.

## The one_shot_timeout → sticky_key_timeout rename: safe

The `BehaviorConfig` field was renamed to `sticky_key_timeout` (`storage/mod.rs:310`), but the
**wire/setting variant name was deliberately preserved**: `FlashOperationMessage::OneShotTimeout(u16)`
(`:144-145`, comment: *"variant name kept for storage-format stability"*). Its handler updates
the renamed field (`:803-804`). So the persisted setting-key encoding is unchanged — no stored
setting is orphaned by the rename.

## Recommendation

Ship as-is. Optionally:

1. Add one line to the release/upgrade notes: *"Upgrading wipes Vial dynamic keymap
   customizations (flash is re-initialized from firmware defaults)."* — true of all RMK updates,
   worth restating here since OneShot users are the affected cohort.
2. (Optional, low value) A more specific deprecation `warn!` for the `0x5280–0x52BF` range on the
   live path, for user awareness. Not required for correctness.

No code changes are required for a safe migration.
