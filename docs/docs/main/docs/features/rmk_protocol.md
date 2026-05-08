# RMK Protocol

The **RMK protocol** is RMK's typed, postcard-rpc-based host configuration
protocol. It exists alongside `vial` (the older byte-oriented protocol over a
32-byte HID report pair) and is mutually exclusive with it: pick one host
service per build.

The protocol is declared in `rmk-types/src/protocol/rmk/`. The firmware-side
server is in `rmk/src/host/rmk_protocol/`. The host-side library + CLI live in
`rmk-host-tool/`.

## Why a second protocol

- **Typed end-to-end.** The wire format uses RMK's canonical types
  (`KeyAction`, `Combo`, `Morse`, `Fork`, `EncoderAction`, `BatteryStatus`,
  `BleStatus`) directly — no custom byte parsers on either side.
- **Faster than Vial-over-HID.** USB transport is vendor-class bulk endpoints
  (64 byte FS / 512 byte HS); BLE transport is a dedicated GATT service with
  MTU-sized characteristics. Both deliver ~10–100× the throughput of Vial's
  32-byte HID report pair.
- **Schema-stable.** Every endpoint/topic carries an 8-byte schema hash; the
  hash list is locked in `rmk-types/src/protocol/rmk/snapshots/` and verified
  by the `endpoint_keys_*_locked` and `topic_keys_*_locked` tests. Wire
  changes are caught at CI time, not at runtime.

## Versioning

`sys/version` is the single immortal endpoint — its path and `ProtocolVersion`
shape never change, even across major bumps. Hosts call `GetVersion` first,
bail on `major` mismatch or `minor` greater than supported, then call
`GetCapabilities` to learn layout dimensions and feature flags, then gate
every subsequent call on those flags.

- **`minor` bump:** new endpoint, new field appended to a wire struct, new
  variant in a wire enum (including `RmkError`).
- **`major` bump:** endpoint removed/retyped, struct field reshaped, enum
  variant renamed/renumbered.

Snapshots regenerate with `UPDATE_SNAPSHOTS=1 cargo test -p rmk-types
--features rmk_protocol`.

## Capability discovery

`GetCapabilities` returns layout dimensions, max-size limits for each domain,
and feature flags (`storage_enabled`, `lighting_enabled`, `is_split`,
`ble_enabled`, `bulk_transfer_supported`, …). Hosts MUST gate
feature-dependent calls on these flags; firmware will reject calls for
disabled features with `RmkError::BadState`.

## Lock / unlock

The ICD declares a three-phase physical-key challenge:

1. `GetLockStatus` → `LockStatus { locked, awaiting_keys, remaining_keys }`
2. `UnlockRequest` → returns an `UnlockChallenge` listing ≤ 2 key positions
   the user must physically hold
3. The device transitions to `unlocked` once those keys are held; `LockRequest`
   re-locks

**v1 ships always-unlocked.** The endpoints are wired and their schemas are
frozen, but the firmware stubs them — `GetLockStatus` returns `locked:
false`, `UnlockRequest` returns an empty challenge, `LockRequest` is a
no-op, and writes are not gated. Resurrecting the gate is a focused follow-up
that lifts the existing `vial_lock.rs` state machine into `host/lock.rs` and
threads a shared `Mutex<HostLock>` through both Servers.

## Cargo features

| Feature         | Pulls                                | Effect                                                                             |
|-----------------|--------------------------------------|------------------------------------------------------------------------------------|
| `rmk_protocol`  | `host`, `dep:postcard-rpc`, `dep:cobs` | Enables the protocol server. Mutually exclusive with `vial`.                       |
| `bulk_transfer` | `rmk_protocol`, `rmk-types/bulk`     | Enables the bulk endpoints (`keymap/bulk_*`, `combo/bulk_*`, `morse/bulk_*`).      |

The `host_security` feature (used by `vial_lock`) is **not** pulled by
`rmk_protocol` in v1 — the lock gate is deferred to v2.

## USB driver setup

The firmware exposes a vendor-class function (interface class `0xFF`,
sub-class `0x00`) with WinUSB MSOS 2.0 descriptors. On Windows the WinUSB
driver binds automatically; the matching DeviceInterfaceGUID is
`{C8B9F0E2-9D4A-4B4C-AAFB-1C3F2D10A8E5}` — the same one `rmk-cli` looks up.

On Linux, add a udev rule so the device is accessible without `sudo` (see
`rmk-host-tool/README.md`).

On macOS no driver setup is required.

## Migrating from Vial

```toml
# keyboard.toml
[host]
vial_enabled = false
rmk_protocol_enabled = true
```

```toml
# Cargo.toml — rmk default features pull `vial`, so disable defaults and
# re-add the features you actually want.
[dependencies]
rmk = { version = "...", default-features = false, features = [
    "rmk_protocol",
    "bulk_transfer",
    "storage",
    "<your-chip-feature>",
] }
```

The two features are mutually exclusive: keeping both enabled raises a
compile-time error.

## See also

- `rmk-types/src/protocol/rmk/mod.rs` — full protocol module documentation
- `rmk-host-tool/README.md` — host CLI usage and OS-specific setup
- `rmk/src/host/rmk_protocol/` — firmware server implementation
