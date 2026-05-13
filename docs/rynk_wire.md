# Rynk wire format

Dev-facing specification of the binary protocol that runs over USB bulk
and BLE GATT. Audience: anyone implementing a third-party host
adapter, or anyone debugging the wire with `xxd`/Wireshark.

## Frame layout

```
┌───────────────────────────────────────────────┐
│                   RYNK FRAME                  │
├──────────────┬───────────┬────────────────────┤
│ CMD u16 LE   │ SEQ u8    │ LEN u16 LE         │  ← 5-byte header
├──────────────┴───────────┴────────────────────┤
│              postcard-encoded payload          │  ← LEN bytes
└────────────────────────────────────────────────┘
```

| Field | Bytes | Meaning |
|---|---|---|
| **CMD** | 2 (LE) | `0x0000–0x7FFF` request/response. `0x8000–0xFFFF` topic. Top bit splits dispatch with one mask. |
| **SEQ** | 1 | Opaque echo. Firmware copies request's SEQ into response. Topics send `SEQ = 0`. Host uses it to correlate concurrent in-flight calls. |
| **LEN** | 2 (LE) | Payload byte count. Bounded by `RYNK_MIN_BUFFER_SIZE - 5`; firmware rejects frames with `LEN > RYNK_BUFFER_SIZE - 5`. |
| **payload** | LEN | `postcard`-encoded `Request` or `Response`. May be empty. |

Header members map 1:1 to `rmk_types::protocol::rynk::Header { cmd, seq,
len }`. Encoding/decoding lives in
[`rmk-types/src/protocol/rynk/header.rs`](../rmk-types/src/protocol/rynk/header.rs)
and is shared between firmware and `rynk-host`.

## Cmd table

Hex grouping mirrors handler modules. Sequential within each group.
**Encoder is part of Keymap** (one handler file, one Cmd group).

### System — `0x00xx`

| Cmd | Hex | Request | Response | Feature |
|---|---|---|---|---|
| `GetVersion` | `0x0001` | `()` | `ProtocolVersion` | - |
| `GetCapabilities` | `0x0002` | `()` | `DeviceCapabilities` | - |
| `Reboot` | `0x0003` | `()` | `()` | - |
| `BootloaderJump` | `0x0004` | `()` | `()` | - |
| `StorageReset` | `0x0005` | `StorageResetMode` | `RynkResult` | - |

> Lock variants (`GetLockStatus`, `UnlockRequest`, `LockRequest`)
> reserved for v2 at slots `0x0006`, `0x0007`, `0x0008`.

### Keymap — `0x01xx` (includes encoder)

| Cmd | Hex | Request | Response | Feature |
|---|---|---|---|---|
| `GetKeyAction` | `0x0101` | `KeyPosition` | `KeyAction` | - |
| `SetKeyAction` | `0x0102` | `SetKeyRequest` | `RynkResult` | - |
| `GetDefaultLayer` | `0x0103` | `()` | `u8` | - |
| `SetDefaultLayer` | `0x0104` | `u8` | `RynkResult` | - |
| `GetEncoderAction` | `0x0105` | `GetEncoderRequest` | `EncoderAction` | - |
| `SetEncoderAction` | `0x0106` | `SetEncoderRequest` | `RynkResult` | - |
| `GetKeymapBulk` | `0x0107` | `GetKeymapBulkRequest` | `GetKeymapBulkResponse` | `bulk` |
| `SetKeymapBulk` | `0x0108` | `SetKeymapBulkRequest` | `RynkResult` | `bulk` |

### Macro / Combo / Morse / Fork / Behavior / Connection / Status

See `rmk-types/src/protocol/rynk/cmd.rs` for the canonical enum. Each
group is sequential under its hex prefix:

- `0x02xx` — macro
- `0x03xx` — combo (incl. bulk variants)
- `0x04xx` — morse (incl. bulk variants)
- `0x05xx` — fork
- `0x06xx` — behavior config
- `0x07xx` — connection / BLE profile management
- `0x08xx` — runtime status (layer, matrix, battery, peripheral, plus
  snapshot getters for wpm/sleep/led)

### Topics — `0x80xx` (server → host push)

| Cmd | Hex | Payload | Source event | Feature |
|---|---|---|---|---|
| `LayerChange` | `0x8001` | `u8` | `LayerChangeEvent` | - |
| `WpmUpdate` | `0x8002` | `u16` | `WpmUpdateEvent` | - |
| `ConnectionChange` | `0x8003` | `ConnectionType` | `ConnectionChangeEvent` | - |
| `SleepState` | `0x8004` | `bool` | `SleepStateEvent` | - |
| `LedIndicator` | `0x8005` | `LedIndicator` | `LedIndicatorEvent` | - |
| `BatteryStatusTopic` | `0x8006` | `BatteryStatus` | `BatteryStatusEvent` | `_ble` |
| `BleStatusChangeTopic` | `0x8007` | `BleStatus` | `BleStatusChangeEvent` | `_ble` |

Dispatch uses `is_topic()` (`cmd as u16 & 0x8000 != 0`).

Each non-persistent topic (`WpmUpdate`, `SleepState`, `LedIndicator`)
also has a Get-equivalent in the `0x08xx` Status group
(`GetWpm = 0x0805`, `GetSleepState = 0x0806`, `GetLedIndicator = 0x0807`).
The firmware runs `run_topic_snapshot` next to the transports; it
subscribes to the three events and latches each payload so the host
can probe the latest cached value without waiting for the next push.

## Framing — how a receiver knows the message is over

**`LEN` in the header is authoritative.** The buffer is a *maximum*
frame size, not a fill target.

```
1. Read bytes into a buffer until you have ≥ 5 bytes (header).
2. Parse header to learn LEN.
3. Continue reading until total = 5 + LEN bytes received.
4. Dispatch the frame.
5. Reset state. Bytes beyond `5 + LEN` are the start of the next
   frame — push them back into the buffer.
```

**USB bulk.** The driver delivers packets up to MPS (64 B FS / 512 B
HS) per `read_packet` call. The protocol layer doesn't need short-
packet / ZLP termination for framing — `LEN` decides. But the USB
*driver* needs a ZLP after an exact-multiple-of-MPS bulk-IN write so
the host's URB completes; this is transmit-side, transparent to the
protocol.

**BLE GATT.** Each `output_data` write or `input_data` notify arrives
as a discrete slice up to MTU − 3 (244 B for typical 247 B MTU).
Receive loop:

```rust
loop {
    let chunk = RYNK_RX_CHANNEL.receive().await;
    buf.extend_from_slice(&chunk);
    if buf.len() < 5 { continue; }
    let len = u16::from_le_bytes([buf[3], buf[4]]) as usize;
    if buf.len() < 5 + len { continue; }
    let frame = &buf[..5 + len];
    // dispatch ...
    buf.drain(..5 + len);
}
```

No COBS, no delimiter, no escape sequences. `LEN` tells you exactly
when the frame ends; the buffer naturally holds at most one frame in
progress plus the start of the next.

## Buffer sizing

Every wire-payload type derives `postcard::MaxSize`. The constant
`RYNK_MIN_BUFFER_SIZE` is the maximum of all request and response sizes
plus the 5-byte header, computed at compile time over the union of
enabled features. See
[`rmk-types/src/protocol/rynk/buffer.rs`](../rmk-types/src/protocol/rynk/buffer.rs).

Users can over-provision via `[rmk] rynk_buffer_size` in
`keyboard.toml`. Under-provisioning fails the build via the const
`assert!` in `rmk/src/host/rynk/mod.rs`.

## Versioning

| Concept | Mechanism |
|---|---|
| **Version negotiation** | `Cmd::GetVersion` → `ProtocolVersion { major: u8, minor: u8 }`. Host bails on major mismatch or `minor > supported`. |
| **Frozen shape** | `GetVersion` payload shape is **permanent** — guarded by `wire_values.snap`. |
| **Capability discovery** | `Cmd::GetCapabilities` → `DeviceCapabilities`. Host gates Cmd calls on the bit-flag fields and limit fields returned here. |
| **New Cmd** | Minor bump. Append a variant in the next free slot of its `0x0Nxx` group. |
| **Reshaping a struct** | Major bump. Older hosts must be rejected at `GetVersion`. |

`postcard` is **not** forward-compatible across field appends. CI
fails the build if `wire_values.snap` changes without
`ProtocolVersion::CURRENT` also being bumped.

## USB descriptors

- Vendor class: `bInterfaceClass = 0xFF`, subclass/protocol = `0x00`.
- One BULK IN + one BULK OUT endpoint per device.
- MS OS 2.0 descriptor set published with `CompatibleId = "WINUSB"` and
  `DeviceInterfaceGUIDs = {F5F5F5F5-1234-5678-9ABC-DEF012345678}` (REG_MULTI_SZ).
- Vendor code byte for the MSOS descriptor request: `0x01`.

## BLE GATT

- Service UUID: `F5F50001-0000-0000-0000-000000000000`
- `input_data` characteristic: `F5F50002-…`, properties = `read | notify`,
  variable-length `Vec<u8, 244>`.
- `output_data` characteristic: `F5F50003-…`, properties = `read | write
  | write-without-response`, variable-length `Vec<u8, 244>`.

Both characteristic value types are sized at `MTU − 3 = 244` for the
typical 247-byte negotiated MTU. Larger MTUs round trip more bytes per
notify/write but cap at the characteristic's declared length.
