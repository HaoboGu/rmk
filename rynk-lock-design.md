# Rynk lock gate — design

Status: accepted design, pre-implementation. Protocol is pre-release (v0.1), so staged types and the error enum may change freely; this document uses that freedom once, now, so the shapes are right before the v1 lock-in.

Problem: today nothing in the Rynk protocol is gated. `BootloaderJump`, `StorageReset`, and every config write execute unauthenticated (`rmk/src/host/rynk/handlers/system.rs`), and `GetMatrixState` (0x0802) returns the live key matrix — a polling host is a keylogger. The command table reserves `0x0006..=0x0008` for the lock gate (`rmk-types/src/protocol/rynk/command.rs`), and staged-but-unwired types exist in `rmk-types/src/protocol/rynk/payload/system.rs` (`LockStatus`, `UnlockChallenge`). Vial's in-repo lock (`rmk/src/host/via/vial_lock.rs`) is the prior art: poll-driven physical-presence unlock, shared `Cell` state, `insecure` bypass.

## 1. Threat model

This is a keyboard, not a bank. The attacker we care about is opportunistic software or a radio neighbor, not a soldering iron. The asset ranking:

1. **Firmware replacement** — `BootloaderJump` puts the device in DFU; anything can then flash a permanent implant. Worst outcome by far.
2. **Persistent input injection** — `SetMacro` / `SetKeyAction` / `SetKeymapBulk` plant macros or remaps that type attacker-chosen input later and survive reboot (storage-backed).
3. **Keystroke exfiltration** — polling `GetMatrixState` is a live keylogger that never touches the HID stack. Vial gates its matrix tester behind unlock for exactly this reason.
4. **Destruction / bond tampering** — `StorageReset` wipes config and BLE bonds; `ClearBleProfile` deletes a bond and opens a re-pairing window an attacker can win.
5. **Nuisance** — `Reboot`, `SwitchBleProfile`: transient, no persistence, no data.

Per transport, who can reach the protocol:

| Transport | Reach | Existing boundary | Residual risk the lock must cover |
|---|---|---|---|
| USB CDC-ACM (Web Serial) — `rmk/src/usb/rynk.rs` | Any local process with serial access; any Chromium page the user **once** granted the port (grant persists) | OS device permissions, browser port-grant prompt | A previously-granted page, or any background process, gets full protocol access silently, forever |
| BLE custom GATT (`RynkService`, `rmk/src/ble/ble_server.rs`) | Any central in radio range that completes pairing while the device advertises | Link **encryption is already enforced** — `gatt_events_task` rejects all reads/writes below `SecurityLevel::Encrypted` with `INSUFFICIENT_ENCRYPTION` and drops rynk writes before the RX pipe (`rmk/src/ble/mod.rs`) | Just Works pairing (the default; `passkey_entry` is optional) yields *encrypted but unauthenticated* links. Encryption stops passive sniffing, not an active attacker who simply pairs. |
| WebHID-over-GATT (`RynkHidService`, muxed into the same session, `rmk/src/ble/rynk.rs`) | Rides the OS-bonded HOGP link; browsers with a WebHID grant | OS bond + browser WebHID prompt; same encrypted-link guard as above | Same as BLE custom GATT — one session, one policy |
| Bare UART (`rmk/src/host/rynk/uart.rs`) | Physically wired | Physical access | Out of scope: physical access defeats everything anyway (SWD, bootloader pins). The gate still applies because it lives in dispatch, not in a transport. |

Three honest caveats, stated rather than hidden:

- Config **reads** (keymap, macros, combos) stay open (§2). Stored macros are therefore readable by anything that can reach the protocol — same posture as Vial. Don't put secrets in macros; document this.
- The lock is physical-presence authorization, not cryptography. It proves "the owner's hands are on this keyboard right now", which is the right strength for the assets above.
- Gating `BootloaderJump` is only the *remote-software* half of threat #1. RMK already ships `dfu_lock` (`rmk/src/dfu/mod.rs`), which gates the DFU **download** behind its own `[dfu].unlock_keys` with a 10 s window — protection against flashing once the device is in DFU *by any path*, physical button included. The two are complementary and independent: the lock gate removes the attacker's remote route into DFU; `dfu_lock` covers the routes the gate can't see (a physically-present attacker who held the unlock keys, or a still-open session, can `BootloaderJump` and then flash freely *unless* `dfu_lock` is also set). A deployment that cares about threat #1 wants both. §6 addresses the resulting two-`unlock_keys` config surface.

## 2. Command gating policy

Three tiers. The gate is a single check in `RynkService::dispatch` before the handler match, so every transport inherits it uniformly.

**Open (always dispatchable):** `GetVersion`, `GetCapabilities` (handshake — must work locked), the three lock endpoints themselves, every read (`GetKeyAction`, `GetDefaultLayer`, `GetEncoderAction`, `GetKeymapBulk`, `GetMacro`, `GetCombo[Bulk]`, `GetMorse[Bulk]`, `GetFork`, `GetBehaviorConfig`), all connection/status reads (`GetConnectionType/Status`, `GetBleStatus`, `GetCurrentLayer`, `GetBatteryStatus`, `GetPeripheralStatus`, `GetWpm`, `GetSleepState`, `GetLedIndicator`), `SwitchBleProfile`, `Reboot`, and the in-flight `GetLayout` (0x0009) / `GetDeviceInfo` (0x000A).

- `Reboot` stays open: worst case is a nuisance restart into identical state; tooling legitimately reboots after applying settings; gating it buys nothing an attacker wants (flashing needs `BootloaderJump`).
- `SwitchBleProfile` stays open: redirecting output to another profile requires that profile to already hold a bond the attacker controls — i.e. prior access.

**Locked (require unlock, else `RynkError::Locked`):**

- `BootloaderJump` — firmware replacement (the remote-software route into DFU; pair with `dfu_lock` to also cover the physical route, §1).
- `StorageReset` — destroys config and bonds.
- `GetMatrixState` — keystroke exfiltration. The handler's current `host_security`-cfg zero-bitmap degrade is replaced by the gate (§6 makes `host_security` implied by `rynk`).
- `ClearBleProfile` — deletes persistent security state (a bond) and opens a re-pair hijack window.

**Policy tier (config writes; keyboard.toml knob, default open):** `SetKeyAction`, `SetDefaultLayer`, `SetEncoderAction`, `SetKeymapBulk`, `SetMacro`, `SetCombo[Bulk]`, `SetMorse[Bulk]`, `SetFork`, `SetBehaviorConfig`.

Default open because on-the-fly configuration is the product: Vial proved the model, and forcing a physical two-key ceremony on every keymap tweak kills the configurator UX. Input-injection persistence (threat #2) is real, so `[host] write_requires_unlock = true` moves this whole tier into the locked set for users who want it. It is a runtime `bool` in the lock config (no cfg matrix), read from keyboard.toml by the macro.

**Error:** add `RynkError::Locked` to `rmk-types/src/protocol/rynk/error.rs` **now**, while breaking is free. The enum is in every response envelope; appending a variant after v1 would make old hosts fail to decode any response that carries it (fail-closed but ugly — effectively a major bump per the `mod.rs` contract). Pre-release it costs nothing. Hosts map it to "unlock required" UX.

The gate is plain control flow next to the dispatch match in `rmk/src/host/rynk/mod.rs` — no macro. It is *not* a single static list: the hard-locked set is one `matches!`, the policy-tier write set is a second, and they combine with the runtime knob — `is_hard_locked(cmd) || (self.write_requires_unlock && is_write_cmd(cmd))`. So the gate reads `&self` (for the bool and the lock), not just `cmd`.

## 3. Wire design

Replace the reservation comment in the `endpoints!` table with:

```rust
// ── System (0x00xx) ──
...
/// Lock gate. All three are dispatchable while locked.
GetLockStatus = 0x0006: () => LockStatus;   // pure read, no side effects
UnlockPoll    = 0x0007: () => LockStatus;   // arms/refreshes the attempt, samples held keys
Lock          = 0x0008: () => ();           // relock immediately
```

**Flow — Vial-style physical-presence poll.** No crypto, no shared secret: the challenge is "hold these physical keys while the host polls", which proves the device owner is present. This matches the staged types' intent, reuses a field-proven model (`VialLock`), and is the right weight for the threat model.

`UnlockPoll` merges Vial's `UnlockStart` + `UnlockPoll`: the first call arms the attempt window, every call refreshes it and counts currently-held challenge keys (`keymap.read_matrix_key`), and the attempt succeeds when all are held simultaneously. Idempotent — no start-before-poll ordering bug class. If polls stop, the window lapses and `unlocking` clears; **window = 500 ms** (Vial uses 100 ms; a BLE WebHID round trip can exceed that, and 500 ms still relocks promptly when the host vanishes). Recommended host cadence ~150 ms (§7).

**Type changes (staged types are not frozen — reshape them):**

```rust
/// Grows to 4: two-key chords are easy to hold accidentally; Vial allows more.
/// rmk-config rejects configs with more keys at build time.
pub const UNLOCK_KEYS_SIZE: usize = 4;

// Derive list vs. today's LockStatus: no `Copy`, no `MaxSize` — a
// `heapless::Vec` field forbids both (see below).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockStatus {
    pub locked: bool,
    /// An unlock attempt is armed (renamed from `awaiting_keys` to match VialLock vocabulary).
    pub unlocking: bool,
    /// Challenge keys not currently held; == unlock_keys.len() when no attempt is armed.
    pub remaining_keys: u8,
    /// The challenge itself: physical (row, col) the user must hold.
    /// Empty while locked ⇒ permanently locked (no unlock_keys configured).
    pub key_positions: Vec<(u8, u8), UNLOCK_KEYS_SIZE>,
}

// `#[derive(MaxSize)]` doesn't support `heapless::Vec` (rmk-types/src/lib.rs),
// so hand-write the bound — the same manual impl `UnlockChallenge` carried,
// folded onto LockStatus:
impl MaxSize for LockStatus {
    const POSTCARD_MAX_SIZE: usize = 2 * bool::POSTCARD_MAX_SIZE
        + u8::POSTCARD_MAX_SIZE
        + crate::heapless_vec_max_size::<(u8, u8), UNLOCK_KEYS_SIZE>();
}
```

Two derive consequences the staged type doesn't anticipate (`payload/system.rs:71` derives both): **`LockStatus` loses `Copy`** (heapless::Vec isn't `Copy`) and **loses derived `MaxSize`** (impl above). Blast radius is nil — handlers return it by value and the only current `Copy` consumers are the round-trip tests.

`UnlockChallenge` is **deleted**: folding the challenge into `LockStatus` gives one response type for both endpoints (single host decode path), lets the UI render "hold these keys" before arming anything, and makes "permanently locked" self-describing (locked + empty list). Cost is ≤ ~12 postcard bytes — irrelevant next to `RYNK_MAX_PAYLOAD`, and the folded `max_const` in `command.rs` re-derives buffer floors automatically.

Semantics fixed points:

- `GetLockStatus` is side-effect-free (lazy window-expiry aside) so status polling can't hold an attempt alive by accident — arming requires `UnlockPoll`.
- `UnlockPoll` with no keys configured: leaves state untouched, warns once, returns `locked=true, unlocking=false, remaining_keys=0, key_positions=[]`.
- `Lock` always succeeds; on an `insecure` device it is a no-op in effect because `is_unlocked()` returns true regardless (§4).

**No topic push for lock state.** Topics are best-effort (droppable); a lock signal you cannot trust is worse than none. Lock state only changes in response to the host's own requests, session end (the host's next gated call gets `Locked`), or reboot — polling covers all of it. Skip the topic; add one later only if a real UI need appears.

**Versioning impact (`mod.rs` contract):** three new `Cmd`s would be a minor bump; reshaping `LockStatus`/deleting `UnlockChallenge` is free (never served by any firmware); appending `RynkError::Locked` would be effectively major post-v1 (see §2). Pre-release, `ProtocolVersion::CURRENT` stays `{0,1}` and the `snapshots/*.snap` goldens are regenerated in the same PR — the snapshot tests failing is the intended tripwire. Firmware without the gate answers `UnknownCmd` to 0x0006, which hosts treat as "no lock support" (graceful degradation).

## 4. Lock state semantics

**Global per-device, one instance, `Cell`-based — mirror `VialLock` exactly.** There is already exactly one `HostService` instance shared by reference across the concurrent USB and BLE sessions (`usb/mod.rs`, `ble/mod.rs` both take `&HostService`), so state owned by `RynkService` is naturally device-global. Physical unlock authorizes *the person at the device*, not a transport; per-session state would double the machinery and still not change what an unlocked handler can do. The `Cell` pattern is safe here for the same reason it is in `VialLock`: every mutation is a non-`await` `Cell::set` on `Copy` data.

Concretely: move `rmk/src/host/via/vial_lock.rs` → `rmk/src/host/lock.rs`, rename `VialLock` → `HostLock`, make the poll window a constructor parameter (via passes 100 ms, rynk 500 ms), and have via keep using it. This is the smallest step that avoids two copies of the same state machine and satisfies the existing `vial_lock` TODO ("remove it, use `host_security`") without dragging via's config migration into this work.

One semantic fix while moving: `is_unlocked()` becomes `insecure || unlocked.get()` (today `insecure` only seeds the Cell). Required because of relock-on-disconnect below — otherwise an `insecure` device with no unlock keys would relock on its first session end and be stuck until reboot. Side effect for via: a wire `Lock` command no longer locks an `insecure` device — acceptable, arguably a fix.

**Relock triggers:**

- **Any session end** — `run_session` relocks on return. It has six `return;` statements and no single exit, so the clean shape is a **Drop guard** (a small struct holding `&locker` whose `drop` calls `lock()`) constructed at the top of `run_session` — not six scattered `lock()` calls, and via's `run_session` has no analogous hook to copy. This kills the hole Vial has today, where unlock persists after the tool disconnects and a later drive-by page with a stale port grant inherits it.
- **Explicit `Lock` command.**
- **Reboot** — state is RAM-only (`Cell`s), nothing to do.
- **No inactivity timer.** A session that stays open holds exactly the trust the user physically granted it; disconnect-relock already covers abandonment, and a timer adds a clock to the state machine for a scenario (open, idle, trusted tool) that isn't an attack.

**Cross-session interaction (state is global):** unlocking via USB unlocks the BLE session too — accepted, it's the same physical authorization. Conversely, *any* session ending relocks everyone. USB and BLE are separate `run_session` calls over one `HostLock`, so an **idle** BLE session that merely churns — a phone backgrounding and reconnecting — relocks a USB flash mid-operation, possibly repeatedly; the one-shot laptop-disconnect case is rare, but the always-connected-BLE case is not. Decision: **keep the simple global relock** — fail-closed beats a session refcount, and a flash workflow can prefer a quiet BLE environment. If the churn proves painful, the narrow fix is to relock only on the session that performed the unlock (tag the unlock with a session id), deferred until a real report.

## 5. Link-layer requirements

**BLE — require `Encrypted`, not `Authenticated`, and declare it on the characteristics.**

Verified against trouble-host 0.7.0 (`~/.cargo/registry/src/.../trouble-host-0.7.0`, macros 0.5.0):

- The `#[characteristic(...)]` macro supports a `permissions(...)` property: `permissions(encrypted)`, `permissions(authenticated)`, or per-op `permissions(read = encrypted, write = encrypted, cccd = ...)` (`trouble-host-macros-0.5.0/src/characteristic.rs`). It maps to `trouble_host::attribute::PermissionLevel::{Allowed, EncryptionRequired, AuthenticationRequired, NotAllowed}`.
- Enforcement is per ATT operation in the attribute server against `connection.security_level()`; below `Encrypted` the peer gets `INSUFFICIENT_AUTHENTICATION` (`attribute.rs` `can_read`/`can_write`, `attribute_server.rs`). `AuthenticationRequired` additionally demands `EncryptedAuthenticated` (MITM-protected pairing).

Current firmware state (corrects this design's original premise): the rynk characteristics carry **no** declarative security, but an unencrypted central still **cannot** use them — `gatt_events_task` already rejects every GATT read/write below `Encrypted` with `INSUFFICIENT_ENCRYPTION` and drops rynk writes before they reach `RYNK_BLE_RX_PIPE` (`rmk/src/ble/mod.rs`, the `encrypted` checks around the `GattEvent::Write` arm).

Decision:

- Add `permissions(encrypted)` to both `RynkService` characteristics and the `RynkHidService` report characteristics in `rmk/src/ble/ble_server.rs`. It makes the requirement declarative and enforced inside the ATT server, so it survives any future refactor of the app event loop. Keep the existing app-level guard as the data-path filter — write-without-response frames carry data regardless of the ATT reply, so the drop-before-pipe check stays load-bearing.
- Do **not** require `authenticated`: Just Works is the default pairing path (`passkey_entry` is opt-in), so demanding MITM pairing would break most existing setups. And it wouldn't buy authorization anyway — which is the point: **BLE encryption is transport privacy, not authorization.** A nearby attacker can Just-Works-pair while the device advertises; the lock gate is the authorization boundary for dangerous ops.

Not verified (flagged): whether trouble-host delivers a `GattEvent::Write` to the app loop at all when declarative permissions reject the operation (enforcement order: attribute server vs. event accept/reject). Nothing here depends on the answer — the app guard remains either way — but confirm during PR2 hardware testing with an unpaired central.

**USB / Web Serial:** the OS permission model and the browser's port-grant prompt are the reach boundary; there is no transport-level identity beyond that, and the `rynk:` serial-magic discovery string deliberately makes the port easy to find. The lock gate *is* the mitigation for everything past the grant. State this in user docs rather than pretending otherwise.

**UART:** physically wired; physical access defeats every software measure. Out of scope — but note the gate still applies because it lives in `dispatch`, not in any transport.

## 6. keyboard.toml surface

Reuse and generalize the existing `[host]` section (`HostConfig`, `rmk-config/src/lib.rs:908`) — `unlock_keys` is already protocol-neutral there:

```toml
[host]
rynk_enabled = true
# Physical keys (row, col) held simultaneously to unlock. Max 4 (UNLOCK_KEYS_SIZE).
unlock_keys = [[0, 0], [3, 12]]
# Start (and stay) unlocked — development escape hatch. Default false.
insecure = false
# Move config writes (SetKeyAction, SetMacro, ...) into the locked tier. Default false.
write_requires_unlock = false
```

- Rename `vial_insecure` → `insecure` — a **four-site** change, not one. `HostConfig` (`rmk-config/src/lib.rs:921`, add `#[serde(alias = "vial_insecure")]` or `deny_unknown_fields` rejects old TOMLs); the resolved mirror `rmk-config/src/resolved/host.rs` (field at `:6`, map at `:17` — a separate struct, the plan's easiest miss); `rmk-macro`'s `expand_vial_config` (`keyboard_config.rs:61`); and rmk's `VialConfig` (`config/vial.rs`). `rynk_enabled` already threads all four (`HostConfig:915`, `resolved/host.rs:4`), so only `insecure` + the new `write_requires_unlock` are net-new.
- `unlock_keys` unset ⇒ **dangerous ops permanently locked**: gated commands return `RynkError::Locked`, `UnlockPoll` warns once and never unlocks — exactly Vial's warn-and-refuse (`check_unlock` returns nonzero forever on an empty list). This is the task's fail-closed default: a fresh config gets safety, and the host UI can say "set `unlock_keys` in keyboard.toml" because `key_positions` comes back empty.
- `insecure = true` ⇒ starts and stays unlocked (§4 semantics). The only opt-out, visible in the TOML next to the keys it bypasses.
- Validation in `rmk-config`: error at build time if `unlock_keys.len() > 4` or any position is outside the matrix dimensions (rows/cols are known from `[layout]`).
- **Two `unlock_keys` now coexist** with different meanings: `[dfu].unlock_keys` (`DfuConfig`, gates the DFU download under `dfu_lock`, §1) and `[host].unlock_keys` (this lock gate). Independent by design — set one, both, or neither. The §7 docs page must name both and say which protects what; do not silently alias them.
- Plumbing: add `LockConfig { unlock_keys: &'static [(u8, u8)], insecure: bool, write_requires_unlock: bool }` to `rmk/src/config` (under the `rynk` feature), a field on `RmkConfig`, `write_requires_unlock` onto **both `HostConfig` and its resolved `Host` mirror**, and a codegen block in `rmk-macro/src/codegen/keyboard_config.rs` mirroring `expand_vial_config`. `RynkService::new(keymap, config)` already receives `&RmkConfig` (currently `_config`) — no signature change; `LockConfig.unlock_keys` is `&'static`, so the `HostLock<'a>` borrow type-checks via `'static: 'a`. Via keeps reading its existing `VialConfig` fields; migrating it to `LockConfig` is the deferred `vial_lock` cleanup.

**Feature wiring:** make `rynk` imply `host_security` (`rynk = ["host", "host_security", "rmk-types/rynk"]`). The unlock mechanism needs matrix-state tracking (`keymap.read_matrix_key`), and a security gate that silently compiles out is the hole this design removes. Cost is one small bitmap plus a per-key-event update — negligible. This also deletes `GetMatrixState`'s zero-bitmap cfg fallback (the cfg is now always true under rynk) and replaces it with the lock check. No new `rynk_lock` feature: the gate is not optional; `insecure` is the opt-out.

## 7. Host-side flow

Client additions (`rynk/src/api.rs`, thin wrappers over `request::<E>` like every other method):

```rust
pub async fn get_lock_status(&mut self) -> Result<LockStatus, RynkHostError>;
pub async fn unlock_poll(&mut self)    -> Result<LockStatus, RynkHostError>;
pub async fn lock(&mut self)           -> Result<(), RynkHostError>;
```

`RynkError::Locked` arrives through the existing device-error path of `RynkHostError` (the decoded `Err` half of the response envelope) — no new plumbing, but the wasm/demo layer should special-case it to open the unlock UI instead of showing a generic error.

Recommended client/UI sequence:

1. On connect (after `get_capabilities`): `get_lock_status()` once. Render a lock badge. If `locked` and `key_positions` is empty → show "permanently locked — set `unlock_keys` in keyboard.toml" and disable gated actions outright.
2. When the user triggers a gated action — or any request returns `Locked` — open the unlock modal. Render the challenge from `key_positions` (map `(row, col)` onto the `GetLayout` geometry when available, plain coordinates otherwise): "press and hold these N keys".
3. Poll `unlock_poll()` every **150 ms** (≥3 polls per 500 ms firmware window, tolerant of BLE latency). Update progress from `remaining_keys` (e.g. "2 of 3 held").
4. Exit conditions: `locked == false` → close modal, retry the original action. User cancel → simply stop polling; the firmware window lapses after 500 ms and clears `unlocking` (no cancel endpoint needed). Suggest a ~30 s UI timeout that stops polling with a "timed out" note.
5. On tool exit, optionally call `lock()` as a courtesy; disconnect relocks regardless (§4).

wasm: add the three methods to `RynkClient` in `rynk-wasm`, regenerate `bindings/rynk.d.ts` via `rynk-wasm/scripts/gen-types.sh` (`LockStatus` picks up its tsify emit from the type change; CI's drift gate enforces the regen). Demo page: lock badge + modal with per-key progress.

## 8. Implementation plan

Three PRs, each independently green.

**PR 1 — types (`rmk-types`):**
1. `payload/system.rs`: reshape `LockStatus` — add `key_positions`, rename `awaiting_keys` → `unlocking`, **drop `Copy` and derived `MaxSize`**, hand-write `MaxSize` (heapless::Vec has no derive support; §3). Delete `UnlockChallenge`, folding its manual `MaxSize` onto `LockStatus`. `UNLOCK_KEYS_SIZE` 2 → 4. Move the `round_trip_unlock_challenge` max-bound assertions onto `LockStatus`.
2. `error.rs`: append `RynkError::Locked`.
3. `command.rs`: the three endpoint rows replacing the reservation comment (coordinate with the in-flight 0x0009/0x000A branches — no value collisions).
4. Regenerate `snapshots/*.snap`; the golden diff is the reviewable wire change.

**PR 2 — firmware (`rmk`, `rmk-config`, `rmk-macro`):**
1. `rmk/Cargo.toml`: `rynk` += `host_security`.
2. Move `host/via/vial_lock.rs` → `host/lock.rs`; `VialLock` → `HostLock`; window as a constructor param (via 100 ms, rynk 500 ms); `is_unlocked()` = `insecure || unlocked`; via updated mechanically.
3. `rmk/src/config`: `LockConfig`; `RmkConfig` field. `rmk-config`: `insecure` rename + alias and `write_requires_unlock` **on both `HostConfig` and `resolved/host.rs`** (four-site rename, §6), unlock-key count/bounds validation. `rmk-macro/src/codegen/keyboard_config.rs`: emit `LockConfig` when `rynk_enabled`.
4. `host/rynk/mod.rs`: `RynkService` gains `locker: HostLock` (built in `new` from config); gate at the top of `dispatch` = `is_hard_locked(cmd) || (self.write_requires_unlock && is_write_cmd(cmd))` — two `matches!` groups + the runtime knob, so it reads `&self`, not just `cmd` (§2); relock via a Drop guard in `run_session` (§4).
5. `host/rynk/handlers/system.rs`: `Handle` impls for `GetLockStatus` / `UnlockPoll` / `Lock`. `handlers/status.rs`: `GetMatrixState` drops the cfg fallback (gate covers it).
6. `ble/ble_server.rs`: `permissions(encrypted)` on the rynk and rynk-HID characteristics; keep the app-level encrypted guard.
7. Tests, in-crate:
   - `host/lock.rs` unit tests (synchronous — `HostLock` has no `.await`, so no `block_on`): arm/refresh/expiry of the window, partial → full key hold, empty-keys never unlocks, `insecure` survives `lock()`. Drive expiry with `MockDriver::get().advance(...)` — the mock clock is frozen between synchronous calls (`test_support.rs`), so cross the 500 ms window explicitly rather than via wall-clock or the window param.
   - `run_session`-level test (extends the existing harness in `host/rynk/mod.rs` with its `ChunkRead`/`VecWrite`): locked `GetMatrixState` → `Err(Locked)`; `GetLockStatus` shows the challenge; simulate presses via `keymap.update_matrix_state`; two `UnlockPoll`s → unlocked; `GetMatrixState` → `Ok`; end the session (EOF), start another → locked again (proves relock-on-disconnect). Use `GetMatrixState` as the gated probe — unlike `BootloaderJump` it has no boot side effects on the test target.
   - Add `host_security` to the dev-loop feature set in `scripts` where the rynk matrix runs.
8. Hardware check (nRF54LM20A rig): unpaired central rejected at ATT; Just-Works-paired central reaches the session but gated commands return `Locked`; unlock ceremony over both Web Serial and WebHID/BLE.

**PR 3 — host (`rynk` workspace):**
1. `rynk/src/api.rs`: three methods; confirm `Locked` maps through the existing device-error variant.
2. `rynk-wasm`: `RynkClient` wrappers, `gen-types.sh` regen (`rynk.d.ts` drift gate), demo unlock modal with `remaining_keys` progress and cancel-by-stop-polling.
3. Extend the e2e harness (`rynk-e2e`) with the before/after-unlock flow; docs page update (`docs/docs/main/docs`) covering tiers, the TOML fields, and the macro-secrets caveat.

Rollout note: PR 2 flips defaults from "everything open" to "dangerous ops locked unless `unlock_keys` is set" — a deliberate breaking change while the protocol is pre-release. Changelog + docs must lead with the two-line fix (`unlock_keys = [[r, c], ...]` or `insecure = true`).
