# RMK Protocol Implementation Roadmap

**Tracking document for the RMK Communication Protocol (final_plan.md)**
**Created**: 2026-03-02
**Status**: In Progress

---

## Phase 1: ICD Types and postcard-rpc Integration

**Goal**: Define the shared type contract between firmware and host.

### Step 1.1 — Add `postcard-rpc` dependency to `rmk-types`

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Check `postcard-rpc` latest version and `no_std` compatibility (disable `use-std` feature) | [x] | v0.12.1, no_std by default |
| b | Add `postcard-rpc = { version = "...", default-features = false }` to `rmk-types/Cargo.toml` `[dependencies]` | [x] | v0.12, also added `postcard-schema` v0.2 with `derive` feature |
| c | Confirm `postcard` dependency has `experimental-derive` feature enabled (for `Schema` derive macro) | [x] | Already present. Note: `Schema` derive comes from `postcard-schema` crate, not `postcard::experimental::schema` |
| d | Run `cargo check -p rmk-types` to verify compilation | [x] | |

### Step 1.2 — Add `#[derive(Schema)]` to existing types

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Add `use postcard::experimental::schema::Schema;` at the top of `rmk-types/src/action.rs` | [x] | Used `postcard_schema::Schema` (inline derive path) instead — `postcard::experimental::schema` no longer exists in postcard 1.x |
| b | Add `#[derive(Schema)]` to `KeyAction` enum | [x] | |
| c | Add `#[derive(Schema)]` to `Action` enum | [x] | |
| d | Add `#[derive(Schema)]` to `EncoderAction` struct | [x] | |
| e | Add `#[derive(Schema)]` to `MorseProfile` struct | [x] | |
| f | Check all types referenced by `Action` variants and add `Schema` as needed: `ModifierCombination`, `LightAction`, `KeyboardAction`, `SpecialKey`, etc. | [x] | Added to: ModifierCombination, LightAction, KeyboardAction, LedIndicator, MouseButtons |
| g | Add `#[derive(Schema)]` to `KeyCode` (including `HidKeyCode`, `ConsumerKey`, `SystemControlKey`) in `rmk-types/src/keycode.rs` | [x] | Also added to SpecialKey |
| h | Run `cargo check -p rmk-types` to confirm all Schema derives compile | [x] | Also verified rmk crate and tests pass |

### Step 1.3 — Define ICD types in `rmk-types/src/protocol/rmk.rs`

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Create file `rmk-types/src/protocol/rmk.rs` | [x] | |
| b | Add required imports: `serde`, `postcard_schema::Schema`, `heapless::Vec`, `postcard_rpc::{endpoint, topic}` | [x] | |
| c | Define `ProtocolVersion { major: u8, minor: u8 }` with `Serialize, Deserialize, Schema` | [x] | |
| d | Define `DeviceCapabilities` struct with all fields (`num_layers`, `num_rows`, `num_cols`, `num_encoders`, `max_combos`, `max_macros`, `macro_space_size`, `max_morse`, `max_forks`, `has_storage`, `has_split`, `num_split_peripherals`, `has_ble`, `num_ble_profiles`, `has_lighting`, `max_payload_size`) | [x] | |
| e | Define `RmkError` enum (`InvalidParameter`, `BadState`, `Busy`, `StorageError`, `InternalError`) and `pub type RmkResult = Result<(), RmkError>` | [x] | |
| f | Define `LockStatus { locked: bool, awaiting_keys: bool, remaining_keys: u8 }` | [x] | |
| g | Define `UnlockChallenge { key_positions: Vec<(u8, u8), MAX_UNLOCK_KEYS> }` with `MAX_UNLOCK_KEYS = 2` | [x] | |
| h | Define `KeyPosition { layer: u8, row: u8, col: u8 }` | [x] | |
| i | Define `BulkRequest { layer: u8, start_row: u8, start_col: u8, count: u16 }` with `MAX_BULK = 32` | [x] | |
| j | Define `StorageResetMode` enum (`Full`, `LayoutOnly`) | [x] | |
| k | Define Topic payload types: `LayerChangePayload`, `WpmPayload`, `BatteryStateEvent` (reuse internal enum), `BleStatePayload`, `BleProfilePayload`, `ConnectionPayload`, `SleepPayload`, `LedPayload` | [x] | See final_plan.md Section 8. `BatteryStateEvent` enum matches internal event for full fidelity |
| l | Define connection/status types: `ConnectionInfo`, `ConnectionType`, `BatteryStateEvent`, `MatrixState`, `SplitStatus` | [x] | `BatteryStateEvent` reuses internal enum: `NotAvailable`, `Normal(u8)`, `Charging`, `Charged` |
| m | Define macro types: `MacroInfo`, `MacroData` | [x] | |
| n | Define combo/morse/fork config types: `ComboConfig`, `MorseConfig`, `ForkConfig` (or reuse existing types from rmk-types) | [x] | Protocol-facing types defined; `ForkConfig` uses full-fidelity state bits (modifiers + LED + mouse), no downgrade |
| o | Define protocol-facing `BehaviorConfig` (or directly reuse existing `BehaviorConfig` from `rmk` crate) | [x] | Protocol-facing version with combo_timeout_ms, oneshot_timeout_ms, tap_interval_ms, tap_tolerance |

### Step 1.4 — Define `endpoint!()` and `topic!()` declarations

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Define System endpoints: `GetVersion("sys/version")`, `GetCapabilities("sys/caps")`, `GetLockStatus("sys/lock_status")`, `UnlockRequest("sys/unlock")`, `LockRequest("sys/lock")`, `Reboot("sys/reboot")`, `BootloaderJump("sys/bootloader")`, `StorageReset("sys/storage_reset")` | [x] | 8 endpoints |
| b | Define Keymap endpoints: `GetKeyAction("keymap/get")`, `SetKeyAction("keymap/set")`, `GetKeymapBulk("keymap/bulk_get")`, `SetKeymapBulk("keymap/bulk_set")`, `GetLayerCount("keymap/layer_count")`, `GetDefaultLayer("keymap/default_layer")`, `SetDefaultLayer("keymap/set_default_layer")`, `ResetKeymap("keymap/reset")` | [x] | 8 endpoints |
| c | Define Encoder endpoints: `GetEncoderAction("encoder/get")`, `SetEncoderAction("encoder/set")` | [x] | 2 endpoints |
| d | Define Macro endpoints: `GetMacroInfo("macro/info")`, `GetMacro("macro/get")`, `SetMacro("macro/set")`, `ResetMacros("macro/reset")` | [x] | 4 endpoints |
| e | Define Combo endpoints: `GetCombo("combo/get")`, `SetCombo("combo/set")`, `ResetCombos("combo/reset")` | [x] | 3 endpoints |
| f | Define Morse endpoints: `GetMorse("morse/get")`, `SetMorse("morse/set")`, `ResetMorse("morse/reset")` | [x] | 3 endpoints |
| g | Define Fork endpoints: `GetFork("fork/get")`, `SetFork("fork/set")`, `ResetForks("fork/reset")` | [x] | 3 endpoints |
| h | Define Behavior endpoints: `GetBehaviorConfig("behavior/get")`, `SetBehaviorConfig("behavior/set")` | [x] | 2 endpoints |
| i | Define Connection endpoints: `GetConnectionInfo("conn/info")`, `SetConnectionType("conn/set_type")`, `SwitchBleProfile("conn/switch_ble")`, `ClearBleProfile("conn/clear_ble")` | [x] | 4 endpoints |
| j | Define Status endpoints: `GetBatteryStatus("status/battery")`, `GetCurrentLayer("status/layer")`, `GetMatrixState("status/matrix")`, `GetSplitStatus("status/split")` | [x] | 4 endpoints |
| k | Define all 8 topic declarations: `LayerChangeTopic`, `WpmUpdateTopic`, `BatteryStateTopic`, `BleStateChangeTopic`, `BleProfileChangeTopic`, `ConnectionChangeTopic`, `SleepStateTopic`, `LedIndicatorTopic` | [x] | |

### Step 1.5 — Module wiring

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Add `pub mod rmk;` to `rmk-types/src/protocol/mod.rs` | [x] | |
| b | Decide whether to re-export key types from `protocol::rmk` in `rmk-types/src/lib.rs` | [x] | No re-exports needed; `rmk_types::protocol::rmk::*` is clean enough |
| c | Run `cargo check -p rmk-types` to confirm module connections are correct | [x] | |

### Step 1.6 — Unit tests

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Create `#[cfg(test)] mod tests` at the bottom of `rmk-types/src/protocol/rmk.rs` or a separate test file | [x] | Inline in rmk.rs |
| b | Write serde round-trip tests: for each ICD struct/enum, use `postcard::to_slice` -> `postcard::from_bytes` and assert equality | [x] | 22 round-trip tests |
| c | Write key hash collision detection test: collect all endpoint/topic key hashes, assert no duplicates | [x] | 3 collision tests (endpoints, topics, cross) |
| d | Test edge cases: empty `heapless::Vec`, max-value fields, all-zero `DeviceCapabilities` | [x] | |
| e | Run `cargo test -p rmk-types` to confirm all tests pass | [x] | 28/28 tests pass |

---

## Phase 2: Feature Gate and ProtocolService Skeleton

**Goal**: Establish the new protocol's code structure alongside Vial.

> **Design decision**: `ProtocolService` implements its own dispatch loop rather than using postcard-rpc's `define_dispatch!` macro + `Server` struct. The reason is that `ProtocolService` is generic over const parameters (`ROW`, `COL`, `NUM_LAYER`, `NUM_ENCODER`) and holds `&RefCell<KeyMap<...>>` — `define_dispatch!` requires static, non-generic context types. RMK reuses postcard-rpc's `endpoint!`/`topic!` definitions, wire format, key hashing, `WireTx`/`WireRx` traits, and serialization.

### Step 2.1 — Add `rmk_protocol` feature to `rmk/Cargo.toml`

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Add `postcard-rpc = { version = "...", default-features = false, optional = true }` to `rmk/Cargo.toml` `[dependencies]` | [ ] | |
| b | Add `rmk_protocol = ["host", "dep:postcard-rpc"]` to `[features]` | [ ] | |
| c | Ensure `rmk_protocol` and `vial` are mutually exclusive (via `#[cfg]` in code, not at Cargo level) | [ ] | |
| d | Run `cargo check -p rmk --no-default-features --features=rmk_protocol` to verify feature compiles | [ ] | |

### Step 2.2 — Create `ProtocolService` struct

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Create directory `rmk/src/host/protocol/` | [ ] | |
| b | Create `rmk/src/host/protocol/mod.rs` | [ ] | |
| c | Define `ProtocolService` struct with fields: `&'a RefCell<KeyMap<ROW, COL, NUM_LAYER, NUM_ENCODER>>`, lock state (`bool`), RX buffer `[u8; BUF_SIZE]`, TX buffer `[u8; BUF_SIZE]`, transport (generic `T: WireTx + WireRx`), event subscribers (created once in `new()`) | [ ] | Follow VialService pattern. Event subscribers must be held as fields, not re-created per loop iteration |
| d | Implement `ProtocolService::new()` constructor — create all event subscribers here | [ ] | |
| e | Implement `ProtocolService::run()` async main loop: use `embassy_futures::select` to multiplex transport reads (endpoint requests) and event subscribers (topic sources). Single-writer: only this loop writes to transport | [ ] | Returns a future composed into `select_biased!`, NOT spawned via Spawner |
| f | Gate entire module with `#[cfg(feature = "rmk_protocol")]` | [ ] | |

### Step 2.3 — Implement dispatch loop

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | In the transport branch of `select`, read frame from `WireRx` | [ ] | `WireRx` handles framing (COBS for serial, packet for USB bulk) |
| b | Parse `VarHeader` from frame: extract discriminant, key, seq_no | [ ] | Use postcard-rpc's `VarHeader::read_from_wire()` |
| c | Match key against registered endpoint keys (compile-time key constants from `endpoint!` macro) | [ ] | Match on `<GetVersion as Endpoint>::REQ_KEY`, etc. |
| d | On match: deserialize payload with `postcard::from_bytes`, call corresponding handler | [ ] | |
| e | On no match: construct `WireError::UnknownKey` error response | [ ] | |
| f | Serialize handler return value and write response frame via `WireTx` | [ ] | Include echoed seq_no for request/response correlation |
| g | In event subscriber branches of `select`, serialize Topic frame and write via `WireTx` | [ ] | Response writes prioritized over topic writes in select order |
| h | Add error handling: on frame parse failure, skip current frame and continue (resync) | [ ] | |
| i | Handle transport write failures: log error, do not crash, continue running | [ ] | |

### Step 2.4 — Feature-gated task wiring

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Add `#[cfg(feature = "rmk_protocol")] pub(crate) mod protocol;` in `rmk/src/host/mod.rs` | [ ] | Parallel to existing `#[cfg(feature = "vial")] pub(crate) mod via;` |
| b | Add `#[cfg(feature = "rmk_protocol")]` version of `run_host_communicate_task()` that creates and runs `ProtocolService` | [ ] | Returns async future, composed into `select_biased!` in `run_keyboard()` |
| c | Ensure `ProtocolService` receives the same `&RefCell<KeyMap>` reference as `VialService` | [ ] | |
| d | Run `cargo check -p rmk --no-default-features --features=rmk_protocol` to verify task wiring compiles | [ ] | |

### Step 2.5 — Rename `VialMessage` -> `HostMessage`

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | In `rmk/src/storage/mod.rs`, rename `FlashOperationMessage::VialMessage` to `FlashOperationMessage::HostMessage` | [ ] | |
| b | Search all references to `VialMessage` and update (`rmk/src/host/via/mod.rs`, `rmk/src/host/via/vial.rs`, etc.) | [ ] | Use `cargo check` to find all |
| c | Keep `#[cfg(feature = "host")]` on the `HostMessage` variant (usable by both vial and rmk_protocol) | [ ] | |
| d | Implement runtime keymap reset in storage: change `FlashOperationMessage::ResetLayout` handler to actually erase stored keymap keys and reload defaults (currently a no-op at runtime) | [ ] | Required for `ResetKeymap` endpoint to work |
| e | Run `cargo test -p rmk --no-default-features --features=vial,storage` to confirm Vial still works | [ ] | |

---

## Phase 3: USB Raw Bulk Transport

**Goal**: Get the first working transport for desktop testing.

### Step 3.1 — Create raw vendor-class bulk endpoint transport

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Create `rmk/src/host/protocol/transport.rs` | [ ] | |
| b | Create vendor interface (class `0xFF`) with bulk IN and bulk OUT endpoints using `embassy_usb::Builder` | [ ] | 1 interface, 2 endpoints — simpler than CDC-ACM |
| c | Add MS OS descriptors for automatic WinUSB driver binding on Windows | [ ] | Required for driverless Windows support |
| d | Implement connect/disconnect handling | [ ] | |
| e | Gate with `#[cfg(feature = "rmk_protocol")]` | [ ] | |

### Step 3.2 — Implement `WireTx`/`WireRx` for USB bulk transport

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Implement postcard-rpc's `WireTx` trait for bulk IN endpoint | [ ] | USB bulk packets are self-delimiting — no COBS needed |
| b | Implement postcard-rpc's `WireRx` trait for bulk OUT endpoint | [ ] | |
| c | Reference postcard-rpc's own `embassy_usb_v0_5` impl for pattern guidance | [ ] | |
| d | Run `cargo check` to confirm generic parameters propagate correctly | [ ] | |

### Step 3.3 — Add vendor bulk endpoints to USB setup

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | In `rmk/src/usb/mod.rs`, under `#[cfg(feature = "rmk_protocol")]`, add vendor-class bulk endpoint creation | [ ] | |
| b | Ensure vendor interface coexists with existing HID composite device (IAD support already enabled in `new_usb_builder`) | [ ] | USB composite |
| c | Pass bulk endpoint handles to `ProtocolService` constructor | [ ] | |
| d | Test USB enumeration in an example project: after plugging in, host should see both HID and vendor-class interfaces | [ ] | |

### Step 3.4 — Integration test: USB handshake

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Build firmware with `rmk_protocol` feature on an nRF52840 or RP2040 example | [ ] | |
| b | Flash firmware, use `nusb` crate (Rust) or `libusb` to claim vendor interface and verify communication | [ ] | |
| c | Send `GetVersion` request using postcard-rpc client with `nusb` backend | [ ] | |
| d | Verify correct `ProtocolVersion` response received | [ ] | |
| e | Send `GetCapabilities` request, verify received `DeviceCapabilities` fields match firmware config | [ ] | |

---

## Phase 4: System and Keymap Endpoints

**Goal**: Core configuration functionality working end-to-end.

### Step 4.1 — System endpoint handlers

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Implement `GetVersion` handler: return hardcoded `ProtocolVersion { major: 1, minor: 0 }` | [ ] | |
| b | Implement `GetCapabilities` handler: construct `DeviceCapabilities` from compile-time constants (`NUM_LAYER`, `NUM_ROW`, `NUM_COL`, etc., emitted by `build.rs`) | [ ] | See `rmk/build.rs` output constants |
| c | Implement `GetLockStatus` handler: read current lock state, return `LockStatus` | [ ] | Initially return always-unlocked |
| d | Register these three handlers in dispatch loop key match | [ ] | |
| e | Test: host sends all three requests, verify correct data returned | [ ] | |

### Step 4.2 — Keymap get/set handlers

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Implement `GetKeyAction` handler: extract `(layer, row, col)` from `KeyPosition`, call `keymap.borrow().get_action_at()`, return `KeyAction` | [ ] | |
| b | Implement `SetKeyAction` handler: receive `SetKeyRequest`, call `keymap.borrow_mut().set_action_at()` to update in-memory state | [ ] | |
| c | In `SetKeyAction` handler, send `FLASH_CHANNEL.send(FlashOperationMessage::HostMessage(KeymapData::KeymapKey(...)))` for flash persistence | [ ] | Follow VialService pattern |
| d | Add parameter validation: return `RmkError::InvalidParameter` when layer/row/col is out of bounds | [ ] | |
| e | Test: read key action at (0,0,0), modify it, read again to verify consistency | [ ] | |

### Step 4.3 — Bulk keymap and layer handlers

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Implement `GetKeymapBulk` handler: batch-read KeyActions per `BulkRequest`, fill `heapless::Vec<KeyAction, MAX_BULK>` | [ ] | Row-major order |
| b | Implement `SetKeymapBulk` handler: receive `SetKeymapBulkRequest`, batch-set KeyActions and send individual `FlashOperationMessage` per key | [ ] | |
| c | Implement `GetLayerCount` handler: return `NUM_LAYER as u8` | [ ] | |
| d | Implement `GetDefaultLayer` handler: call `keymap.borrow().get_default_layer()` | [ ] | |
| e | Implement `SetDefaultLayer` handler: call `keymap.borrow_mut().set_default_layer()` + send `FlashOperationMessage::DefaultLayer` | [ ] | |
| f | Implement `ResetKeymap` handler: send `FlashOperationMessage::ResetLayout` to `FLASH_CHANNEL` | [ ] | |
| g | Test: bulk-read entire layer keymap, bulk-write, re-read to verify consistency | [ ] | |

### Step 4.4 — Device control handlers

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Implement `Reboot` handler: call `cortex_m::peripheral::SCB::sys_reset()` or platform-specific reset function | [ ] | Needs `#[cfg]` per chip |
| b | Implement `BootloaderJump` handler: write bootloader magic value then reset (reference existing `KeyboardAction::Bootloader` impl) | [ ] | |
| c | Implement `StorageReset` handler: based on `StorageResetMode`, send `FlashOperationMessage::Reset` or `FlashOperationMessage::ResetLayout` | [ ] | |
| d | These three operations are `Dangerous` permission level; skip lock check for now (added in Phase 5) | [ ] | |

### Step 4.5 — Minimal host CLI tool

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Create new Cargo binary project `rmk-cli/` (or under `tools/` directory) | [ ] | |
| b | Add dependencies: `postcard-rpc` (client mode), `nusb` (raw USB), `clap` (CLI arg parsing) | [ ] | |
| c | Implement USB connection logic: scan for vendor-class USB interface using `nusb`, claim interface | [ ] | |
| d | Implement `handshake` command: send `GetVersion` + `GetCapabilities`, print results | [ ] | |
| e | Implement `get-key` subcommand: specify layer/row/col, call `GetKeyAction`, print KeyAction | [ ] | |
| f | Implement `set-key` subcommand: specify layer/row/col and KeyAction, call `SetKeyAction` | [ ] | |
| g | Implement `dump-keymap` subcommand: call `GetKeymapBulk` layer by layer, print as table | [ ] | |

---

## Phase 5: Security (Lock/Unlock)

**Goal**: Protect write operations with physical key unlock.

### Step 5.1 — Extract shared lock logic

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Read `rmk/src/host/via/vial_lock.rs`, understand existing `VialLock` struct and state machine | [ ] | |
| b | Create `rmk/src/host/lock.rs`, define protocol-neutral `DeviceLock` struct | [ ] | |
| c | Migrate `VialLock` core logic (key position generation, matrix state checking, state transitions) into `DeviceLock` | [ ] | |
| d | Gate with `#[cfg(feature = "host")]` (usable by both vial and rmk_protocol) | [ ] | |
| e | Refactor `vial_lock.rs` to become a thin wrapper around `DeviceLock`, keeping Vial functionality unchanged | [ ] | |
| f | Run `cargo test` to confirm Vial functionality is not broken | [ ] | |

### Step 5.2 — Unlock/Lock endpoint handlers

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Add `DeviceLock` field to `ProtocolService` | [ ] | |
| b | Implement `UnlockRequest` handler: call `DeviceLock::start_unlock()`, return `UnlockChallenge` (physical key positions to press) | [ ] | |
| c | Implement `LockRequest` handler: call `DeviceLock::lock()`, return `()` | [ ] | |
| d | Add permission checks in dispatch loop: per Appendix B Permission Matrix, check lock state for `RequiresUnlock` and `Dangerous` endpoints | [ ] | |
| e | Return `RmkError::BadState` when write operation attempted while locked | [ ] | |
| f | Test: `SetKeyAction` while locked should return `BadState`; after unlock it should succeed | [ ] | |

### Step 5.3 — Auto-timeout

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Add `last_write_instant: Option<Instant>` field to `DeviceLock` to track last write operation time | [ ] | |
| b | Update `last_write_instant` after each successful write operation | [ ] | |
| c | Add 90-second timer check in `ProtocolService` select loop | [ ] | |
| d | Auto-call `DeviceLock::lock()` when timeout triggers | [ ] | |
| e | Make timeout configurable via `keyboard.toml` `[protocol].lock_timeout` (default 90s) | [ ] | |
| f | Test: unlock, wait for timeout, verify automatic re-lock | [ ] | |

---

## Phase 6: Remaining Endpoints

**Goal**: Full v1 endpoint coverage.

### Step 6.1 — Encoder endpoints

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Implement `GetEncoderAction` handler: receive `GetEncoderRequest`, read `EncoderAction` from `keymap.borrow().encoders` | [ ] | |
| b | Implement `SetEncoderAction` handler: receive `SetEncoderRequest`, update in-memory + send `FlashOperationMessage` | [ ] | |
| c | Add parameter validation: return `RmkError::InvalidParameter` when encoder_id or layer is out of bounds | [ ] | |
| d | Test: read/modify encoder action, verify persistence | [ ] | |

### Step 6.2 — Macro endpoints

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Implement `GetMacroInfo` handler: return `MacroInfo { max_macros, macro_space_size }` | [ ] | |
| b | Implement `GetMacro` handler: receive macro index, read `MacroData` from `BehaviorConfig.macros` | [ ] | |
| c | Implement `SetMacro` handler: receive `SetMacroRequest`, update in-memory + persist | [ ] | |
| d | Implement `ResetMacros` handler: clear all macro definitions + send flash reset message | [ ] | |
| e | Test: complete macro CRUD flow | [ ] | |

### Step 6.3 — Combo endpoints

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Implement `GetCombo` handler: receive combo index, read `ComboConfig` from `BehaviorConfig.combos` | [ ] | |
| b | Implement `SetCombo` handler: receive `SetComboRequest`, update in-memory + persist | [ ] | |
| c | Implement `ResetCombos` handler: clear all combos + flash reset | [ ] | |
| d | Test: combo config read/write and reset | [ ] | |

### Step 6.4 — Morse/Tap-Dance endpoints

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Implement `GetMorse` handler: receive morse index, read `MorseConfig` | [ ] | |
| b | Implement `SetMorse` handler: receive `SetMorseRequest`, update morse config + persist | [ ] | |
| c | Implement `ResetMorse` handler: reset all morse configs | [ ] | |
| d | Test: complete morse config CRUD | [ ] | |

### Step 6.5 — Fork endpoints

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Implement `GetFork` handler: receive fork index, read `ForkConfig` | [ ] | |
| b | Implement `SetFork` handler: receive `SetForkRequest`, update fork config + persist | [ ] | |
| c | Implement `ResetForks` handler: reset all fork configs | [ ] | |
| d | Test: complete fork config CRUD | [ ] | |

### Step 6.6 — Behavior endpoints

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Implement `GetBehaviorConfig` handler: read full `BehaviorConfig` from `keymap.borrow().behavior` (combo_timeout, oneshot_timeout, tap_interval, etc.) | [ ] | |
| b | Implement `SetBehaviorConfig` handler: update behavior config + send per-field `FlashOperationMessage` variants (`ComboTimeout`, `OneShotTimeout`, `TapInterval`, etc.) | [ ] | |
| c | Test: read and modify behavior config | [ ] | |

### Step 6.7 — Connection endpoints

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Implement `GetConnectionInfo` handler: return current connection type (USB/BLE), BLE profile info | [ ] | `#[cfg(feature = "_ble")]` |
| b | Implement `SetConnectionType` handler: switch connection type + send `FlashOperationMessage::ConnectionType` | [ ] | |
| c | Implement `SwitchBleProfile` handler: switch BLE profile + send `FlashOperationMessage::ActiveBleProfile` | [ ] | `#[cfg(feature = "_ble")]` |
| d | Implement `ClearBleProfile` handler: clear specified BLE profile pairing info + send `FlashOperationMessage::ClearSlot` | [ ] | `#[cfg(feature = "_ble")]` |
| e | In non-BLE builds, BLE-related endpoints return `WireError::UnknownKey` | [ ] | |
| f | Test: test in both BLE and non-BLE builds | [ ] | |

### Step 6.8 — Status endpoints

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Implement `GetBatteryStatus` handler: read battery state via `BatteryStateEvent` (enum: `NotAvailable`, `Normal(u8)`, `Charging`, `Charged`) | [ ] | `#[cfg(feature = "_ble")]` |
| b | Implement `GetCurrentLayer` handler: get currently active layer from `keymap.borrow()` | [ ] | |
| c | Implement `GetMatrixState` handler: read matrix key states, return `MatrixState` | [ ] | |
| d | Implement `GetSplitStatus` handler: return split peripheral connection status | [ ] | `#[cfg(feature = "split")]` |
| e | Endpoints without matching feature return `WireError::UnknownKey` | [ ] | |
| f | Test: each status query under matching and non-matching feature configs | [ ] | |

---

## Phase 7: Topics (Notifications)

**Goal**: Device-to-host event streaming via single-writer architecture.

> **Architecture**: Topics are integrated directly into `ProtocolService::run()`'s existing `embassy_futures::select` loop. No separate writer task — the ProtocolService is the single owner of the transport writer. Response writes are prioritized over topic writes in select order to prevent notification bursts from starving request/response traffic.

### Step 7.1 — Event bridging module

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Create `rmk/src/host/protocol/topics.rs` | [ ] | |
| b | Implement conversion functions for each event: internal event → Topic payload struct (e.g., `LayerChangeEvent` → `LayerChangePayload`, `BatteryStateEvent` → `BatteryStateEvent` (direct reuse)) | [ ] | |
| c | Implement `encode_topic_frame()` function: serialize Topic payload into wire frame (header with topic key + seq=0 + postcard payload). Framing (COBS vs raw) is handled by `WireTx` | [ ] | |
| d | Gate BLE-related topics (`BatteryState`, `BleStateChange`, `BleProfileChange`) with `#[cfg]` | [ ] | |

### Step 7.2 — Integrate Topics into ProtocolService

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Verify that event subscribers created in `ProtocolService::new()` (Phase 2.2d) are already held as struct fields | [ ] | CRITICAL: do NOT create subscribers inside the select loop — watch-based subscribers immediately return on `changed()` if newly created |
| b | Add event subscriber branches to the existing `select` loop in `ProtocolService::run()` | [ ] | Already set up in Phase 2.3g; this step adds the actual encoding and writing |
| c | Ensure response branches are listed before topic branches in select for priority | [ ] | Prevents notification bursts from starving request/response |
| d | Handle transport write failures (e.g., disconnected): log error but don't crash, continue running | [ ] | |
| e | Test: simulate layer change event, verify host receives corresponding Topic frame | [ ] | |

### Step 7.3 — Host CLI Topic listener

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Add `listen` subcommand to `rmk-cli` | [ ] | |
| b | Implement Topic frame decoding: identify topic key, deserialize payload | [ ] | |
| c | Implement real-time display: battery level changes, layer switches, BLE connection state changes, etc. | [ ] | |
| d | Support Ctrl+C graceful exit | [ ] | |
| e | Test: switch layers on keyboard, verify CLI displays layer changes in real-time | [ ] | |

---

## Phase 8: BLE Serial Transport

**Goal**: Protocol works over BLE.

### Step 8.1 — NUS-like GATT service

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Create `rmk/src/ble/host_service/protocol.rs` (or under existing `host_service/` directory) | [ ] | |
| b | Define GATT service: RX characteristic (Write/Write Without Response) + TX characteristic (Notify) | [ ] | Use NUS UUID or custom UUID |
| c | Register service using `trouble-host` crate's GATT server API | [ ] | Reference existing BLE HID service |
| d | Implement RX characteristic write handler: write received data to internal buffer | [ ] | |
| e | Implement TX notification: send data via TX characteristic notify | [ ] | |
| f | Handle MTU negotiation: adjust single transfer size based on negotiated MTU | [ ] | |

### Step 8.2 — BLE serial `WireTx`/`WireRx` wrapper

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Create `BleTransport` struct wrapping GATT service RX/TX channels | [ ] | |
| b | Implement `embedded_io_async::Read` for `BleTransport`: read data from RX buffer | [ ] | |
| c | Implement `embedded_io_async::Write` for `BleTransport`: send data via TX characteristic notify | [ ] | |
| d | Wrap with COBS framing layer, then implement postcard-rpc's `WireTx`/`WireRx` traits | [ ] | BLE serial is byte stream — needs COBS (unlike USB bulk) |
| e | Handle BLE connect/disconnect events: `read()` returns EOF on disconnect | [ ] | |
| f | Pass `BleTransport` to `ProtocolService::new()` — ProtocolService works without any modification | [ ] | |

### Step 8.3 — BLE integration test

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Build firmware with `rmk_protocol,_ble` features on nRF52840 BLE example | [ ] | |
| b | Use phone nRF Connect app or PC BLE tool to scan and connect to device | [ ] | |
| c | Send `GetVersion` request through NUS/custom GATT service | [ ] | |
| d | Verify correct `ProtocolVersion` response received | [ ] | |
| e | Complete full handshake + keymap read, verify BLE transport reliability | [ ] | |

---

## Phase 9: Host Tool and Migration

**Goal**: End-user-facing tooling and Vial deprecation.

### Step 9.1 — Web-based configurator

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Technology decision: Tauri (desktop app) vs Rust->WASM + WebUSB (pure web) | [ ] | Raw vendor bulk is natively WebUSB-compatible |
| b | Set up frontend project scaffold (React/Svelte + TypeScript) | [ ] | |
| c | Implement WebUSB connection layer: connect to vendor-class USB interface via browser WebUSB API (raw bulk endpoints are natively compatible) | [ ] | |
| d | Implement protocol client: COBS encode/decode + postcard serialization (WASM version or JS rewrite) | [ ] | |
| e | Implement handshake flow UI: display device info and capabilities | [ ] | |
| f | Implement keymap editor: visual key layout, drag/drop/select to modify KeyAction | [ ] | |
| g | Implement macro/combo/morse/fork editing UI | [ ] | |
| h | Implement behavior settings UI | [ ] | |
| i | Implement connection management UI (BLE profile switch/clear) | [ ] | |
| j | Implement status panel: real-time battery, layer, connection state display (via Topics) | [ ] | |
| k | Implement unlock flow UI: prompt user to press physical keys | [ ] | |

### Step 9.2 — Deprecate Vial

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Add "Migration Guide: Vial -> RMK Protocol" page to RMK documentation | [ ] | |
| b | Add `#[deprecated]` comment to `vial` feature in `rmk/Cargo.toml` | [ ] | |
| c | Update README and Getting Started docs to recommend `rmk_protocol` | [ ] | |
| d | Add `rmk_protocol` version of examples in `examples/use_config/` | [ ] | |

### Step 9.3 — Optional HID transport fallback

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Design HID transport layer: split COBS frames into fixed-size HID reports (1-byte length prefix + payload) | [ ] | |
| b | Implement `HidTransport` wrapper implementing `embedded_io_async::Read + Write` | [ ] | |
| c | Add HID transport option in `rmk/src/usb/mod.rs` (feature-gated) | [ ] | |
| d | Implement WebHID version of protocol client (for environments without WebUSB support) | [ ] | Uses 32-byte HID reports with COBS fragmentation |

### Step 9.4 — Remove Vial feature gate

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Confirm community has sufficiently migrated to `rmk_protocol` (via issue tracker / Discord feedback) | [ ] | |
| b | Remove `vial` and `vial_lock` feature definitions | [ ] | |
| c | Delete `rmk/src/host/via/` directory and related code | [ ] | |
| d | Delete `rmk-types/src/protocol/vial.rs` | [ ] | |
| e | Clean up Vial-related optional dependencies in `rmk/Cargo.toml` | [ ] | |
| f | Update documentation, remove all Vial-related content | [ ] | |
| g | Publish breaking-change version | [ ] | |

---

## Progress Summary

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | ICD Types and postcard-rpc Integration | **Complete** |
| 2 | Feature Gate and ProtocolService Skeleton | Not Started |
| 3 | USB Raw Bulk Transport | Not Started |
| 4 | System and Keymap Endpoints | Not Started |
| 5 | Security (Lock/Unlock) | Not Started |
| 6 | Remaining Endpoints | Not Started |
| 7 | Topics (Notifications) | Not Started |
| 8 | BLE Serial Transport | Not Started |
| 9 | Host Tool and Migration | Not Started |

---

## Key Files Reference

| File | Purpose |
|------|---------|
| `final_plan.md` | Full design specification |
| `rmk-types/src/protocol/rmk.rs` | ICD types and endpoint/topic definitions |
| `rmk/src/host/protocol/mod.rs` | ProtocolService and dispatch loop |
| `rmk/src/host/protocol/transport.rs` | Transport adapters (raw USB bulk, BLE serial) |
| `rmk/src/host/protocol/topics.rs` | Event bus -> Topic bridging |
| `rmk/src/host/lock.rs` | Shared lock/unlock logic |
| `rmk/src/host/mod.rs` | Feature-gated task spawning |
| `rmk/src/storage/mod.rs` | Flash persistence (HostMessage) |
| `rmk/src/usb/mod.rs` | USB class setup (vendor bulk + HID) |
