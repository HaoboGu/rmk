# RMK Communication Protocol Design Specification

**Status**: RFC (Request for Comments)
**Date**: 2026-03-01

## Abstract

This document specifies a new host communication protocol for RMK, replacing the current Via/Vial protocol. The new protocol uses postcard-rpc type-level endpoint definitions over raw USB vendor-class bulk endpoints and BLE serial, supports bidirectional communication with device-initiated event notifications (Topics), and serializes RMK's native types directly via postcard with zero lossy conversion. It includes physical-key-based security with auto-timeout and structured capability discovery.

---

## 1. Why Replace Vial?

The current Vial/VIA implementation (`rmk/src/host/via/`) has fundamental structural limitations:

**1.1 32-byte HID payload ceiling** — All data must fit in 32-byte HID reports (`UsbVialReaderWriter` uses `HidReaderWriter<'d, D, 32, 32>`), forcing ~28-byte effective payload with paging logic for bulk operations. Throughput is severely limited.

**1.2 Lossy QMK keycode conversion** — RMK's `KeyAction`/`Action` types are far richer than Via's 16-bit keycode space. `KeyAction::Tap`, full Morse patterns (15 steps), and `Fork` are not representable in QMK keycodes. The conversion in `rmk/src/host/via/keycode_convert.rs` is lossy in both directions.

**1.3 Incomplete feature coverage** — Fork/KeyOverride handlers are stubs, lighting/RGB returns "not supported", battery/split status/BLE profile/WPM/sleep state have no host-facing channel. Macro count is hardcoded to 32, `VIAL_COMBO_MAX_LENGTH` fixed to 4 (Vial limitation vs RMK's configurable `COMBO_MAX_LENGTH`).

**1.4 No notifications** — Pure request/response. The host cannot receive layer changes, WPM updates, battery state, or BLE connection events. RMK already has a rich internal event bus (`LayerChangeEvent`, `WpmUpdateEvent`, `BatteryStatusEvent`, `BleStateChangeEvent`, `SleepStateEvent`, etc. in `rmk/src/event/`) with no way to expose it to the host.

**1.5 Mixed endianness** — Via commands use big-endian, Vial sub-commands use little-endian, both in the same file (`byteorder::{BigEndian, LittleEndian}` in `rmk/src/host/via/mod.rs`).

**1.6 Three-layer dispatch nesting** — Via -> Vial -> VialDynamic, each with separate command enums and nested match arms.

**1.7 Tight coupling to Vial ecosystem** — Requires `vial.json`, `vial_keyboard_id`, LZMA-compressed keyboard definition transfer, and Vial GUI-specific protocol quirks.

---

## 2. Goals

| ID | Goal | Rationale |
|----|------|-----------|
| **G1** | **Full RMK feature coverage** | Expose all runtime-configurable features: keymap, macros, combos, morse/tap-dance, forks, encoders, behavior settings, connection management, device control, status queries |
| **G2** | **Native types, zero conversion loss** | Use RMK's own `KeyAction`, `Action`, `KeyCode`, `ComboConfig`, `Fork`, `Morse` types on the wire via postcard. No QMK keycode intermediary |
| **G3** | **Transport-agnostic over byte streams** | Single protocol works over USB raw vendor-class bulk endpoints and BLE serial (NUS). Optional HID fallback for WebHID environments |
| **G4** | **Bidirectional with notifications from day 1** | Topic-based device-to-host events for layer changes, WPM, battery, BLE state, connection changes, sleep state, split peripheral status |
| **G5** | **Self-describing and discoverable** | Device reports capabilities, topology, and limits at runtime. No separate `vial.json` needed |
| **G6** | **Embedded-friendly** | `no_std` compatible, zero heap allocation, bounded buffer sizes. Must work on nRF52840, RP2040, STM32, ESP32 |
| **G7** | **Secure by default** | Device starts locked; writes require physical unlock with auto-timeout |
| **G8** | **Forward-compatible extensibility** | Adding endpoints/fields must not break existing host tools. Schema hashing prevents silent type mismatches |
| **G9** | **Tooling-friendly** | Enable host tools in Rust (native + WASM/WebUSB), Python, TypeScript via schema generation |

---

## 3. Non-Goals (v1)

1. End-to-end encryption.
2. Backward compatibility with existing Via/Vial desktop clients and Via/Vial protocol. New protocol and Vial are mutually exclusive via feature gate("vial" and "rmk_protocol").
3. Lighting/RGB configuration (deferred to v1.1).
4. Firmware OTA over protocol.
5. Full Ergot dependency (design with Ergot addressing in mind, do not depend on it).
6. Live key event stream for matrix tester (deferred).
7. Multi-segment routing for split (v1 is point-to-point; central only).
8. Explicit save/discard pattern. Configuration changes are infrequent; write-through to flash on each `Set*` operation is sufficient and avoids unnecessary complexity.

---

## 4. Design Principles

### DP1. Type-level endpoint definitions (postcard-rpc pattern)

Each protocol operation is a Rust type implementing the `Endpoint` trait, not a hand-assigned opcode. Following postcard-rpc's design:

```rust
// Shared ICD (Interface Control Document), in rmk-types
use postcard_rpc::{endpoints, topics, TopicDirection};

endpoints! {
    list = ENDPOINT_LIST;
    | EndpointTy     | RequestTy       | ResponseTy         | Path                |
    | ----------     | ---------       | ----------         | ----                |
    | GetVersion     | ()              | ProtocolVersion    | "sys/version"       |
    | GetCapabilities| ()              | DeviceCapabilities | "sys/caps"          |
    | GetKeyAction   | KeyPosition     | KeyAction          | "keymap/get"        |
    | SetKeyAction   | SetKeyRequest   | RmkResult          | "keymap/set"        |
    // ... each new operation = one new row, purely additive
}
```

On the wire, the PATH + Schema are hashed (FNV1a-64) into a compact `Key` (1-8 bytes). This means:
- **Adding a new endpoint** = one Rust type + one handler. No central ID registry.
- **Type safety**: Request/Response types checked at compile time.
- **Schema change = different key**: if types change, the hash changes, preventing silent incompatibility.

### DP2. Transport-agnostic core over byte streams

The protocol uses COBS framing over byte streams, eliminating the need for manual fragmentation/reassembly. RMK's split serial communication already uses this pattern (`postcard::to_slice_cobs` / `postcard::take_from_bytes_cobs` in `rmk/src/split/serial/mod.rs`).

Two transport implementations are planned:
- **USB**: Raw vendor-class bulk endpoints (class `0xFF`), with MS OS descriptors for automatic WinUSB binding on Windows. This provides WebUSB compatibility for browser-based configurators and uses a simpler descriptor (1 interface, 2 endpoints) than CDC-ACM. postcard-rpc provides a native `embassy-usb` server implementation for this transport that handles framing without COBS (USB bulk packets are self-delimiting).
- **BLE serial**: NUS-like GATT service with RX/TX characteristics, implementing `embedded_io_async::{Read, Write}` for COBS-framed byte streams.

`ProtocolService` is generic over transport via postcard-rpc's `WireTx`/`WireRx` traits. Each transport implements these traits, abstracting framing differences.

### DP3. Endpoints + Topics (two-primitive model)

Following postcard-rpc and Ergot's architecture:

| Pattern | Direction | Use Case |
|---------|-----------|----------|
| **Endpoint** (Request -> Response) | Host <-> Device | Config queries, keymap get/set, macro CRUD |
| **Topic** (fire-and-forget) | Device -> Host | Layer change, battery update, BLE state, connection state |

This maps directly to RMK's internal event pub/sub system (`EventPublisher` / `EventSubscriber` traits in `rmk/src/event/mod.rs`).

### DP4. RMK native types on the wire

All payload types are from `rmk-types`, serialized directly with postcard:
- `KeyAction`, `Action`, `KeyCode` (no QMK 16-bit conversion)
- `ComboConfig`, `MorseProfile`, `Fork` (full fidelity, no truncation)
- `BehaviorConfig`, `EncoderAction`, `MacroOperation`

These types already derive `serde::Serialize`, `serde::Deserialize`, and `postcard::MaxSize` in `rmk-types/src/action.rs`. `postcard::Schema` will be added.

### DP5. Deterministic resource usage

- `no_std` compatible, zero heap allocation in the protocol core path.
- All variable-length collections use `heapless::Vec<T, N>` with compile-time capacity bounds.
- Bounded buffer sizes: firmware allocates fixed-size RX/TX buffers (configurable in `keyboard.toml`, default 128 bytes). The original Vial protocol used 32-byte HID reports; the new protocol's default is larger because postcard-rpc frame overhead (discriminant + key + seq_no) plus payload for structs like `DeviceCapabilities` requires more space. Users can reduce buffer size to 32 bytes for HID fallback transport compatibility.
- postcard serialization is zero-alloc and operates on fixed buffers.
- COBS encoding/decoding is in-place, no additional buffer beyond the frame buffer.

### DP6. Explicit error semantics

Errors use postcard-rpc's standard error path (reserved `"error"` key), extended with RMK-specific variants. Unknown endpoints return `WireError::UnknownKey` — natural graceful degradation.

### DP7. Feature-gate isolation

```toml
rmk = { features = ["rmk_protocol"] }   # new protocol
# OR
rmk = { features = ["vial"] }           # legacy Vial
```

Mutually exclusive. No protocol-multiplexing logic, no compatibility shims. Only one protocol's code is compiled.

### DP8. Forward-compatible extensibility

Within a protocol major version:
- **New endpoints** added freely (hash produces new key, old hosts never send it).
- **New fields** use `#[serde(default)]` — older hosts/firmware ignore unknown trailing bytes.
- **Removing endpoints** allowed — host gets `UnknownKey` and adapts.
- **Major version bump** reserved for wire-format breaking changes only (extremely rare).

### DP9. Security by physical unlock

Device starts locked. Write operations require physical key unlock with auto-timeout. Reuses the existing `VialLock` mechanism (`rmk/src/host/via/vial_lock.rs`).

---

## 5. Design Decision Analysis

### Decision 1: Dispatch Model

The dispatch model determines how incoming messages are routed to handlers.

#### Option A: Module ID + Command ID (manual opcodes)

```
[module_id: u8][command_id: u8][payload...]
```

| Aspect | Assessment |
|--------|-----------|
| Debuggability | **Good** — opcodes are readable in hex dumps |
| Implementation | Simple match-based dispatch |
| Type safety | **None** — manual byte packing, no compile-time checks |
| Schema evolution | **Manual** — changing a type doesn't automatically signal incompatibility |
| Extensibility | Requires central ID registry; collision risk with multiple contributors |

#### Option B: Hash-based Key dispatch (postcard-rpc pattern)

```
[key: 1-8B FNV1a hash of PATH + Schema][payload...]
```

| Aspect | Assessment |
|--------|-----------|
| Debuggability | Moderate — hashes are opaque, but PATH strings provide documentation |
| Implementation | Requires postcard-rpc or equivalent dispatch table |
| Type safety | **Excellent** — compile-time Request/Response type agreement |
| Schema evolution | **Automatic** — type change = hash change = wire-incompatible |
| Extensibility | Purely additive, no central registry, no collision risk |

**Verdict: Option B.** The automatic schema mismatch detection is critical for preventing subtle bugs between mismatched firmware/host versions. Path prefixes (`keymap/get`, `combo/set`) provide natural grouping for documentation. This is the approach used by postcard-rpc and Ergot.

> **Implementation note**: While postcard-rpc provides `define_dispatch!` macro and `Server` struct for automatic key-based dispatch, these require static (non-generic) context types. RMK's `ProtocolService` holds `&RefCell<KeyMap<ROW, COL, NUM_LAYER, NUM_ENCODER>>` with compile-time generic parameters, making it incompatible with `define_dispatch!`'s static context model. Therefore, RMK implements its own dispatch loop using the same key-based matching pattern, while reusing postcard-rpc's `endpoints!`/`topics!` macro definitions, wire format, key hashing, and serialization infrastructure.

### Decision 2: Capability Discovery

The host needs to know what the connected keyboard supports (layers, encoders, BLE, storage, lighting).

#### Option A: Protocol Version Only

`GetVersion -> { major, minor }` — host maintains a hardcoded table mapping version to features.

**Verdict: NOT suitable.** RMK firmware is too configurable per-build. Two builds at the same version with different `cfg` flags are indistinguishable.

#### Option B: Protocol Version + Feature Bitmap

`GetVersion -> { protocol: u8, features: u64 }` — each bit = one feature.

**Verdict: Partially suitable.** Handles feature presence but not parametric info (knows "has encoders" but not "has 2 encoders"). Limited to 64 features.

#### Option C: Endpoint Discovery (postcard-rpc DeviceMap)

`GetEndpoints -> [Key, Key, ...]` — the host learns exactly which endpoints exist.

**Verdict: Not included.** The host can probe endpoint existence but still can't know parametric limits without calling each endpoint. `GetCapabilities` provides sufficient discovery, and sending to an unsupported endpoint already returns `WireError::UnknownKey`, which is adequate for feature probing.

#### Option D: Structured Capabilities Endpoint (Recommended)

```rust
endpoint!(GetCapabilities, (), DeviceCapabilities, "sys/caps");

#[derive(Serialize, Deserialize, Schema)]
struct DeviceCapabilities {
    num_layers: u8,
    num_rows: u8,
    num_cols: u8,
    num_encoders: u8,
    max_combos: u8,
    max_macros: u8,
    macro_space_size: u16,
    max_morse: u8,
    max_forks: u8,
    has_storage: bool,
    has_split: bool,
    num_split_peripherals: u8,
    has_ble: bool,
    num_ble_profiles: u8,
    has_lighting: bool,
    max_payload_size: u16,
    // Future fields added with #[serde(default)]
}
```

**Verdict: Best fit.** The host gets feature availability AND parametric limits in a single round-trip. Combined with `GetVersion` for wire compatibility check. Two endpoints, two round-trips at connect time.

### Decision 3: Notification Mechanism

RMK has a rich internal event bus. The question is whether and how to expose it to the host in v1.

#### Option A: No notifications in v1 (polling only)

Defer all event streaming. Host polls status endpoints for changes.

| Aspect | Assessment |
|--------|-----------|
| Implementation complexity | Minimal |
| Host experience | **Poor** — layer indicator, battery widget require constant polling |
| Bandwidth | Wastes bandwidth on empty polls |

#### Option B: Subscribe/unsubscribe with bitmap

Host sends subscription requests with event type bitmaps. Keyboard pushes matching events.

| Aspect | Assessment |
|--------|-----------|
| Implementation complexity | Medium — subscription state management on firmware |
| Host experience | Good — only receives subscribed events |
| RAM cost | Requires per-connection subscription bitmap |

#### Option C: Topics (fire-and-forget, always-on)

Firmware publishes Topic frames for all state changes to all connected transports. No subscription management.

| Aspect | Assessment |
|--------|-----------|
| Implementation complexity | Low — firmware publishes, host ignores what it doesn't need |
| Host experience | Good — receives all events, filters client-side |
| RAM cost | Minimal — no subscription state |
| Bandwidth | Slightly more than B, but events are small and infrequent |

**Verdict: Option C (Topics).** RMK's internal event bus already publishes all events. The `ProtocolService` simply subscribes to internal events and forwards them as Topic frames. No subscription management needed on firmware. Events are small (layer number, battery percentage, BLE state enum) and infrequent, so the bandwidth overhead of always-on is negligible. Primary v1 use cases: battery state reporting and connection state changes.

---

## 6. Wire Format Specification

### 6.1 Framing: COBS over byte streams

Each message is a COBS-encoded frame over the byte stream, terminated by a `0x00` delimiter. COBS guarantees no `0x00` inside the encoded frame, providing reliable frame boundaries with ~0.4% overhead.

### 6.2 Frame structure

```
COBS-encoded frame:
+-------------+----------+----------+----------------------+
| discriminant|   key    |  seq_no  |  postcard payload    |
|    (1B)     | (1-8B)   | (1-4B)   |    (variable)        |
+-------------+----------+----------+----------------------+
|<------------- COBS encoded ----------------------------->|
[0x00 frame delimiter]
```

### 6.3 Discriminant byte

`0bNNMM_VVVV`:
- `NN` (2 bits): key length as `2^N` bytes (1, 2, 4, or 8)
- `MM` (2 bits): sequence number length as `2^M` bytes (1, 2, or 4)
- `VVVV` (4 bits): wire format version (currently `0000`)

### 6.4 Key

FNV1a-64 hash of (endpoint PATH + type Schema), XOR-compressed to the length indicated by NN. The minimum key length is chosen at compile time to be collision-free across all defined endpoints.

### 6.5 Sequence number

Little-endian. Host sets it on request; firmware echoes it in response. Used for request/response correlation. Topics use seq_no = 0.

### 6.6 Message types (FrameKind)

Encoded within the key space:
- Endpoint Request — host to device
- Endpoint Response — device to host (echoes seq_no)
- Topic Message — device to host (fire-and-forget, seq_no = 0)
- Protocol Error — device to host (error key)

### 6.7 Maximum frame size

Bounded by firmware RX/TX buffer (configurable in `keyboard.toml`, default 128 bytes). Advertised in `DeviceCapabilities.max_payload_size`. For most operations (single key get/set), messages are well under 128 bytes. The original Vial protocol used 32-byte HID reports; the new protocol defaults to 128 bytes because postcard-rpc frame overhead (up to 13 bytes for header) plus payload for multi-field structs like `DeviceCapabilities` exceeds 32 bytes. Bulk operations (full keymap dump) may benefit from a larger configured buffer. The optional HID fallback transport (future) constrains frames to 32 bytes.

### 6.8 No manual fragmentation

Unlike HID-based Vial (28-byte chunks), raw USB bulk and BLE serial provide higher-bandwidth channels. For USB bulk, packets are self-delimiting and do not need COBS. For BLE serial, COBS framing provides frame boundaries over the byte stream. No manual fragmentation/reassembly layer needed in either case. The protocol layer does not deal with MTU — that is handled by the transport layer.

### 6.9 Optional HID transport (future)

For WebHID fallback: COBS frames chunked into fixed-size HID reports with a 1-byte length prefix per chunk. Lower priority, not required for v1.

---

## 7. Endpoint Inventory (v1)

### 7.1 System

| Endpoint | Request | Response | Path | Permission |
|----------|---------|----------|------|------------|
| GetVersion | `()` | `ProtocolVersion` | `sys/version` | ReadOnly |
| GetCapabilities | `()` | `DeviceCapabilities` | `sys/caps` | ReadOnly |
| GetLockStatus | `()` | `LockStatus` | `sys/lock_status` | ReadOnly |
| Unlock | `()` | `UnlockChallenge` | `sys/unlock` | ReadOnly |
| Lock | `()` | `()` | `sys/lock` | ReadOnly |
| Reboot | `()` | `()` | `sys/reboot` | Dangerous |
| BootloaderJump | `()` | `()` | `sys/bootloader` | Dangerous |
| StorageReset | `StorageResetMode` | `()` | `sys/storage_reset` | Dangerous |

### 7.2 Keymap

For endpoints with multi-field request payloads, v1 uses named request structs (instead of tuples) so the schema is self-describing and unambiguous.

| Endpoint | Request | Response | Path | Permission |
|----------|---------|----------|------|------------|
| GetKeyAction | `KeyPosition` | `KeyAction` | `keymap/get` | ReadOnly |
| SetKeyAction | `SetKeyRequest` | `RmkResult` | `keymap/set` | RequiresUnlock |
| GetKeymapBulk | `BulkRequest` | `heapless::Vec<KeyAction, MAX_BULK>` | `keymap/bulk_get` | ReadOnly |
| SetKeymapBulk | `SetKeymapBulkRequest` | `RmkResult` | `keymap/bulk_set` | RequiresUnlock |
| GetLayerCount | `()` | `u8` | `keymap/layer_count` | ReadOnly |
| GetDefaultLayer | `()` | `u8` | `keymap/default_layer` | ReadOnly |
| SetDefaultLayer | `u8` | `RmkResult` | `keymap/set_default_layer` | RequiresUnlock |
| ResetKeymap | `()` | `RmkResult` | `keymap/reset` | Dangerous |

### 7.3 Encoder

| Endpoint | Request | Response | Path | Permission |
|----------|---------|----------|------|------------|
| GetEncoderAction | `GetEncoderRequest` | `EncoderAction` | `encoder/get` | ReadOnly |
| SetEncoderAction | `SetEncoderRequest` | `RmkResult` | `encoder/set` | RequiresUnlock |

### 7.4 Macro

| Endpoint | Request | Response | Path | Permission |
|----------|---------|----------|------|------------|
| GetMacroInfo | `()` | `MacroInfo` | `macro/info` | ReadOnly |
| GetMacro | `u8` | `MacroData` | `macro/get` | ReadOnly |
| SetMacro | `SetMacroRequest` | `RmkResult` | `macro/set` | RequiresUnlock |
| ResetMacros | `()` | `RmkResult` | `macro/reset` | Dangerous |

### 7.5 Combo

| Endpoint | Request | Response | Path | Permission |
|----------|---------|----------|------|------------|
| GetCombo | `u8` | `ComboConfig` | `combo/get` | ReadOnly |
| SetCombo | `SetComboRequest` | `RmkResult` | `combo/set` | RequiresUnlock |
| ResetCombos | `()` | `RmkResult` | `combo/reset` | Dangerous |

### 7.6 Morse / Tap-Dance

| Endpoint | Request | Response | Path | Permission |
|----------|---------|----------|------|------------|
| GetMorse | `u8` | `MorseConfig` | `morse/get` | ReadOnly |
| SetMorse | `SetMorseRequest` | `RmkResult` | `morse/set` | RequiresUnlock |
| ResetMorse | `()` | `RmkResult` | `morse/reset` | Dangerous |

### 7.7 Fork (Key Override)

| Endpoint | Request | Response | Path | Permission |
|----------|---------|----------|------|------------|
| GetFork | `u8` | `ForkConfig` | `fork/get` | ReadOnly |
| SetFork | `SetForkRequest` | `RmkResult` | `fork/set` | RequiresUnlock |
| ResetForks | `()` | `RmkResult` | `fork/reset` | Dangerous |

### 7.8 Behavior Settings

| Endpoint | Request | Response | Path | Permission |
|----------|---------|----------|------|------------|
| GetBehaviorConfig | `()` | `BehaviorConfig` | `behavior/get` | ReadOnly |
| SetBehaviorConfig | `BehaviorConfig` | `RmkResult` | `behavior/set` | RequiresUnlock |

### 7.9 Connection Management

| Endpoint | Request | Response | Path | Permission |
|----------|---------|----------|------|------------|
| GetConnectionInfo | `()` | `ConnectionInfo` | `conn/info` | ReadOnly |
| SetConnectionType | `ConnectionType` | `RmkResult` | `conn/set_type` | RequiresUnlock |
| SwitchBleProfile | `u8` | `RmkResult` | `conn/switch_ble` | RequiresUnlock |
| ClearBleProfile | `u8` | `RmkResult` | `conn/clear_ble` | Dangerous |

### 7.10 Status (read-only)

| Endpoint | Request | Response | Path | Permission |
|----------|---------|----------|------|------------|
| GetBatteryStatus | `()` | `BatteryStatusEvent` | `status/battery` | ReadOnly |
| GetCurrentLayer | `()` | `u8` | `status/layer` | ReadOnly |
| GetMatrixState | `()` | `MatrixState` | `status/matrix` | ReadOnly |
| GetSplitStatus | `()` | `SplitStatus` | `status/split` | ReadOnly |

---

## 8. Topic Inventory (v1 Events)

Topics are fire-and-forget device-to-host notifications. Each maps directly to an existing RMK internal event. Primary v1 use cases are **battery state** and **connection state** reporting; all listed Topics are included in v1 as the implementation cost is minimal once the bridging infrastructure exists.

| Topic | Payload | Path | Internal Event |
|-------|---------|------|----------------|
| LayerChange | `LayerChangePayload { layer: u8 }` | `event/layer` | `LayerChangeEvent` |
| WpmUpdate | `WpmPayload { wpm: u16 }` | `event/wpm` | `WpmUpdateEvent` |
| BatteryStatus | `BatteryStatusEvent` | `event/battery` | `BatteryStatusEvent` |
| BleStateChange | `BleStatePayload { ... }` | `event/ble_state` | `BleStateChangeEvent` |
| BleProfileChange | `BleProfilePayload { profile: u8 }` | `event/ble_profile` | `BleProfileChangeEvent` |
| ConnectionChange | `ConnectionPayload { ... }` | `event/connection` | `ConnectionChangeEvent` |
| SleepState | `SleepPayload { sleeping: bool }` | `event/sleep` | `SleepStateEvent` |
| LedIndicator | `LedPayload { indicator: LedIndicator }` | `event/led` | `LedIndicatorEvent` |

### Implementation pattern

Topics use a **single-writer architecture** to prevent concurrent writes to the transport. The `ProtocolService::run()` async loop uses `embassy_futures::select` to multiplex between transport reads (endpoint requests) and internal event subscribers (topic sources). Only one writer exists — the `ProtocolService` itself — ensuring responses and topic frames never race on the same byte stream.

Event subscribers must be created once at `ProtocolService` initialization and held as fields. Creating subscribers inside the select loop would cause immediate returns due to the watch-based event system's `changed()` semantics.

```rust
// In ProtocolService, subscribers created once in new()
struct ProtocolService<...> {
    layer_sub: LayerChangeEventSubscriber,
    battery_sub: BatteryStatusEventSubscriber,
    // ...
}

// In ProtocolService::run(), single select loop handles both
loop {
    select! {
        // Handle endpoint requests from transport
        frame = read_frame(&mut self.transport) => {
            let response = self.dispatch(frame);
            self.transport.write(&response).await;
        }
        // Forward internal events as Topic frames
        event = self.layer_sub.next_event() => {
            let frame = topic_frame::<LayerChangeTopic>(&LayerChangePayload { layer: event.layer });
            self.transport.write(&frame).await;
        }
        event = self.battery_sub.next_event() => {
            let frame = topic_frame::<BatteryStatusTopic>(&event.into());
            self.transport.write(&frame).await;
        }
        // ... other event subscribers
    }
}
```

Response writes are prioritized over topic writes in the select order to prevent notification bursts from starving request/response traffic. Transport write failures (e.g., disconnected host) are logged but do not crash the service.

Host-to-device Topics are deferred beyond v1.

---

## 9. Persistence Model

Write operations (`Set*` endpoints) update in-memory state AND persist to flash immediately via `FLASH_CHANNEL`. This is write-through persistence, the same model used by the current Vial implementation.

Since configuration changes from the host tool are infrequent (a user configuring their keyboard, not a continuous stream), write-through is sufficient and avoids the complexity of dirty-state tracking, explicit save/discard endpoints, and disconnect-while-dirty edge cases.

The existing `FLASH_CHANNEL` (`rmk/src/channel.rs`) and `Storage::run()` loop (`rmk/src/storage/mod.rs`) are reused. `FlashOperationMessage::VialMessage` will be renamed to `FlashOperationMessage::HostMessage` to be protocol-neutral.

> **Implementation note**: The current `FlashOperationMessage::ResetLayout` variant is a no-op at runtime (ignored in `Storage::run()`, only effective at startup via `clear_layout` flag). For the `ResetKeymap` endpoint to work correctly, a new runtime keymap reset path must be implemented in the storage layer that erases stored keymap keys and reloads defaults from the compiled-in keymap.

---

## 10. Security Model

### 10.1 Permission Levels

Three levels, assigned per endpoint (see Section 7):

| Level | Description | Examples |
|-------|-------------|----------|
| `ReadOnly` | Always allowed | Status queries, get operations, version, capabilities |
| `RequiresUnlock` | Write operations, require unlock | Keymap set, behavior set, connection set |
| `Dangerous` | Destructive operations, require unlock | Storage reset, bootloader jump, reboot |

### 10.2 Lock State Machine

```
              Unlock request
  Locked ─────────────────────> AwaitingPhysicalKey
    ^                               |
    |  auto-timeout (90s)           | physical keys pressed
    |  or explicit Lock             v
    |  or disconnect            Unlocked
    +-------------------------------+
```

### 10.3 Physical Key Unlock

Reuse the existing `VialLock` mechanism from `rmk/src/host/via/vial_lock.rs`:
- `Unlock` endpoint returns positions of required physical keys
- User presses the keys within a timeout window
- `GetLockStatus` returns current state (`locked`, `awaiting_keys`, `unlocked`)

### 10.4 Auto-timeout

Default 90 seconds of inactivity (no write operations) triggers automatic re-lock. Configurable via build-time constant.

### 10.5 Error on locked write

Attempting a write while locked returns `RmkError::BadState`.

---

## 11. Versioning and Capability Discovery

### 11.1 Version Structure

```rust
#[derive(Serialize, Deserialize, Schema)]
struct ProtocolVersion {
    major: u8,
    minor: u8,
}
```

### 11.2 Version Semantics

- **Major**: wire format changes only (discriminant layout, COBS encoding, key computation). Extremely rare. Mismatch = hard stop.
- **Minor**: additive changes — new endpoints, new struct fields with `#[serde(default)]`, deprecated endpoints. Forward and backward compatible.

### 11.3 Schema Hashing

postcard-rpc's key computation hashes both PATH and type Schema. If a request/response type changes, its key changes automatically. This prevents silent deserialization mismatch between firmware/host versions.

### 11.4 Connection Handshake

```
1. Host -> GetVersion -> ProtocolVersion { major, minor }
   - major mismatch -> refuse connection ("update firmware/tool")
   - minor mismatch -> proceed (forward-compatible)

2. Host -> GetCapabilities -> DeviceCapabilities { ... }
   - Host adapts UI based on actual capabilities
   - Unknown fields in newer firmware -> ignored by older host
   - Missing fields from older firmware -> defaults used by newer host

3. Host -> GetLockStatus -> LockStatus
   - Determine if device is already unlocked
```

### 11.5 DeviceCapabilities

```rust
#[derive(Serialize, Deserialize, Schema)]
struct DeviceCapabilities {
    num_layers: u8,
    num_rows: u8,
    num_cols: u8,
    num_encoders: u8,
    max_combos: u8,
    max_macros: u8,
    macro_space_size: u16,
    max_morse: u8,
    max_forks: u8,
    has_storage: bool,
    has_split: bool,
    num_split_peripherals: u8,
    has_ble: bool,
    num_ble_profiles: u8,
    has_lighting: bool,
    max_payload_size: u16,
    // Future fields added with #[serde(default)]
}
```

Values are sourced from compile-time constants already emitted by `rmk/build.rs`: `NUM_LAYER`, `NUM_ROW`, `NUM_COL`, `COMBO_MAX_NUM`, `FORK_MAX_NUM`, `MORSE_MAX_NUM`, `MACRO_SPACE_SIZE`, `NUM_ENCODER`.

### 11.6 Version Compatibility Matrix

| Scenario | major match? | Action |
|----------|-------------|--------|
| New host + old firmware | Yes | Host probes endpoints; missing ones -> degrade gracefully |
| Old host + new firmware | Yes | New endpoints unused; new struct fields ignored |
| Major mismatch | No | Hard stop, prompt user to update |

---

## 12. Error Model

### 12.1 Standard Error Path

Errors follow postcard-rpc's built-in error mechanism:
- Reserved error key (`"error"`) carries `WireError`
- Includes: frame too short, deserialization failure, unknown key, handler spawn failure

### 12.2 RMK Error Enum

```rust
#[derive(Serialize, Deserialize, Schema)]
enum RmkError {
    InvalidParameter,  // valid endpoint but bad parameter values
    BadState,          // operation not valid in current state (e.g., locked)
    Busy,              // temporary contention (retry recommended)
    StorageError,      // flash read/write failure
    InternalError,     // unexpected firmware error
}
```

Note: `Unsupported` is NOT an `RmkError` variant — an unsupported endpoint is signaled by `WireError::UnknownKey`.

### 12.3 Retry Guidance

| Error | Retry? | Guidance |
|-------|--------|----------|
| `InvalidParameter` | No | Fix the parameter values |
| `BadState` | No | Change state first (e.g., unlock) |
| `Busy` | Yes | Bounded backoff, max 3 retries |
| `StorageError` | Maybe | May succeed after retry; persistent = hardware issue |
| `InternalError` | Maybe | Report bug if persistent |

---

## 13. Protocol Comparison Table

| Aspect | Vial (current) | QMK XAP | ZMK Studio | Ergot | **RMK Protocol** |
|--------|---------------|---------|------------|-------|------------------|
| Serialization | Raw bytes | Raw bytes + codegen | Protobuf | postcard | **postcard (serde)** |
| Transport | HID only | HID (future serial) | Serial + BLE | Any (COBS) | **USB raw bulk + BLE + HID** |
| Notifications | None (poll) | Planned | Yes | Topics | **Yes (Topics)** |
| Discovery | Separate JSON | Subsystem bitmask | Protobuf introspect | Key + NameHash | **Capability struct + schema hash** |
| Persistence | Write-through | N/A | Explicit save/discard | N/A | **Write-through** |
| Addressing | None | None | None | Network/Node/Socket | **Optional (future split)** |
| Error handling | Silent | Status codes | Protobuf status | ProtocolError(u16) | **Typed errors + seq correlation** |
| Split support | Separate protocol | N/A | N/A | Multi-node native | **Unified design (future)** |
| Language | C | C | C (Zephyr) | Rust (no_std) | **Rust (no_std serde)** |
| Security | Optional lock | N/A | N/A | N/A | **Physical unlock + auto-timeout** |
| Schema evolution | None | Manual versioning | Protobuf compat | Schema hashing | **Schema hashing (auto)** |

---

## 14. Architecture Integration Points

### 14.1 New module structure

```
rmk-types/src/protocol/
    mod.rs          (existing)
    vial.rs         (existing — Vial types)
    rmk.rs          (NEW — ICD types: endpoint defs, DeviceCapabilities, RmkError, etc.)

rmk/src/host/
    mod.rs          (existing — task spawning, feature-gated)
    via/            (existing — Vial implementation)
    protocol/       (NEW — new protocol implementation)
        mod.rs      (ProtocolService, dispatch loop)
        transport.rs (raw USB bulk and BLE serial adapters)
        topics.rs   (event bus to Topic bridging)
```

### 14.2 ProtocolService

Replaces `VialService` with the same structural pattern:
- Holds `&RefCell<KeyMap>` for in-memory keymap access
- Reads/writes over transport via postcard-rpc's `WireTx`/`WireRx` traits
- Implements custom key-based dispatch (postcard-rpc's `define_dispatch!` cannot be used because `ProtocolService` is generic over `ROW`, `COL`, `NUM_LAYER`, `NUM_ENCODER` const parameters — the macro requires static, non-generic context types)
- Uses `embassy_futures::select` to multiplex endpoint request handling and internal event subscriber polling (single-writer architecture for Topics)
- Manages lock state and write-through persistence via `FLASH_CHANNEL`
- Runs as an async future composed into `run_keyboard()`'s `futures::select_biased!` (not spawned via embassy Spawner, matching the existing `VialService` pattern)

### 14.3 Transport adapters

- **USB**: Raw vendor-class bulk endpoints (class `0xFF`, 1 interface, 2 bulk endpoints). Uses MS OS descriptors for automatic WinUSB driver binding on Windows. Natively WebUSB-compatible for browser-based configurators. Implements postcard-rpc's `WireTx`/`WireRx` traits. USB bulk transport uses packet-based framing (no COBS needed — USB bulk packets are self-delimiting).
- **BLE serial**: New GATT service with NUS-like RX/TX characteristics replaces `BleVialServer`. Implements `embedded_io_async::Read` + `embedded_io_async::Write`, wrapped to implement `WireTx`/`WireRx`. BLE serial transport uses COBS framing over the byte stream.
- `ProtocolService` is generic over postcard-rpc's `WireTx`/`WireRx` traits, abstracting transport and framing differences.

### 14.4 Event bus bridging

`ProtocolService` subscribes to internal events via the existing `SubscribableEvent` trait. All subscribers are created once during `ProtocolService::new()` and stored as struct fields (not re-created per loop iteration, due to watch-based subscriber semantics):
```rust
// Created once in ProtocolService::new()
let layer_sub = LayerChangeEvent::subscriber();
let wpm_sub = WpmUpdateEvent::subscriber();
let battery_sub = BatteryStatusEvent::subscriber();
// ... etc
```
In the main `run()` loop, `embassy_futures::select` multiplexes transport reads and event subscribers. When an event fires, the service serializes a Topic frame and writes it to the transport. This single-writer architecture ensures responses and topic frames never race on the same transport.

### 14.5 Storage integration

- `FlashOperationMessage::VialMessage` renamed to `FlashOperationMessage::HostMessage` (protocol-neutral)
- Write operations send `FlashOperationMessage` variants through `FLASH_CHANNEL` (`rmk/src/channel.rs`) immediately (write-through, same as current Vial)
- Storage task loop (`Storage::run()`) processes messages unchanged

### 14.6 Feature gate wiring

```toml
# rmk/Cargo.toml
[features]
host = []                              # shared base
vial = ["host"]                        # legacy Vial
vial_lock = ["vial"]                   # Vial unlock
rmk_protocol = ["host", "dep:postcard-rpc"]  # new protocol
```

### 14.7 Split keyboard consideration

v1: protocol runs only on the central keyboard. The central proxies status from peripherals via the existing `SplitMessage` system (`rmk/src/split/mod.rs`).

Future: Ergot-style addressing (`network_id: u16, node_id: u8, socket_id: u8`) could unify split and host communication. Central = node 1, peripherals = nodes 2+. This would allow host tools to query peripherals directly through the central.

### 14.8 Configuration

New `[protocol]` section in `keyboard.toml` (or extension of `[rmk]`):
```toml
[protocol]
type = "rmk"           # or "vial"
buffer_size = 128      # RX/TX buffer size in bytes (default: 128, min: 32 for HID fallback)
lock_timeout = 90      # auto-lock seconds
```

The `buffer_size` is configurable to allow users to adjust throughput vs RAM tradeoff. The default of 128 bytes accommodates postcard-rpc frame overhead (up to 13 bytes header) plus the largest standard response payloads. It is advertised as `DeviceCapabilities.max_payload_size`.

---

## 15. Implementation Roadmap

### Phase 1: ICD Types and postcard-rpc Integration

**Goal**: Define the shared type contract between firmware and host.

| Step | File(s) | Details |
|------|---------|---------|
| 1.1 | `rmk-types/Cargo.toml` | Add `postcard-rpc` and `postcard` with `experimental-derive` (for `Schema`) as dependencies |
| 1.2 | `rmk-types/src/action.rs` | Add `#[derive(Schema)]` to `KeyAction`, `Action`, `KeyCode`, `EncoderAction`, `MorseProfile` |
| 1.3 | `rmk-types/src/protocol/rmk.rs` | Define all ICD types: `ProtocolVersion`, `DeviceCapabilities`, `RmkError`, `LockStatus`, `UnlockChallenge`, `KeyPosition`, `BulkRequest`, `StorageResetMode`, payload types for Topics |
| 1.4 | `rmk-types/src/protocol/rmk.rs` | Define all `endpoints!()` and `topics!()` declarations (see Appendix A) |
| 1.5 | `rmk-types/src/protocol/mod.rs` | Add `pub mod rmk;` |
| 1.6 | Unit tests | Serialization/deserialization round-trip tests for all ICD types; key hash collision detection across all endpoints |
| 1.7 | `rmk-types/src/connection.rs`, `rmk-types/src/protocol/rmk.rs`, `rmk/src/event/connection.rs`, `rmk/src/event/state.rs`, `rmk/src/event/battery.rs`, `rmk/src/state.rs`, `rmk/src/ble/mod.rs` | Consolidate shared types and add event↔payload conversions: (a) Move `ConnectionType` to `rmk-types/src/connection.rs` as the single definition, remove duplicates from `rmk/src/event/connection.rs` and `rmk/src/state.rs`, update imports in `rmk/src/ble/mod.rs`; (b) Add `From` conversions between internal events and protocol topic payload types (e.g. `LayerChangeEvent` ↔ `LayerChangePayload`, `ConnectionChangeEvent` ↔ `ConnectionPayload`, `BatteryStatusEvent` → `BatteryStatus`). Events keep their current structure — wrappers use `rmk-types` types as fields where applicable (already the case for `LedIndicator`, `ModifierCombination`) |

### Phase 2: Feature Gate and ProtocolService Skeleton

**Goal**: Establish the new protocol's code structure alongside Vial.

> **Design decision**: `ProtocolService` implements its own dispatch loop rather than using postcard-rpc's `define_dispatch!` macro + `Server` struct. The reason is that `ProtocolService` is generic over const parameters (`ROW`, `COL`, `NUM_LAYER`, `NUM_ENCODER`) and holds `&RefCell<KeyMap<...>>` — `define_dispatch!` requires static, non-generic context types. RMK reuses postcard-rpc's `endpoints!`/`topics!` definitions, wire format, key hashing, and serialization, but handles dispatch manually.

| Step | File(s) | Details |
|------|---------|---------|
| 2.1 | `rmk/Cargo.toml` | Add `rmk_protocol = ["host", "dep:postcard-rpc"]` feature; add `postcard-rpc` as optional dependency |
| 2.2 | `rmk/src/host/protocol/mod.rs` | Create `ProtocolService` struct: holds `&RefCell<KeyMap>`, lock state, RX/TX buffers |
| 2.3 | `rmk/src/host/protocol/mod.rs` | Implement dispatch loop: read COBS frame -> decode key -> match handler -> encode response -> write COBS frame |
| 2.4 | `rmk/src/host/mod.rs` | Add `#[cfg(feature = "rmk_protocol")]` version of `run_host_communicate_task()` that creates and runs `ProtocolService` as async future (composed into `select_biased!`, not spawned) |
| 2.5 | `rmk/src/storage/mod.rs` | Rename `FlashOperationMessage::VialMessage` to `FlashOperationMessage::HostMessage` |

### Phase 3: USB Raw Bulk Transport

**Goal**: Get the first working transport for desktop testing.

| Step | File(s) | Details |
|------|---------|---------|
| 3.1 | `rmk/src/host/protocol/transport.rs` | Implement raw vendor-class bulk endpoint transport: create vendor interface (class `0xFF`) with bulk IN/OUT endpoints using `embassy_usb::Builder`. Add MS OS descriptors for WinUSB auto-binding on Windows |
| 3.2 | `rmk/src/host/protocol/transport.rs` | Implement postcard-rpc's `WireTx` and `WireRx` traits for the USB bulk transport |
| 3.3 | `rmk/src/host/protocol/mod.rs` | Make `ProtocolService` generic over `WireTx + WireRx` traits |
| 3.4 | `rmk/src/usb/mod.rs` | Add vendor-class bulk endpoint creation alongside existing HID setup (feature-gated on `rmk_protocol`). Ensure composite device coexistence with HID via IAD |
| 3.5 | Integration test | Connect via USB, complete handshake (GetVersion + GetCapabilities). Host tool uses `nusb` crate to claim vendor interface |

### Phase 4: System and Keymap Endpoints

**Goal**: Core configuration functionality working end-to-end.

| Step | File(s) | Details |
|------|---------|---------|
| 4.1 | `rmk/src/host/protocol/mod.rs` | Implement handlers: `GetVersion`, `GetCapabilities`, `GetLockStatus` |
| 4.2 | `rmk/src/host/protocol/mod.rs` | Implement handlers: `GetKeyAction`, `SetKeyAction` (with write-through to `FLASH_CHANNEL`) |
| 4.3 | `rmk/src/host/protocol/mod.rs` | Implement handlers: `GetKeymapBulk`, `SetKeymapBulk`, `GetLayerCount`, `GetDefaultLayer`, `SetDefaultLayer`, `ResetKeymap` |
| 4.4 | `rmk/src/host/protocol/mod.rs` | Implement handlers: `Reboot`, `BootloaderJump`, `StorageReset` |
| 4.5 | Host CLI tool | Minimal Rust CLI using `postcard-rpc` client with `nusb` backend: connect to vendor-class USB interface, handshake, read/write keymap, display capabilities |

### Phase 5: Security (Lock/Unlock)

**Goal**: Protect write operations with physical key unlock.

| Step | File(s) | Details |
|------|---------|---------|
| 5.1 | `rmk/src/host/lock.rs` | Extract lock logic from `rmk/src/host/via/vial_lock.rs` into a shared, protocol-neutral module |
| 5.2 | `rmk/src/host/protocol/mod.rs` | Implement `Unlock`, `Lock` endpoint handlers; integrate lock state into dispatch loop (check permission before executing write handlers) |
| 5.3 | `rmk/src/host/protocol/mod.rs` | Implement auto-timeout: re-lock after 90s of no write operations |

### Phase 6: Remaining Endpoints

**Goal**: Full v1 endpoint coverage.

| Step | File(s) | Details |
|------|---------|---------|
| 6.1 | `rmk/src/host/protocol/mod.rs` | Encoder endpoints: `GetEncoderAction`, `SetEncoderAction` |
| 6.2 | `rmk/src/host/protocol/mod.rs` | Macro endpoints: `GetMacroInfo`, `GetMacro`, `SetMacro`, `ResetMacros` |
| 6.3 | `rmk/src/host/protocol/mod.rs` | Combo endpoints: `GetCombo`, `SetCombo`, `ResetCombos` |
| 6.4 | `rmk/src/host/protocol/mod.rs` | Morse/Tap-Dance endpoints: `GetMorse`, `SetMorse`, `ResetMorse` |
| 6.5 | `rmk/src/host/protocol/mod.rs` | Fork endpoints: `GetFork`, `SetFork`, `ResetForks` |
| 6.6 | `rmk/src/host/protocol/mod.rs` | Behavior endpoints: `GetBehaviorConfig`, `SetBehaviorConfig` |
| 6.7 | `rmk/src/host/protocol/mod.rs` | Connection endpoints: `GetConnectionInfo`, `SetConnectionType`, `SwitchBleProfile`, `ClearBleProfile` |
| 6.8 | `rmk/src/host/protocol/mod.rs` | Status endpoints: `GetBatteryStatus`, `GetCurrentLayer`, `GetMatrixState`, `GetSplitStatus` |

### Phase 7: Topics (Notifications)

**Goal**: Device-to-host event streaming via single-writer architecture.

| Step | File(s) | Details |
|------|---------|---------|
| 7.1 | `rmk/src/host/protocol/topics.rs` | Create event bridging module: conversion functions from internal events to Topic payload structs, Topic frame encoding |
| 7.2 | `rmk/src/host/protocol/mod.rs` | Integrate event subscribers into `ProtocolService`: create all subscribers in `new()`, add to `select` loop in `run()`. Response writes prioritized over topic writes in select order |
| 7.3 | Host CLI tool | Add Topic listener: display battery state, connection changes, layer changes in real-time |

### Phase 8: BLE Serial Transport

**Goal**: Protocol works over BLE.

| Step | File(s) | Details |
|------|---------|---------|
| 8.1 | `rmk/src/ble/host_service/protocol.rs` | Implement NUS-like GATT service with RX/TX characteristics |
| 8.2 | `rmk/src/ble/host_service/protocol.rs` | Implement `embedded_io_async::Read` + `Write` for BLE serial wrapper, then wrap with COBS framing to implement postcard-rpc's `WireTx`/`WireRx` traits — `ProtocolService` works unchanged |
| 8.3 | Integration test | Connect via BLE on nRF52840, complete handshake and keymap read/write |

### Phase 9: Host Tool and Migration

**Goal**: End-user-facing tooling and Vial deprecation.

| Step | Details |
|------|---------|
| 9.1 | Build web-based configurator (Tauri or Rust -> WASM + WebUSB). Raw vendor-class bulk endpoints are natively WebUSB-compatible |
| 9.2 | Deprecate Vial in documentation; mark `vial` feature as legacy |
| 9.3 | Optional: HID transport fallback for environments without WebUSB support (uses 32-byte HID reports with COBS fragmentation) |
| 9.4 | After adoption window: remove `vial` feature gate |

---

## 16. Test Plan and Acceptance Criteria

### Unit Tests
1. Serialization/deserialization round-trip for all ICD types.
2. Key hash collision detection across all defined endpoints.
3. Error variant coverage.

### Transport Tests
1. USB raw bulk lifecycle: connect, handshake, operations, disconnect.
2. BLE serial lifecycle.
3. Reconnect after transport drop — re-handshake succeeds.

### Protocol Robustness Tests
1. Corrupted COBS frame rejection and resync.
2. Oversized payload handling (exceeds firmware RX buffer).
3. Unknown key returns `WireError::UnknownKey` without crash.
4. Malformed postcard payload returns deserialization error.

### Security Tests
1. Write while locked returns `BadState`.
2. Physical unlock flow completes.
3. Auto-timeout re-locks after configured period.
4. Dangerous operations require unlock.

### Persistence Tests
1. Write-through persists correctly; survives reboot.
2. Power-loss during write does not corrupt storage (sequential-storage guarantees).

### Version Compatibility Tests
1. Newer host + older firmware — graceful degradation via `UnknownKey`.
2. Older host + newer firmware — unknown struct fields ignored.
3. Major version mismatch — hard stop, no silent misbehavior.

### Performance Acceptance Criteria
1. Bulk keymap transfer throughput exceeds current Vial baseline.
2. Single key get/set round-trip latency under 10ms on USB.
3. Protocol feature adds no more than ~8KB to firmware binary vs Vial.

---

## 17. References

- [ZMK Studio RPC Protocol](https://zmk.dev/docs/development/studio-rpc-protocol)
- [ZMK Studio Feature Docs](https://zmk.dev/docs/features/studio)
- [QMK XAP Specification](https://hackmd.io/@tzarc/ryY5liviO)
- [QMK XAP Client](https://github.com/qmk/qmk_xap)
- [QMK XAP Scoping Issue](https://github.com/qmk/qmk_firmware/issues/11567)
- [Ergot Messaging Library](https://github.com/jamesmunns/ergot)
- [postcard-rpc GitHub](https://github.com/jamesmunns/postcard-rpc)
- [postcard-rpc Book](https://onevariable.com/postcard-rpc-book/comms-postcard-rpc.html)
- [postcard-rpc Overview](https://github.com/jamesmunns/postcard-rpc/blob/main/docs/overview.md)
- [James Munns: Thoughts on Network Protocols](https://onevariable.com/blog/thoughts-netstack/)
- [COBS Encoding (Wikipedia)](https://en.wikipedia.org/wiki/Consistent_Overhead_Byte_Stuffing)

---

## Appendix A: Rust Type Definitions (ICD)

```rust
use postcard_rpc::{endpoints, topics, TopicDirection};
use serde::{Serialize, Deserialize};
use postcard_schema::Schema;
use heapless::Vec;
use crate::led_indicator::LedIndicator;
use crate::modifier::ModifierCombination;
use crate::mouse_button::MouseButtons;

// === Version & Capabilities ===

#[derive(Serialize, Deserialize, Schema)]
pub struct ProtocolVersion {
    pub major: u8,
    pub minor: u8,
}

#[derive(Serialize, Deserialize, Schema)]
pub struct DeviceCapabilities {
    pub num_layers: u8,
    pub num_rows: u8,
    pub num_cols: u8,
    pub num_encoders: u8,
    pub max_combos: u8,
    pub max_macros: u8,
    pub macro_space_size: u16,
    pub max_morse: u8,
    pub max_forks: u8,
    pub has_storage: bool,
    pub has_split: bool,
    pub num_split_peripherals: u8,
    pub has_ble: bool,
    pub num_ble_profiles: u8,
    pub has_lighting: bool,
    pub max_payload_size: u16,
}

// === Error ===

#[derive(Serialize, Deserialize, Schema)]
pub enum RmkError {
    InvalidParameter,
    BadState,
    Busy,
    StorageError,
    InternalError,
}

pub type RmkResult = Result<(), RmkError>;

// === Security ===

pub const MAX_UNLOCK_KEYS: usize = 2;

#[derive(Serialize, Deserialize, Schema)]
pub struct LockStatus {
    pub locked: bool,
    pub awaiting_keys: bool,
    pub remaining_keys: u8,
}

#[derive(Serialize, Deserialize, Schema)]
pub struct UnlockChallenge {
    pub key_positions: Vec<(u8, u8), MAX_UNLOCK_KEYS>, // (row, col) of keys to press
}

// === Keymap ===

/// Maximum number of key actions in a single bulk request.
/// Bounded by max_payload_size; host should check DeviceCapabilities.
pub const MAX_BULK: usize = 32;

#[derive(Serialize, Deserialize, Schema)]
pub struct KeyPosition {
    pub layer: u8,
    pub row: u8,
    pub col: u8,
}

#[derive(Serialize, Deserialize, Schema)]
pub struct BulkRequest {
    pub layer: u8,
    pub start_row: u8,
    pub start_col: u8,
    pub count: u16,
}

#[derive(Serialize, Deserialize, Schema)]
pub struct SetKeyRequest {
    pub position: KeyPosition,
    pub action: KeyAction,
}

#[derive(Serialize, Deserialize, Schema)]
pub struct SetKeymapBulkRequest {
    pub request: BulkRequest,
    pub actions: Vec<KeyAction, MAX_BULK>,
}

#[derive(Serialize, Deserialize, Schema)]
pub struct GetEncoderRequest {
    pub encoder_id: u8,
    pub layer: u8,
}

#[derive(Serialize, Deserialize, Schema)]
pub struct SetEncoderRequest {
    pub encoder_id: u8,
    pub layer: u8,
    pub action: EncoderAction,
}

#[derive(Serialize, Deserialize, Schema)]
pub struct SetMacroRequest {
    pub index: u8,
    pub data: MacroData,
}

#[derive(Serialize, Deserialize, Schema)]
pub struct SetComboRequest {
    pub index: u8,
    pub config: ComboConfig,
}

#[derive(Serialize, Deserialize, Schema)]
pub struct SetMorseRequest {
    pub index: u8,
    pub config: MorseConfig,
}

#[derive(Serialize, Deserialize, Schema)]
pub struct ForkStateBits {
    pub modifiers: ModifierCombination,
    pub leds: LedIndicator,
    pub mouse: MouseButtons,
}

#[derive(Serialize, Deserialize, Schema)]
pub struct ForkConfig {
    pub trigger: KeyAction,
    pub negative_output: KeyAction,
    pub positive_output: KeyAction,
    pub match_any: ForkStateBits,
    pub match_none: ForkStateBits,
    pub kept_modifiers: ModifierCombination,
    pub bindable: bool,
}

#[derive(Serialize, Deserialize, Schema)]
pub struct SetForkRequest {
    pub index: u8,
    pub config: ForkConfig,
}

// === Storage ===

#[derive(Serialize, Deserialize, Schema)]
pub enum StorageResetMode {
    Full,       // erase all stored data
    LayoutOnly, // erase only keymap/layout data
}

// === Endpoint Declarations ===
endpoints! {
    list = ENDPOINT_LIST;
    | EndpointTy          | RequestTy              | ResponseTy                    | Path                          |
    | ----------          | ---------              | ----------                    | ----                          |
    // System
    | GetVersion          | ()                     | ProtocolVersion               | "sys/version"                 |
    | GetCapabilities     | ()                     | DeviceCapabilities            | "sys/caps"                    |
    | GetLockStatus       | ()                     | LockStatus                    | "sys/lock_status"             |
    | UnlockRequest       | ()                     | UnlockChallenge               | "sys/unlock"                  |
    | LockRequest         | ()                     | ()                            | "sys/lock"                    |
    | Reboot              | ()                     | ()                            | "sys/reboot"                  |
    | BootloaderJump      | ()                     | ()                            | "sys/bootloader"              |
    | StorageReset        | StorageResetMode       | ()                            | "sys/storage_reset"           |
    // Keymap
    | GetKeyAction        | KeyPosition            | KeyAction                     | "keymap/get"                  |
    | SetKeyAction        | SetKeyRequest          | RmkResult                     | "keymap/set"                  |
    | GetKeymapBulk       | BulkRequest            | Vec<KeyAction, MAX_BULK>      | "keymap/bulk_get"             |
    | SetKeymapBulk       | SetKeymapBulkRequest   | RmkResult                     | "keymap/bulk_set"             |
    | GetLayerCount       | ()                     | u8                            | "keymap/layer_count"          |
    | GetDefaultLayer     | ()                     | u8                            | "keymap/default_layer"        |
    | SetDefaultLayer     | u8                     | RmkResult                     | "keymap/set_default_layer"    |
    | ResetKeymap         | ()                     | RmkResult                     | "keymap/reset"                |
    // Encoder
    | GetEncoderAction    | GetEncoderRequest      | EncoderAction                 | "encoder/get"                 |
    | SetEncoderAction    | SetEncoderRequest      | RmkResult                     | "encoder/set"                 |
    // Macro
    | GetMacroInfo        | ()                     | MacroInfo                     | "macro/info"                  |
    | GetMacro            | u8                     | MacroData                     | "macro/get"                   |
    | SetMacro            | SetMacroRequest        | RmkResult                     | "macro/set"                   |
    | ResetMacros         | ()                     | RmkResult                     | "macro/reset"                 |
    // Combo
    | GetCombo            | u8                     | ComboConfig                   | "combo/get"                   |
    | SetCombo            | SetComboRequest        | RmkResult                     | "combo/set"                   |
    | ResetCombos         | ()                     | RmkResult                     | "combo/reset"                 |
    // Morse / Tap-Dance
    | GetMorse            | u8                     | MorseConfig                   | "morse/get"                   |
    | SetMorse            | SetMorseRequest        | RmkResult                     | "morse/set"                   |
    | ResetMorse          | ()                     | RmkResult                     | "morse/reset"                 |
    // Fork
    | GetFork             | u8                     | ForkConfig                    | "fork/get"                    |
    | SetFork             | SetForkRequest         | RmkResult                     | "fork/set"                    |
    | ResetForks          | ()                     | RmkResult                     | "fork/reset"                  |
    // Behavior
    | GetBehaviorConfig   | ()                     | BehaviorConfig                | "behavior/get"                |
    | SetBehaviorConfig   | BehaviorConfig         | RmkResult                     | "behavior/set"                |
    // Connection
    | GetConnectionInfo   | ()                     | ConnectionInfo                | "conn/info"                   |
    | SetConnectionType   | ConnectionType         | RmkResult                     | "conn/set_type"               |
    | SwitchBleProfile    | u8                     | RmkResult                     | "conn/switch_ble"             |
    | ClearBleProfile     | u8                     | RmkResult                     | "conn/clear_ble"              |
    // Status
    | GetBatteryStatus    | ()                     | BatteryStatusEvent             | "status/battery"              |
    | GetCurrentLayer     | ()                     | u8                            | "status/layer"                |
    | GetMatrixState      | ()                     | MatrixState                   | "status/matrix"               |
    | GetSplitStatus      | ()                     | SplitStatus                   | "status/split"                |
}

// === Topic Declarations (Device -> Host Events) ===
topics! {
    list = TOPICS_OUT_LIST;
    direction = TopicDirection::ToClient;
    | TopicTy               | MessageTy              | Path                  |
    | -------               | ---------              | ----                  |
    | LayerChangeTopic      | LayerChangePayload     | "event/layer"         |
    | WpmUpdateTopic        | WpmPayload             | "event/wpm"           |
    | BatteryStatusTopic     | BatteryStatusEvent      | "event/battery"       |
    | BleStateChangeTopic   | BleStatePayload        | "event/ble_state"     |
    | BleProfileChangeTopic | BleProfilePayload      | "event/ble_profile"   |
    | ConnectionChangeTopic | ConnectionPayload      | "event/connection"    |
    | SleepStateTopic       | SleepPayload           | "event/sleep"         |
    | LedIndicatorTopic     | LedPayload             | "event/led"           |
}
```

---

## Appendix B: Endpoint Permission Matrix

| Path | Permission |
|------|------------|
| `sys/version` | ReadOnly |
| `sys/caps` | ReadOnly |
| `sys/lock_status` | ReadOnly |
| `sys/unlock` | ReadOnly |
| `sys/lock` | ReadOnly |
| `sys/reboot` | Dangerous |
| `sys/bootloader` | Dangerous |
| `sys/storage_reset` | Dangerous |
| `keymap/get` | ReadOnly |
| `keymap/set` | RequiresUnlock |
| `keymap/bulk_get` | ReadOnly |
| `keymap/bulk_set` | RequiresUnlock |
| `keymap/layer_count` | ReadOnly |
| `keymap/default_layer` | ReadOnly |
| `keymap/set_default_layer` | RequiresUnlock |
| `keymap/reset` | Dangerous |
| `encoder/get` | ReadOnly |
| `encoder/set` | RequiresUnlock |
| `macro/info` | ReadOnly |
| `macro/get` | ReadOnly |
| `macro/set` | RequiresUnlock |
| `macro/reset` | Dangerous |
| `combo/get` | ReadOnly |
| `combo/set` | RequiresUnlock |
| `combo/reset` | Dangerous |
| `morse/get` | ReadOnly |
| `morse/set` | RequiresUnlock |
| `morse/reset` | Dangerous |
| `fork/get` | ReadOnly |
| `fork/set` | RequiresUnlock |
| `fork/reset` | Dangerous |
| `behavior/get` | ReadOnly |
| `behavior/set` | RequiresUnlock |
| `conn/info` | ReadOnly |
| `conn/set_type` | RequiresUnlock |
| `conn/switch_ble` | RequiresUnlock |
| `conn/clear_ble` | Dangerous |
| `status/battery` | ReadOnly |
| `status/layer` | ReadOnly |
| `status/matrix` | ReadOnly |
| `status/split` | ReadOnly |
