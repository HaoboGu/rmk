# Rynk protocol

**Rynk** is RMK's native host-communication protocol — a transport-agnostic
binary protocol that carries RMK's canonical types (`KeyAction`, `Combo`,
`Morse`, `Fork`, `EncoderAction`, `BatteryStatus`, `BleStatus`, ...) on
the wire. Use Rynk when you want a richer host integration than Vial's
fixed 32-byte HID reports.

Rynk and Vial are **mutually exclusive** — enable exactly one on a given
firmware build.

## Quick start

1. Enable the Cargo feature on the `rmk` dependency in your firmware
   crate's `Cargo.toml`:

   ```toml
   [dependencies]
   rmk = { version = "*", default-features = false, features = [
       "rynk",                # ← enables the Rynk protocol
       "bulk_transfer",       # ← optional: bulk keymap/combo/morse Cmds
       "storage",
       "async_matrix",
       # ... your chip / connectivity features
   ] }
   ```

2. Switch `host.vial_enabled` to `false` and set `host.rynk_enabled =
   true` in `keyboard.toml`:

   ```toml
   [host]
   vial_enabled = false
   rynk_enabled = true
   ```

   The macro layer cross-checks the Cargo feature and `keyboard.toml`
   setting at build time — they must agree, or the build fails.

3. (Optional) Override the wire buffer size:

   ```toml
   [rmk]
   rynk_buffer_size = 2048    # default = RYNK_MIN_BUFFER_SIZE
   ```

   The default is sized to fit every possible wire frame exactly.
   Overprovisioning is fine; underprovisioning fails the build with a
   clear error.

4. Install the host CLI:

   ```sh
   cd rmk-host-tool
   cargo install --path rynk-cli
   ```

5. Talk to a flashed device:

   ```sh
   rynk info                    # protocol version + capabilities
   rynk get-key 0 0 0           # read one key
   rynk layer                   # current active layer
   rynk matrix                  # live matrix bitmap
   rynk wpm                     # latest WPM snapshot
   rynk sleep                   # latest sleep flag
   rynk led                     # latest HID LED indicator bits
   rynk reboot                  # reboot the firmware
   ```

## Transports

- **USB.** Rynk adds one vendor-specific interface (class `0xFF`) with
  one BULK IN + one BULK OUT endpoint. WinUSB MS OS 2.0 descriptors are
  registered so Windows binds WinUSB automatically — no `.inf` install
  required. The interface is discoverable by GUID
  `{F5F5F5F5-1234-5678-9ABC-DEF012345678}`.
- **BLE.** A custom GATT service (UUID `F5F50001-…`) with two
  characteristics: `input_data` (server → host notify) and `output_data`
  (host → server write). Both carry `≤ MTU − 3` bytes per write/notify.

Both transports share the same 5-byte fixed header + postcard-encoded
payload framing. The host library transparently reassembles by `LEN`.

## Capabilities & versioning

On connect, the host runs a two-step handshake:

1. `Cmd::GetVersion` → `ProtocolVersion { major, minor }`. Host bails on
   `major` mismatch or `minor > supported`. Version is the sole runtime
   gate — schema drift between equal-version `rmk-types` checkouts is
   prevented by the wire-format snapshot test in CI.
2. `Cmd::GetCapabilities` → `DeviceCapabilities`. Host caches the
   returned layout / feature flags / limits and gates every subsequent
   Cmd on them.

Host code gates every subsequent call on the bitflags in
`DeviceCapabilities`.

## Migrating from Vial

Vial-only users don't need to do anything — Vial remains the default and
its build path is unchanged. To migrate:

1. Change `default-features = false, features = ["rynk", ...]` (drop
   `vial`, `vial_lock`).
2. Set `host.rynk_enabled = true` (and `host.vial_enabled = false`).
3. Re-flash; install `rynk-cli`.

The firmware will no longer expose Vial's 32-byte HID interface — it
exposes the Rynk vendor-class interface instead.

## Reference

See `docs/rynk_wire.md` in the repo for the wire-format specification
(header layout, `Cmd` table, framing rules).
