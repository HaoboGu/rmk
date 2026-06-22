# Rynk Host Plan — type-safe web client over four transports

## Context

One web frontend serves a **pure browser** and **native shells** (Tauri/Electron), with
type-safe Rust↔TS generated from `rmk-types`, over **four transports selected by context**:

| | USB (fastest) | BLE |
|---|---|---|
| **Pure browser** | WebSerial → wasm `Client` | WebHID (`0xFF60` HID-over-BT) → wasm `Client` |
| **Native shell** (Tauri / Electron) | tokio-serial (`rynk-serial`) → native `Client` | bluest (`rynk-ble`, custom GATT, 244 B) → native `Client` |

Selection rule: prefer the native shell when present ("use local when available"); within
either row prefer **USB when plugged in** (fastest), else BLE. Same `rynk` protocol /
`rynk/src/api.rs` / dispatcher under all four. This supersedes the earlier "browser = USB-only,
BLE = native shells only" stance — WebHID makes the browser a complete USB+BLE solution; the
native shells become a speed/UX enhancement.

## A. Type generation (`rmk-types` → one source, two emitters)

- **Drop ts-rs, adopt tsify** (`0.5`, `js` feature, `Ts<T>` path). Rename the `rmk-types`
  feature `ts → wasm` (`Cargo.toml:47`); replace the ts-rs derives (`payload/system.rs:14,31`
  plus the full ~52-type closure reachable from the `endpoints!`/`topics!` tables) with
  `#[cfg_attr(feature="wasm", derive(tsify::Tsify))]`; flip the no_std gate to
  `#![cfg_attr(not(feature="wasm"), no_std)]`. Rationale (heapless is a wash — both need
  overrides): tsify emits the wasm-bindgen ABI (fn signatures carry real types, not `any`) and
  is runtime-coupled to serde-wasm-bindgen (overrides can't lie). Delete `rynk-wasm/bindings/*.ts`.
- **Bitfields → named-boolean TS objects** via a declarative `flag_bitfield_serde!` macro in
  `rmk-types` for `LedIndicator` / `ModifierCombination` / `MouseButtons` (`led_indicator.rs`,
  `modifier.rs`, `mouse_button.rs`). It emits, from one field list: a companion `…Flags` tsify
  struct + an `is_human_readable`-branching serde — **compact `u8` on postcard (wire
  byte-identical → firmware/`MaxSize`/snapshots unchanged)**, struct of named `bool` on
  serde-wasm-bindgen **and** serde_json. serde recurses → fixes every nested use (e.g.
  `Action::Modifier`, `Fork.kept_modifiers`).
- **Field overrides** (foreign/custom-serde): `heapless::Vec<T,N>` → `T[]` (`Combo.actions`,
  bulk vecs, `MacroData.data`, `MatrixState.pressed_bitmap`, `UnlockChallenge.key_positions`),
  `Morse.actions` → `[number, Action][]`. `missing_as_null` on `Option`-bearing types.
- **Split emitter.** A codegen-only cdylib **`rmk-types-ts`** anchors the derives (one
  `#[wasm_bindgen]` anchor over the payload roots, survives DCE) and emits the canonical
  **`rmk_types.d.ts`** → Tauri imports it. `rynk-wasm` re-emits the same types + its fn
  signatures into **`rynk_wasm.d.ts`** → browser/Electron import it. One source, two files,
  both golden-snapshotted so they can't drift. Sound because serde_json (Tauri IPC) and
  serde-wasm-bindgen (wasm) are both `is_human_readable() == true` → byte-identical shapes,
  including the bitfield named-bool objects.
- **Verification**: V1 build both cdylibs; V2 `tsc --noEmit` + a tracked `assert.ts`; V3
  `wasm-bindgen-test` round-trip; **V3b serde_json parity** (locks the Tauri reuse); V4
  golden-diff both `.d.ts`; plus a guard that `rmk-types` compiles with no features / a
  firmware feature set (no_std intact, tsify/wasm-bindgen never reach embedded).

## B. Browser transports (JsLinks over the wasm `Client`)

`rynk-wasm/src/bridge.rs` (`BridgeTransport`, the `JsLink {send,recv}` extern contract) and
`session.rs` (`connect(link, on_topic)`) are **transport-agnostic — no changes**; transports
are plugged in `index.html`.

- **WebSerial JsLink (USB, fastest)** — already exists (`index.html` `link(port)` factory +
  `navigator.serial.requestPort`); keep.
- **WebHID JsLink (BLE, `0xFF60`) — NEW**: `navigator.hid.requestDevice({ filters:
  [{ usagePage: 0xFF60, usage: 0x61 }] })` / `getDevices()`. The keyboard exposes a single
  vendor HID report (report ID 0) carrying the rynk byte stream; frame it into fixed 32-byte
  reports (the firmware's `RYNK_HID_REPORT_SIZE`) with a **1-byte length prefix** —
  `[len][payload 0..len][zero-pad to 32]` (31 usable bytes/report; `len = 0` is a keep-alive,
  never end the stream): `send` = `sendReport(0, …)` (prepend `len`, pad to 32); `recv` =
  `oninputreport` (strip `len`, buffer across reads). Reuses `BridgeTransport` reassembly
  unchanged.
- **Typed wasm signatures**: `Ts<T>` in the `session.rs` `endpoints!` macro (keep the rows;
  extract the row list into a `rynk` module so the Tauri command macro (§D) reuses the same
  single fn surface).

## C. Transport selector (greenfield — none exists today)

- **Shell detection** ("use local when available"): a native bridge present (Tauri `invoke` /
  Electron preload API) → use the native transport; else fall back to the web APIs.
- **Auto USB/BLE**: `navigator.serial.getPorts()` + `navigator.hid.getDevices()` on load and on
  `connect`/`disconnect` events — these list only **previously-granted** devices, so a
  first-time connection still needs one user-gesture chooser per transport; afterward it is
  automatic. Prefer USB (fastest) when a keyboard is plugged in, else BLE; switch live.
- **Seamless BLE**: rides the OS keyboard bond → **no pairing prompt**; one-time WebHID chooser,
  then `getDevices()` silent reconnect on later visits.
- **Cross-transport identity (open dependency)**: showing the *same* physical keyboard as one
  device across USB and BLE needs a stable unique id — which the protocol does **not** expose
  today (`get_version` is identical for every keyboard; `DeviceCapabilities` has no id field).
  Either add one (a device UUID/serial in `DeviceCapabilities`, or a `GetDeviceId` endpoint — a
  small firmware addition) or descope to a per-transport device with no cross-transport
  unification.

## D. Native shells

- **Tauri**: native `rynk::Client` over `rynk-serial` (USB) / `rynk-ble` (BLE); typed
  `#[tauri::command]`s generated from the shared `endpoints!` table; imports `rmk_types.d.ts`.
  No wasm, no plugin. Cargo graph: `rynk` + `rynk-serial` + `rynk-ble` + `rmk-types` (no
  `rynk-wasm`).
- **Electron** (decided: **native sidecar**): bundle a Rust sidecar reusing
  `rynk-serial`/`rynk-ble` (tokio-serial + bluest, 244 B BLE). The sidecar is a **dumb byte
  pipe**: the renderer runs the *same* wasm `Client` over an IPC-backed `JsLink` (so `Client` +
  `bridge.rs` are reused unchanged; no native command surface), and the sidecar only owns the
  transport. WebSerial/WebHID are the fallback when the sidecar is absent.

## Critical files

- `rmk-types/Cargo.toml` (feature `ts→wasm`, deps); `rmk-types/src/{protocol/rynk/payload/*,
  led_indicator.rs, modifier.rs, mouse_button.rs, action/*, keycode/*, combo.rs, fork.rs,
  morse.rs, …}` (tsify derives, `flag_bitfield_serde!`, field overrides).
- New **`rmk-types-ts`** cdylib (canonical `.d.ts` emitter).
- `rynk/rynk-wasm/src/session.rs` (`Ts<T>` signatures); `…/src/bridge.rs` (**unchanged**);
  `…/index.html` (WebHID JsLink + transport selector); delete `…/bindings/*.ts`.
- New Tauri app crate; new Electron shell dir + Rust sidecar (reusing `rynk-serial`/`rynk-ble`).

## Reuse

`BridgeTransport` + `session.connect` (no change), `rynk-serial`/`rynk-ble` (native, already
implemented), and the `endpoints!` table (single fn surface for both the wasm and Tauri command
macros).

## Verification

`cargo build -p rmk-types` (no features + a firmware feature set, no_std intact) and
`--features wasm`; `wasm-pack build` both cdylibs emit real types; V2 `tsc`; V3/V3b round-trips;
V4 golden diff; browser smoke over USB (WebSerial) **and** BLE (WebHID) against `rynk`-feature
firmware; Tauri + Electron connect over USB then BLE; confirm no `rynk-wasm`/specta/plugin in
the Tauri graph.

## De-risk

A **Windows + Linux WebHID-over-BLE spike** (macOS is proven by Vial-web) before betting the
"browser covers everything" story on non-macOS.
