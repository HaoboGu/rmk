# Rynk Firmware Plan — BLE config over WebHID (`RynkHidService`)

## Context

The rynk web/host client uses **four transports selected by context**:

| | USB (fastest) | BLE |
|---|---|---|
| **Pure browser** | WebSerial → wasm `Client` | **WebHID** (`0xFF60` HID-over-BT) → wasm `Client` |
| **Native shell** (Tauri / Electron) | tokio-serial (`rynk-serial`) | bluest (`rynk-ble`, custom GATT, 244 B) |

A pure browser **cannot** reach rynk's custom 128-bit GATT service on an OS-bonded keyboard
(the device stops advertising once connected as HID; Web Bluetooth can't attach). It **can**
reach a **vendor HID-over-GATT report via WebHID** — proven, since Vial-web over BLE works on
macOS — because WebHID rides the OS's existing HID connection.

**This plan's only firmware change:** add a second BLE config transport — a vendor HID service
(`RynkHidService`) carrying the *same* rynk protocol — so the browser gets a seamless pure-web
BLE path. The custom-GATT `RynkService` stays as the faster native (bluest) path. Both feed the
same transport-agnostic dispatcher `RynkService::run_session<R: Read, T: Write>`
(`rmk/src/host/rynk/mod.rs:146`, **unchanged**).

Scope: `rmk` + one `rmk-types` const. (Host-side `rmk-types` type-generation is out of scope
here and keeps the postcard wire byte-identical — u8 unchanged — so it doesn't affect firmware.)

## Design decisions

- **Separate 4th HID GATT service.** RMK already runs three `#[gatt_service(uuid =
  HUMAN_INTERFACE_DEVICE)]` instances (`HidService`, `CompositeService`, and under vial
  `VialService` — `ble_server.rs:68/87/105`); a fourth is the same proven pattern, enumerated
  by WebHID as its own collection. Vendor **usage page `0xFF60` / usage `0x61`** (free —
  `rynk` and `vial` are mutually exclusive). The `#[gatt_server]` attribute table **auto-sizes**
  (no manual budget to overflow). Single report, **report ID 0** (the `2908` report-reference
  descriptors carry `[0,1]`/`[0,2]`, as Vial does) → the host uses `sendReport(0, …)`.
- **Report size N = 32** (`RYNK_HID_REPORT_SIZE`), matching Vial's proven report. A fixed HID
  report does **not** auto-fragment: a notification past `ATT_MTU − 3` is silently truncated,
  not split (`ble/rynk.rs:47-48`), and write-without-response is capped the same — so an
  N-byte report **requires the negotiated MTU ≥ N + 3**. N = 32 (MTU ≥ 35) is exactly what
  Vial-over-BLE already proves on real hosts. Raising N to 64 (MTU ≥ 67 — which RMK's ~247-MTU
  config satisfies) would halve round-trips on large payloads, but that gain is only realized
  by bulk ops (still `Unimplemented`); keep 32 until bulk lands and MTU ≥ 67 is confirmed on
  the target host stacks.
- **Framing — 1-byte per-report length prefix.** Wire layout `[len][payload 0..len][zero-pad to
  N]` (N=32 → 31 usable bytes/report). Adapters add/strip it so `run_session` sees a clean
  contiguous byte stream (exactly as the Vial 32-byte adapters hide chunking today). Edge cases:
  - a frame spanning multiple reports — Tx splits into ≤(N−1)-byte slices, Rx reassembles via
    the rynk header's `payload_len`;
  - multiple small frames pack into one report fine;
  - **`len = 0` is a keep-alive, NOT EOF** — the Rx adapter must loop for the next report and
    never return `Ok(0)` (which ends the session at `host/rynk/mod.rs:154`);
  - Rx must buffer **sub-report reads** (`pending`/`pos`, like `rynk-wasm/src/bridge.rs` and
    `rynk-ble`), because `run_session` reads 5 bytes (header) then N.

  Chosen over report-aligned framing (which can't carry frames > 63 B without a continuation
  scheme).
- **Channel `Channel<RawMutex, [u8;RYNK_HID_REPORT_SIZE], SIZE>`** (preserves report
  boundaries, like `VIAL_BLE_RX_CHANNEL` at `channel.rs:112`) — **not** a `Pipe` (which would
  lose the per-report length prefix).
- **Concurrency guard (the main risk).** The custom-GATT and HID sessions share one
  `RynkService` → one `KeyboardContext` → one `KeyMap` (`RefCell`, `keymap.rs:80`). The
  single-threaded embassy executor + no-`await`-across-borrow means **no `RefCell` panic
  today**, but a future multi-`await` bulk handler (`host/rynk/mod.rs:72-95`, currently
  `Unimplemented`) could interleave a read-modify-write → lost update. Add a **tight per-turn
  dispatch guard** (`static Mutex<RawMutex, ()>`) around the dispatch + response write
  (`mod.rs:231-235`), `_ble`-gated, ~6 lines. (In an `_ble` build the guard is global across
  *all* transports — USB/UART dispatch acquires it too; harmless, as only one host configures at
  a time.) Rejected alternative: a single shared-Rx session — a fiddly per-origin Tx demux,
  since rynk frames carry no transport tag.
- **Security:** mirror the existing encryption gate (`conn.raw().security_level()?.encrypted()`
  at `ble/mod.rs:374`; drop + warn on unencrypted at `:436-443`) on the HID output writes,
  **before** the channel send — otherwise bytes reach the dispatcher before the ATT reply.
- **Always-on under `rynk` + `_ble`** (both transports compiled in; the browser always has a
  path). **No advertising change** — HID is discovered via GATT enumeration post-connect
  (`advertise` at `ble/mod.rs:588` unchanged).

## Steps (file:line anchors)

1. **Const** — `pub const RYNK_HID_REPORT_SIZE: usize = 32;` in
   `rmk-types/src/protocol/rynk/mod.rs` (near `RYNK_BLE_CHUNK_SIZE` `:63`). 32 = Vial-proven,
   needs MTU ≥ 35; see the report-size decision before raising to 64.
2. **Descriptor** — `RynkHidReport` in `rmk/src/hid.rs` after `ViaReport` (`:46-59`):
   `#[gen_hid_descriptor]` with `[u8; RYNK_HID_REPORT_SIZE]` (32) input/output,
   `usage_page = 0xFF60`, `usage = 0x61`.
3. **GATT service** — `RynkHidService` in `rmk/src/ble/ble_server.rs`: clone `VialService`
   (`:67-84`, already 32-byte reports), swap `ViaReport→RynkHidReport`; add the field to the
   `#[cfg(feature="rynk")] struct Server` (`:29-35`). Leave the vial / no-host variants alone.
   The macro fixes `report_map: [u8; LEN]` — read LEN off the compile error (as the codebase
   does for 27/67/111).
4. **Channel** — `RYNK_HID_BLE_RX_CHANNEL: Channel<RawMutex,[u8;RYNK_HID_REPORT_SIZE],SIZE>` in
   `rmk/src/channel.rs` after `:116`.
5. **Adapters + runner** — new `rmk/src/ble/rynk_hid.rs`, mirroring `rmk/src/ble/vial.rs`:
   `RynkHidBleRx { pending: heapless::Vec<u8,RYNK_HID_REPORT_SIZE>, pos }`, `RynkHidBleTx {
   input_data: Characteristic<[u8;RYNK_HID_REPORT_SIZE]>, conn }`, and `run_host_ble_hid(server,
   conn, &RynkService)` that clears the channel and calls `service.run_session(&mut rx, &mut
   tx)`. Implement the length-prefix framing + the `len=0` keep-alive loop (and the `pending`/
   `pos` sub-report buffering — unlike `VialBleRx`, which reads whole 32-byte frames). Register
   the module in `ble/mod.rs`.
6. **Guard** — `static RYNK_DISPATCH_GUARD: Mutex<RawMutex,()>` in `rmk/src/host/rynk/mod.rs`;
   acquire around `dispatch` + the response `write_all` (`:231-235`), `_ble`-gated.
7. **Spawn** — in `rmk/src/ble/mod.rs` `host_task`/`inner` join (`:744-756`), add a
   `host_hid_task` (→ `join4`) running `run_host_ble_hid` with the **same** `&RynkService` +
   `conn`; `pending()` when not `rynk`+`_ble` **or** when no host service is bound (mirror
   `host_task`'s `if let Some(service)`).
8. **Routing** — cache `rynk_hid_service.output_data`/`input_data` handles (`Characteristic` is
   `Copy`, no clone) alongside `ble/mod.rs:298-302`; add the `else if event.handle() ==
   output_hid.handle` write arm + the CCCD arm (`:421-450`), reusing the in-scope `encrypted`
   flag.

## Reuse

`rmk/src/ble/vial.rs` (adapter template), `ViaReport` (`hid.rs:46`), `RynkService::run_session`
(`host/rynk/mod.rs:146`, unchanged), and the existing channel + handle-routing patterns.

## Verification (no hardware)

- **Unit-test the framing adapters**: len-prefix correctness, a multi-report frame, two frames
  packed in one report, a `len=0` keep-alive (must NOT end the stream), sub-report buffering.
- **Loopback through the real dispatcher**: mirror `rmk/tests/rynk_loopback.rs` via
  `tests/common/rynk_link.rs`, interposing the HID framing; run `get_version`, a reply that
  **exceeds one report** to exercise multi-report framing (`get_macro` chunk, or
  `get_keymap_bulk` when `bulk` is on — note `get_capabilities` is only ~33 B and stays in one
  report), set/get key, a topic push.
- **Concurrency test**: two `run_session` instances sharing one `RynkService`, interleaved
  `Set`/`GetKeyAction`, assert no lost update and no `RefCell` panic (guard in place).
