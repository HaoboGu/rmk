# Rynk Firmware Plan — one BLE config session, custom GATT wire unchanged (B-plus)

## Status

`RynkHidService` (rynk config over BLE WebHID) shipped in `c633582f`. This document describes the
follow-up that is now implemented: **keep both GATT services** (each reaches a disjoint client
surface — neither can be dropped), and collapse the duplicated firmware machinery below them into a
**single per-connection session**, while leaving the custom-GATT `RynkService` wire **completely
unchanged**. The de-frame is unified at the producer seam, not by changing either wire.

## Why both GATT services stay (don't merge them)

Verified against desktop BLE stacks and confirmed empirically on macOS (a `bluest` probe enumerated
the bonded keyboard's GATT and saw the custom `RynkService` but **not** the `0x1812` HID service):

| Client | Reaches | Never reaches |
|---|---|---|
| Native app (bluest, `rynk-ble`) | custom 128-bit `RynkService` (full-MTU) | the `0x1812` HID service |
| Browser (WebHID, `rynk-wasm`) | `0xFF60` vendor report in HOGP (`RynkHidService`) | any custom GATT service |

A native generic-GATT client can never read a bonded keyboard's `0x1812` HID service — macOS/iOS
CoreBluetooth filters it from discovery, Windows WinRT returns `AccessDenied`, Linux BlueZ's
`hog`/`input` plugin hides it. WebHID rides the OS HID stack so it reaches the `0xFF60` vendor report
but never a custom service; Web Bluetooth blocklists `0x1812`. Peers confirm the split: **ZMK Studio =
custom service only**, **Vial = HOGP report only**. RMK is the union → two services is correct.

The only waste was running them as two concurrent sessions. This collapses them into one — **without
touching the custom-GATT wire**, so the native `rynk-ble` host is unchanged.

## Design — one session, de-frame at the seam, custom wire untouched

1. **Custom `RynkService` unchanged.** `input_data`/`output_data` stay `heapless::Vec<u8,
   RYNK_BLE_CHUNK_SIZE>` (244): variable-length, full-MTU notifications, exactly as before WebHID
   existed. The native bluest host (`rynk/rynk-ble`) speaks the same raw wire — no host change.
2. **One inbound buffer with a clean producer contract.** Both transports feed the single
   `RYNK_BLE_RX_PIPE` (`channel.rs`), which carries **only de-framed rynk bytes**. It is written from
   exactly two encryption-gated arms in `gatt_events_task`: the custom-GATT arm writes its raw
   `output_data`; the WebHID arm writes `ble::rynk::hid_report_payload(report)` — a pure, unit-tested
   helper that strips the `[len][payload 0..len][zero-pad to 32]` framing (`len == 0` = keep-alive →
   nothing written). So the consumer sees a clean contiguous stream and needs no framing knowledge.
3. **Transport-agnostic interior, one monomorphization.** The session's `Read` is a bare
   `&RYNK_BLE_RX_PIPE` (the pipe's ring provides cross-read buffering, so no `pending`/`pos` and no
   `read_exact` re-framing). `RynkService::run_session<Read, Write>` is monomorphized once.
4. **Outbound mux.** `MuxBleTx` holds both reply characteristics and routes each reply/topic by
   `ACTIVE_SOURCE: AtomicU8` (`NONE`/`CUSTOM`/`HID`) — raw MTU-chunked on the custom char, or
   `[len][payload][zero-pad]` 32-byte reports on the HID char. `ACTIVE_SOURCE` is set **only on a real
   config write** (never on a CCCD subscribe — the OS HOGP driver auto-subscribes the HID input CCCD
   on bond, which would otherwise mis-route a native session's topics). Reset per connection; one
   peer per connection means it never flaps.
5. **One `TopicSubscribers`.** Falls out of (3)–(4): one session → one subscription set → each topic
   encoded and notified once.
6. **Deleted:** `ble/rynk_hid.rs`, the second `run_session` future (`join4` → `join3`), and the
   per-HID RX channel. `RYNK_DISPATCH_GUARD` is kept (`_ble`-gated): it serializes the at-most-two
   concurrent host sessions (one USB + one BLE) — forward-defense for a future multi-`await` bulk
   handler.

## Measured footprint & speed (nRF54LM20A, `rynk`+`_ble`)

- **FLASH 406,084 B / RAM 50,700 B** — the smallest of every keep-`Vec<244>` variant.
- **Speed is identical across all transports regardless of internal design** (the wire is unchanged):
  USB/Web-Serial `get_macro` p50 ~2.3 ms (~26.7 KiB/s); GATT-over-BLE ~45 ms (~1.4 KiB/s);
  HID-over-BLE ~60 ms (~0.9 KiB/s). BLE is connection-interval-bound; USB is ~20× faster — the fast
  path when the keyboard is plugged in, with WebHID-over-BLE the wireless/bonded fallback.

## Why this shape (vs the alternatives that were built and measured)

- **vs changing the custom wire to fixed-32 (the "unify everything" variant):** that is ~1.3 KB
  smaller, but it changes the custom-GATT wire (forcing a host-crate rewrite and hand-duplicating the
  decoder across firmware + host, which drifted), and downgrades the native transport to 31-byte
  reports. Rejected: keep the custom service untouched.
- **vs a dedicated HID `Channel` + adapter:** a bounded inbound channel can stall the whole GATT loop
  on `.send().await` if the unbound char is written; the single always-drained Pipe is immune.
- **vs reading 32-byte frames back off the Pipe with `read_exact`:** that re-acquires a boundary the
  writer just destroyed and relies on an implicit cross-module "every write is 32 bytes" invariant;
  stripping at the seam keeps a single, self-delimiting pipe contract.
- **vs a bind-once `Signal` router:** the `AtomicU8` set-on-write is sufficient (one peer per
  connection, so the binding can't flip mid-session); a `Signal` costs more for an unreachable guard.

## Verification

- Unit tests (`ble/rynk.rs`): `hid_report_payload` framing (strip, `len == 0` keep-alive, oversize and
  short-slice clamps) + a seam→pipe smoke test that drives a multi-report message through the
  production de-frame into `RYNK_BLE_RX_PIPE` and reads it back as the contiguous stream the session
  consumes.
- Integration (`rynk_loopback`, `rynk_hid_loopback`): both transports round-trip through the real
  `run_session` — single-report frames, multi-report reassembly, pipelined requests, a topic push.
- `cargo nextest run --no-default-features --features=split,rynk,storage,async_matrix,_ble` is green
  (556 tests). Hardware-verified on the nRF54LM20A (boot, encrypted, GATT + WebHID + USB all serve).
