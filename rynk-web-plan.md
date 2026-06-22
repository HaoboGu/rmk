# Rynk Web Client — Architecture & Implementation Plan

## What changed in this revision

1. **BLE on Electron + Tauri (native, never WebBluetooth); browser stays USB-only.** A plain browser tab cannot reach BLE without WebBluetooth, so BLE lives in the desktop shells' native layer. Electron (a "web" app) gets BLE via its main process; Tauri via its Rust core.
2. **No Tauri plugin.** Tauri calls `rynk-serial`/`rynk-ble` directly from `main.rs` via `#[tauri::command]`s.
3. **Bitfield handling is now a first-class framework criterion** and resolved: the 3 flag types (`ModifierCombination`/`LedIndicator`/`MouseButtons`) cross to JS as a **struct of named booleans** (`{ left_shift: true, left_ctrl: true }`) via a small custom `is_human_readable` serde (compact `u8` on the wire, structured object at the JS boundary) + a tsify container `type` override. Researched against ts-rs/specta/zod_gen: only tsify (runtime-coupled) can make this honest.
4. **Type verification added** (compile-time `tsc` + runtime round-trip + golden snapshot).
5. **Architecture + data-flow diagrams added** (below).
6. **Compile-time type generation made explicit** — tsify emits the `.d.ts` during the wasm build; verification is compile-time.
7. **Cargo feature renamed `ts` → `wasm`** (the feature that pulls in tsify/wasm-bindgen and flips `rmk-types` to std for the web build).

## Context

Rynk is RMK's native host-communication protocol (5-byte header + postcard payload;
CMD high bit = topic push; `endpoints!`/`topics!` tables in `rmk-types` are the single
source of truth). A browser demo already exists (`rynk/rynk-wasm`, `wasm-pack --target
web`): the page owns a WebSerial port and hands a byte link (`JsLink {send, recv}`) into
a wasm module where the Rust `rynk::Client` drives the protocol. Typed values cross the
wasm boundary via hand-written `serde-wasm-bindgen`; `ts-rs` types only two structs.

Goal: one architecture serving **browser pages, Electron, and Tauri** with **type-safe
Rust↔TS**, **`rmk-types` as the single source of truth**, the **fastest practical**
web↔firmware path, and **bidirectional** flow (request/response, topic push, config-set).

## Architecture

**One wasm core (the protocol brain), per-host byte transport behind `JsLink`.** The
version-independent `rynk::Client` + typed API (`rynk/src/{driver,api}.rs`) + `rmk-types`
compile into one wasm module used on all three hosts. Only opaque postcard bytes cross
the host boundary; the `Client` never crosses into JS, so there is exactly **one
marshaler** (serde-wasm-bindgen via tsify) everywhere — no browser-vs-desktop drift.

```
                ┌──────────────────────────────────────────────────────────┐
                │ rmk-types  (single source of truth, no_std for firmware)   │
                │  • endpoints!/topics! tables  • wire payload types         │
                │  • #[cfg_attr(feature="wasm", derive(Tsify))]              │
                └───────────────────────────┬──────────────────────────────┘
                                            │ compiled into (wasm feature on)
                ┌───────────────────────────▼──────────────────────────────┐
                │ rynk-wasm   (wasm-pack build --target web)                 │
                │  • rynk::Client (driver+api)  • single session actor       │
                │  • tsify Ts<T> boundary → pkg/rynk_wasm.{js,d.ts}          │
                └───────────────────────────┬──────────────────────────────┘
                          JsLink { send(Uint8Array), recv()->Uint8Array }
                                  (opaque postcard-framed bytes only)
        ┌──────────────────────────────────┼──────────────────────────────────┐
        ▼                                   ▼                                    ▼
  BROWSER PAGE                        ELECTRON                                 TAURI
  JsLink = WebSerial                  renderer: wasm Client                renderer: wasm Client
  USB only, no BLE                    main: native USB+BLE  ──IPC──>        core main.rs: #[command]s
        │                             (rynk-serial + rynk-ble)             rynk-serial + rynk-ble
        │                             JsLink = IPC bytes                   JsLink = invoke/listen bytes
        ▼                                   ▼                                    ▼
   keyboard (USB)                     keyboard (USB + BLE)                 keyboard (USB + BLE)

  Two JsLink implementations total: (1) WebSerial (browser); (2) IPC-to-native-host
  (Electron main / Tauri core) — both reuse rynk-serial / rynk-ble for the native side.
```

Data flow (all three ride one byte link into the one wasm `Client`, owned by the
session actor in `rynk/rynk-wasm/src/session.rs`):

```
request/response  JS get_key(0,0,0) → [Ts<args>] → actor → Client.get_key
                  → postcard frame → JsLink.send → transport → firmware
                  → SEQ-matched reply → Client → [Ts<KeyAction>] → typed JS object

topic push        firmware (CMD high bit, SEQ 0) → Client.next_event()
                  → TopicEvent → [Ts] → on_topic(JS callback)   (best-effort)

config-set        JS set_behavior(cfg) → [Ts<BehaviorConfig>.to_rust()] → Client.set_behavior
                  → postcard → firmware → Result<(),RynkError> ack
                  → resolved promise, or typed Error whose .name is the kind
```

**Fastest = wire-bound, not boundary-bound** (BLE RTT 7.5–30 ms, USB ~1 ms; postcard
<1 µs; marshaling single µs). Levers: keep binary postcard end-to-end (done); use
serde-wasm-bindgen not JSON at the boundary (tsify `js` feature); cut round-trip count
on bulk dumps via native MTU (the desktop BLE path negotiates a real 244-byte MTU).

## Deployment matrix

| Host | `JsLink` backing | USB | BLE | New code |
|---|---|---|---|---|
| **Browser** | WebSerial (page owns port) | ✓ | ✗ (no WebBluetooth) | none (exists) |
| **Electron** | IPC to main process | ✓ | ✓ | main-process native device I/O on `rynk-serial`/`rynk-ble` + IPC glue |
| **Tauri** | `invoke`/`listen` to core | ✓ | ✓ | `#[tauri::command]`s in `main.rs` on `rynk-serial`/`rynk-ble` (no plugin) |

## Framework choice & bitfield handling

Selection criteria (bitfields are now explicit, per review):
1. **Covers `heapless 0.9` types** — `ts-rs` 12's `heapless-impl` pins `<0.9`, so it can
   only ever type the 2 heapless-free structs. **Disqualifying.**
2. **Generates the wasm ABI** so `#[wasm_bindgen]` fn signatures carry real TS types,
   not `JsValue`/`any`.
3. **One definition for type + runtime** (no ts-rs↔serde-wasm-bindgen drift).
4. **Type overrides for types whose serde wire shape ≠ Rust shape** — the deciding
   capability for **bitfields** and custom-serde fields.

**tsify** (`madonoharu`, 0.5.x, `js` feature, `Ts<T>` path) meets all four; ts-rs fails
(1)(2), specta/tauri-specta add a second RC-pinned pipeline, zod_gen is runtime-only.

**Bitfields → a structured, type-safe TS shape (verified).** `bitfield-struct` expands
`ModifierCombination`/`LedIndicator`/`MouseButtons` to `#[repr(transparent)] struct X(u8)`
serializing as `u8` — which would cross to JS as a bare, un-type-safe `number`. Instead,
give the 3 flag types a small hand-written `is_human_readable`-branching serde: a compact
`u8` for postcard (`is_human_readable()==false` → wire/firmware/`MaxSize`/snapshots
unchanged) and a **struct of named booleans** for serde-wasm-bindgen
(`is_human_readable()==true`), e.g. `{ left_shift: true, left_ctrl: true }` (optional
fields, absent = false). Declare the matching shape with a container
`#[cfg_attr(feature="wasm", tsify(type="{ left_shift?: boolean; … }"))]`. serde recurses,
so this fixes every use automatically — top-level (`GetLedIndicator`, `LedIndicatorChange`)
and nested (`StateBits`, `Fork.kept_modifiers`, `Action::Modifier(..)`, etc.).

Verified facts behind this: serde-wasm-bindgen's Serializer inherits serde's default
`is_human_readable()==true` and its Deserializer returns `true`; postcard returns `false`
— so the branch routes correctly in both directions. Researched against the other
type-binding crates for this exact case: **only tsify is runtime-coupled**, so it is the
only one that can make a structured bitfield *honest*. `ts-rs`/`specta` are type-only (a
structured override would *lie* — they don't drive the serde-wasm-bindgen runtime; making
them honest needs `#[serde(into/from)]`, which bloats the `u8` wire), and `zod_gen` would
need a hand-written Zod transform plus a redundant runtime validator. Cost here: ~one
`Serialize`/`Deserialize` pair per flag type (3 total) + 3 container overrides.

## Type generation (compile-time)

tsify generates TS **at wasm build time**, not at runtime:
- Data types: each `Tsify` derive emits a `typescript_custom_section`; wasm-bindgen
  collects them at link, so `wasm-pack build --target web` writes the bundled
  `pkg/rynk_wasm.d.ts`. `rynk-wasm` must enable `rmk-types/wasm` (via its dependency
  features) so the build turns the derives on.
- Function surface: the `#[wasm_bindgen]` wrappers in `session.rs` (typed `Ts<T>` after
  A2) contribute their real signatures to the same `.d.ts`.
- There is **no runtime codegen** and no separate `bindings/*.ts` step (ts-rs is removed).
  Runtime conversion is serde-wasm-bindgen inside the wasm; the `.d.ts` is the build-time
  description of that conversion.

## Type verification

Because types are compile-time artifacts, verify them at compile time plus a runtime
round-trip:
- **V1 — build is the first gate.** CI runs `wasm-pack build`; a missing override or a
  type error fails the build. (Primary guard; `pkg/` is gitignored so it can't be diffed
  directly.)
- **V2 — `tsc --noEmit`** over the generated `.d.ts` plus a tiny tracked `verify/assert.ts`
  that exercises representative shapes (a `KeyAction` union member incl. `TapHold`, a
  `Combo` whose `actions` is `KeyAction[]`, `LedIndicator` as a named-boolean struct, a `TopicEvent`
  variant). Proves the emitted types compile and have the expected shape.
- **V3 — runtime round-trip** via `wasm-bindgen-test`: for representative values —
  bitfield (`LedIndicator`), data enum (`KeyAction::TapHold`), `heapless::Vec`
  (`Combo.actions`), custom serde (`Morse.actions`), tuple vec (`UnlockChallenge`) —
  assert Rust→`JsValue`→Rust round-trips AND the `JsValue` shape matches (e.g.
  a named-boolean object for the bitfield (and postcard keeps it a single `u8`), array for the vecs). This catches any
  tsify-type vs serde-wasm-bindgen-runtime divergence.
- **V4 — golden snapshot.** Copy the generated `.d.ts` to a **tracked** path and
  `git diff --exit-code` it in CI, mirroring the existing `rmk-types/.../snapshots/*.snap`
  wire-golden discipline; a wire-type change that wasn't regenerated fails CI.
- **Guard:** CI also builds `rmk-types` with **no features** and a firmware feature set, so
  tsify/wasm-bindgen/std can never reach an embedded target.

## Dependency verdicts

| Dep | Verdict | How |
|---|---|---|
| **tsify** (0.5.x, `js`, `Ts<T>`) | **Use — sole type+ABI generator** | One derive → `.d.ts` type + serde-wasm-bindgen conversion from the same serde def. The only **runtime-coupled** generator, so the only one that can make custom-serde shapes (bitfields → named-boolean structs) *honest*. |
| **serde-wasm-bindgen** | **Use, transitively** | It is tsify's `js` backend; stop hand-calling `parse`/`encode`. Set `missing_as_null` (Option→null). No bigint config — every wire int ≤ u32. |
| **ts-rs** | **Remove** | `heapless 0.9` pin blocks it; emits no wasm ABI. |
| **specta / tauri-specta** | **Skip** | No typed Tauri command surface here (raw bytes only); RC-pinned; would add a second pipeline. |
| **zod_gen** | **Skip at protocol layer** | Firmware is authoritative; wire path already type-checks twice. Optional later: `ts-to-zod` over the tsify `.d.ts` at the **UI** layer only, no Rust dep. |
| **tsify-next** | **Skip** | Unmaintained fork (RUSTSEC-flagged); upstream `tsify` is maintained again. |

## Implementation

### Track A — Core (host-agnostic; do first)

**A0 · Baseline & guards.** Rebuild stale `pkg/` (`wasm-pack build --target web`),
smoke-test `index.html` over USB against `rynk`-feature firmware. Add the
"firmware builds without `wasm`" CI guard. Files: `rynk/rynk-wasm/{src/session.rs,index.html}`, CI.

**A1 · ts-rs → tsify, full wire surface typed.**
- `rmk-types/Cargo.toml`: drop `ts-rs`; add `tsify = { version = "0.5", optional = true, features = ["js"] }` + `wasm-bindgen = { version = "0.2", optional = true }`; **rename the feature `ts` → `wasm`**: `ts = ["dep:ts-rs", "host"]` → `wasm = ["dep:tsify", "dep:wasm-bindgen", "host"]`. Update the no_std gate in `lib.rs` to `#![cfg_attr(not(feature = "wasm"), no_std)]` (was `feature = "ts"`).
- Replace the two `ts_rs::TS` derives in `rmk-types/src/protocol/rynk/payload/system.rs` with `#[cfg_attr(feature="wasm", derive(tsify::Tsify))]`.
- Add the item-level `cfg_attr` Tsify derive to the **full closure** of wire types reachable from the tables (~52 types — incl. the action/keycode enum tree). Representative files: `rmk-types/src/protocol/rynk/payload/{system,keymap,encoder,combo,morse,fork,macro_data,status}.rs`; `rmk-types/src/{action/*,keycode/*,combo,fork,morse,connection,battery,ble,steno}.rs`; `TopicEvent` in `rmk-types/src/protocol/rynk/command.rs`. **Item-level only** — never field-level `wasm_bindgen` attrs inside `cfg_attr`.
- **Bitfields → named-boolean structs (3 types):** on `LedIndicator`, `ModifierCombination`, `MouseButtons` (`rmk-types/src/{led_indicator,modifier,mouse_button}.rs`), replace the derived serde with a hand-written `is_human_readable`-branching `Serialize`/`Deserialize`: emit a `u8` when `!is_human_readable()` (postcard — byte-identical to today, so `MaxSize`/snapshots/firmware are unchanged), and a struct of named `bool` fields (optional, default false) when human-readable (serde-wasm-bindgen). Add `#[cfg_attr(feature="wasm", derive(tsify::Tsify))]` + a container `#[cfg_attr(feature="wasm", tsify(type="{ left_shift?: boolean; left_ctrl?: boolean; … }"))]` matching that struct. Keep the `bitfield-struct` definition + accessors as-is (the serde impl reads/writes via the accessors).
- **Field overrides** (foreign/custom-serde): every `heapless::Vec<T,N>` → `T[]`
  (`Combo.actions`→`KeyAction[]`; bulk `actions`/`configs`→`KeyAction[]`/`Combo[]`/`Morse[]`;
  `MacroData.data` & `MatrixState.pressed_bitmap`→`number[]`; `UnlockChallenge.key_positions`→`[number,number][]`),
  and `Morse.actions` (LinearMap, custom `morse_actions_serde`) → `[number, Action][]`.
- `missing_as_null` on types with `Option` fields (e.g. `Combo`). Use the `Ts<T>` path, not deprecated `into/from_wasm_abi`.
- Delete `rynk/rynk-wasm/bindings/*.ts`.

**A2 · Typed wasm signatures (`Ts<T>` in the `endpoints!` macro).** In
`rynk/rynk-wasm/src/session.rs`: value arg becomes `Ts<$jty>`, return becomes
`Result<Ts<T>, JsValue>`; delete `parse()`, rewrite `encode()`/`run()` to produce `Ts<T>`;
lifecycle fns (`connect`/`capabilities`/`protocol_version`/`resync`/`events_dropped`)
return `Ts<…>`; `on_topic` serializes `TopicEvent` via `Ts`. 11 wrappers have a typed
value arg (`storage_reset, set_key, set_encoder, set_keymap_bulk, set_combo,
set_combo_bulk, set_fork, set_morse, set_morse_bulk, set_macro, set_behavior`); the rest
change return-side only. **Keep the macro rows unchanged** (single source for the fn
surface; each must resolve to a real `Client` method). Error helpers stay `JsValue`.

**A3 · Verification & drift gate.** Implement V1–V4 above; add a test asserting every
`Cmd` in `command.rs` has a matching `endpoints!`/`topics!` row.

### Track B — Per-host transports over the same wasm artifact (after A2)

USB first on all three, then add BLE to the desktop native layer.

- **B-browser** (`rynk/rynk-wasm/index.html`): confirm the existing WebSerial `JsLink` + VID/PID discovery (`RYNK_SERIAL_MAGIC` isn't readable pre-open) + frozen `GetVersion` probe + `loadCore`. USB only. Near-zero new code.
- **B-electron** (new shell dir): renderer runs the wasm Client; **main process** owns device I/O on `rynk-serial`/`rynk-ble` and bridges raw bytes to a renderer `JsLink` over `ipcRenderer`/`contextBridge`. Recommended: a small Rust sidecar binary built on `rynk-serial`/`rynk-ble` (one transport impl, consistent with "wrap your own"), spawned by main over stdio; `node-serialport`/`noble` is the fallback if shipping a sidecar is undesirable. **USB first, then BLE** in the same main-process layer.
- **B-tauri** (app `main.rs`, **no plugin**): `#[tauri::command]`s `open_port`/`close_port`/`rynk_write` + a `rynk_recv` event, implemented directly on `rynk-serial`/`rynk-ble`. Receive raw bytes via `tauri::ipc::Request` + `InvokeBody::Raw` (not a `Vec<u8>` param). ~30-line JS `JsLink` adapter over `invoke`/`listen`. **USB first, then BLE** in the same core. Zero changes to `Client`/`bridge.rs`/`session.rs`.

### Later (deferred)

- `ts-to-zod` over the tsify `.d.ts` for form/JSON-import validation when a config-editor UI lands — UI-only, no Rust dep.

## Risks & mitigations

- **tsify pulls wasm-bindgen+std under `wasm`.** Keep every tsify ref behind item-level `cfg_attr`; A0 guard compiles firmware without `wasm`.
- **Bitfield custom serde must stay in lockstep with the `bitfield-struct` fields** (a new flag bit needs a matching struct field + `tsify(type=…)` entry). The `is_human_readable` split is verified sound (serde-wasm-bindgen Serializer default `true` / Deserializer `true`; postcard `false`); bound the 3 hand-written impls with the V3 round-trip test (asserts the named-boolean object at the boundary *and* a single `u8` on postcard).
- **Field overrides tracked by hand** (heapless::Vec / Morse.actions). Bounded by V3 round-trip + V4 golden snapshot + a CLAUDE.md note pairing new `heapless::Vec`/custom-serde fields with an override.
- **Electron two-transport-impl risk** if using node libs for raw I/O instead of the Rust sidecar — prefer the shared Rust sidecar to keep one transport implementation.
- **tsify 0.5.x pre-1.0** — pin a known-good version; use `Ts<T>` (the non-deprecated, leak-free path). Pre-release stance: a tsify break is a localized fix behind `wasm`.

## End-to-end verification

- `cargo build -p rmk-types` (no features) **and** a firmware feature set compile (no_std intact); `cargo build -p rmk-types --features wasm` compiles; `cargo nextest` for `rmk-types` green.
- `wasm-pack build --target web` succeeds; `pkg/rynk_wasm.d.ts` shows real types: e.g. `set_key(layer:number,row:number,col:number,action:KeyAction): Promise<void>`, `get_combo(index:number): Promise<Combo>`, `get_led_indicator(): Promise<{ num_lock?: boolean; caps_lock?: boolean; … }>`, topics deliver typed `TopicEvent`.
- V2 `tsc --noEmit` over the `.d.ts` + `assert.ts` passes; V3 `wasm-bindgen-test` round-trips pass; V4 golden snapshot matches.
- Browser: serve over `http://localhost`, smoke-test `index.html` vs real `rynk`-over-USB firmware (connect → caps; get/set round-trip; live topics).
- Electron: connect over USB, then BLE, on macOS/Linux/Windows; confirm renderer is pure wasm + IPC `JsLink`.
- Tauri: `cargo build` the app; run on macOS (WKWebView), connect over USB then BLE via the core commands; confirm no `serde_json`/specta in the graph and no plugin crate.
```
