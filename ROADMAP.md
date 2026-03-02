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
| g | Define `UnlockChallenge { key_positions: Vec<(u8, u8), MAX_UNLOCK_KEYS> }` with `MAX_UNLOCK_KEYS = 4` | [x] | |
| h | Define `KeyPosition { layer: u8, row: u8, col: u8 }` | [x] | |
| i | Define `BulkRequest { layer: u8, start_row: u8, start_col: u8, count: u16 }` with `MAX_BULK = 32` | [x] | |
| j | Define `StorageResetMode` enum (`Full`, `LayoutOnly`) | [x] | |
| k | Define Topic payload types: `LayerChangePayload`, `WpmPayload`, `BatteryPayload`, `BleStatePayload`, `BleProfilePayload`, `ConnectionPayload`, `SleepPayload`, `LedPayload` | [x] | See final_plan.md Section 8 |
| l | Define connection/status types: `ConnectionInfo`, `ConnectionType`, `BatteryStatus`, `MatrixState`, `SplitStatus` | [x] | |
| m | Define macro types: `MacroInfo`, `MacroData` | [x] | |
| n | Define combo/morse/fork config types: `ComboConfig`, `MorseConfig`, `ForkConfig` (or reuse existing types from rmk-types) | [x] | Protocol-facing types defined; firmware types in rmk crate left as-is |
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
| e | Run `cargo test -p rmk-types` to confirm all tests pass | [x] | 29/29 tests pass |

---

## Phase 2: Feature Gate and ProtocolService Skeleton

**Goal**: Establish the new protocol's code structure alongside Vial.

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
| c | Define `ProtocolService` struct with fields: `&'a RefCell<KeyMap<ROW, COL, NUM_LAYER, NUM_ENCODER>>`, lock state (`bool`), RX buffer `[u8; BUF_SIZE]`, TX buffer `[u8; BUF_SIZE]`, transport (generic `T: Read + Write`) | [ ] | Follow VialService pattern |
| d | Implement `ProtocolService::new()` constructor | [ ] | |
| e | Implement `ProtocolService::run()` async main loop skeleton (`loop { self.process().await }` pattern) | [ ] | |
| f | Gate entire module with `#[cfg(feature = "rmk_protocol")]` | [ ] | |

### Step 2.3 — Implement dispatch loop

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | In `process()`, implement: read bytes from transport until `0x00` delimiter (COBS frame end marker) | [ ] | |
| b | Decode COBS frame using `postcard::take_from_bytes_cobs` or `cobs` crate | [ ] | |
| c | Parse discriminant byte, extract key length and seq_no length | [ ] | See final_plan.md Section 6.3 |
| d | Extract key bytes, match against registered endpoint keys | [ ] | |
| e | On match: deserialize payload with `postcard::from_bytes`, call corresponding handler | [ ] | |
| f | On no match: construct `WireError::UnknownKey` error response | [ ] | |
| g | Serialize handler return value to TX buffer with `postcard::to_slice_cobs` | [ ] | |
| h | Write serialized COBS frame to transport | [ ] | |
| i | Add error handling: on frame parse failure, skip current frame and continue reading (resync) | [ ] | |

### Step 2.4 — Feature-gated task spawning

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Add `#[cfg(feature = "rmk_protocol")] pub(crate) mod protocol;` in `rmk/src/host/mod.rs` | [ ] | Parallel to existing `#[cfg(feature = "vial")] pub(crate) mod via;` |
| b | Add `#[cfg(feature = "rmk_protocol")]` block to create and spawn `ProtocolService` async task | [ ] | Follow existing Vial task spawn code |
| c | Ensure `ProtocolService` task receives the same `&RefCell<KeyMap>` reference as `VialService` | [ ] | |
| d | Run `cargo check -p rmk --no-default-features --features=rmk_protocol` to verify task spawn compiles | [ ] | |

### Step 2.5 — Rename `VialMessage` -> `HostMessage`

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | In `rmk/src/storage/mod.rs`, rename `FlashOperationMessage::VialMessage` to `FlashOperationMessage::HostMessage` | [ ] | |
| b | Search all references to `VialMessage` and update (`rmk/src/host/via/mod.rs`, `rmk/src/host/storage.rs`, etc.) | [ ] | Use `cargo check` to find all |
| c | Keep `#[cfg(feature = "host")]` on the `HostMessage` variant (usable by both vial and rmk_protocol) | [ ] | |
| d | Run `cargo test -p rmk --no-default-features --features=vial,storage` to confirm Vial still works | [ ] | |

---

## Phase 3: USB CDC-ACM Transport

**Goal**: Get the first working transport for desktop testing.

### Step 3.1 — Implement `embedded_io_async` for CDC-ACM

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Create `rmk/src/host/protocol/transport.rs` | [ ] | |
| b | Check if `embassy_usb::class::cdc_acm::CdcAcmClass` already implements `embedded_io_async::Read` + `Write`; if so, use directly | [ ] | May already be provided by embassy |
| c | If not, create newtype wrapper `CdcTransport<'d, D: Driver<'d>>` wrapping `CdcAcmClass<'d, D>` | [ ] | |
| d | Implement `embedded_io_async::Read` for wrapper: map `CdcAcmClass::read_packet()` to `Read::read()` | [ ] | |
| e | Implement `embedded_io_async::Write` for wrapper: map `CdcAcmClass::write_packet()` to `Write::write()` | [ ] | |
| f | Handle CDC-ACM connect/disconnect state (`wait_connection()` / `CdcAcmClass::line_coding()` checks) | [ ] | |
| g | Gate with `#[cfg(feature = "rmk_protocol")]` | [ ] | |

### Step 3.2 — Make `ProtocolService` generic over transport

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Change `ProtocolService` transport field type to generic `T: embedded_io_async::Read + embedded_io_async::Write` | [ ] | |
| b | Update type constraints in `new()` and `run()` | [ ] | |
| c | Ensure all read/write operations in dispatch loop use generic trait methods | [ ] | |
| d | Run `cargo check` to confirm generic parameters propagate correctly | [ ] | |

### Step 3.3 — Add CDC-ACM class to USB setup

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | In `rmk/src/usb/mod.rs`, under `#[cfg(feature = "rmk_protocol")]`, add CDC-ACM class creation | [ ] | |
| b | In `new_usb_builder()` or related init function, use `embassy_usb::class::cdc_acm::CdcAcmClass::new()` to register CDC-ACM interface | [ ] | |
| c | Ensure CDC-ACM coexists with existing HID composite device (IAD support) | [ ] | USB composite |
| d | Pass created `CdcAcmClass` instance to `ProtocolService` constructor | [ ] | |
| e | Test USB enumeration in an example project: after plugging in, host should see both HID and CDC-ACM devices | [ ] | |

### Step 3.4 — Integration test: USB handshake

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Build firmware with `rmk_protocol` feature on an nRF52840 or RP2040 example | [ ] | |
| b | Flash firmware, open USB CDC serial port with `picocom` / `screen` / `minicom` to confirm communication | [ ] | |
| c | Send `GetVersion` request using postcard-rpc client or hand-crafted bytes | [ ] | |
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
| b | Implement `SetKeyAction` handler: receive `(KeyPosition, KeyAction)`, call `keymap.borrow_mut().set_action_at()` to update in-memory state | [ ] | |
| c | In `SetKeyAction` handler, send `FLASH_CHANNEL.send(FlashOperationMessage::HostMessage(KeymapData::KeymapKey(...)))` for flash persistence | [ ] | Follow VialService pattern |
| d | Add parameter validation: return `RmkError::InvalidParameter` when layer/row/col is out of bounds | [ ] | |
| e | Test: read key action at (0,0,0), modify it, read again to verify consistency | [ ] | |

### Step 4.3 — Bulk keymap and layer handlers

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Implement `GetKeymapBulk` handler: batch-read KeyActions per `BulkRequest`, fill `heapless::Vec<KeyAction, MAX_BULK>` | [ ] | Row-major order |
| b | Implement `SetKeymapBulk` handler: batch-set KeyActions and send individual `FlashOperationMessage` per key | [ ] | |
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
| b | Add dependencies: `postcard-rpc` (client mode), `serialport` (USB CDC serial), `clap` (CLI arg parsing) | [ ] | |
| c | Implement serial connection logic: auto-scan or specify port to connect to CDC-ACM device | [ ] | |
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
| a | Implement `GetEncoderAction` handler: receive `(encoder_id: u8, layer: u8)`, read `EncoderAction` from `keymap.borrow().encoders` | [ ] | |
| b | Implement `SetEncoderAction` handler: receive `(encoder_id, layer, EncoderAction)`, update in-memory + send `FlashOperationMessage` | [ ] | |
| c | Add parameter validation: return `RmkError::InvalidParameter` when encoder_id or layer is out of bounds | [ ] | |
| d | Test: read/modify encoder action, verify persistence | [ ] | |

### Step 6.2 — Macro endpoints

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Implement `GetMacroInfo` handler: return `MacroInfo { max_macros, macro_space_size }` | [ ] | |
| b | Implement `GetMacro` handler: receive macro index, read `MacroData` from `BehaviorConfig.macros` | [ ] | |
| c | Implement `SetMacro` handler: receive `(index, MacroData)`, update in-memory + persist | [ ] | |
| d | Implement `ResetMacros` handler: clear all macro definitions + send flash reset message | [ ] | |
| e | Test: complete macro CRUD flow | [ ] | |

### Step 6.3 — Combo endpoints

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Implement `GetCombo` handler: receive combo index, read `ComboConfig` from `BehaviorConfig.combos` | [ ] | |
| b | Implement `SetCombo` handler: receive `(index, ComboConfig)`, update in-memory + persist | [ ] | |
| c | Implement `ResetCombos` handler: clear all combos + flash reset | [ ] | |
| d | Test: combo config read/write and reset | [ ] | |

### Step 6.4 — Morse/Tap-Dance endpoints

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Implement `GetMorse` handler: receive morse index, read `MorseConfig` | [ ] | |
| b | Implement `SetMorse` handler: update morse config + persist | [ ] | |
| c | Implement `ResetMorse` handler: reset all morse configs | [ ] | |
| d | Test: complete morse config CRUD | [ ] | |

### Step 6.5 — Fork endpoints

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Implement `GetFork` handler: receive fork index, read `ForkConfig` | [ ] | |
| b | Implement `SetFork` handler: update fork config + persist | [ ] | |
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
| a | Implement `GetBatteryStatus` handler: read battery state (level percentage, charging status) | [ ] | `#[cfg(feature = "_ble")]` |
| b | Implement `GetCurrentLayer` handler: get currently active layer from `keymap.borrow()` | [ ] | |
| c | Implement `GetMatrixState` handler: read matrix key states, return `MatrixState` | [ ] | |
| d | Implement `GetSplitStatus` handler: return split peripheral connection status | [ ] | `#[cfg(feature = "split")]` |
| e | Endpoints without matching feature return `WireError::UnknownKey` | [ ] | |
| f | Test: each status query under matching and non-matching feature configs | [ ] | |

---

## Phase 7: Topics (Notifications)

**Goal**: Device-to-host event streaming.

### Step 7.1 — Event bridging module

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Create `rmk/src/host/protocol/topics.rs` | [ ] | |
| b | Create subscribers for each internal event: `LayerChangeEvent::subscriber()`, `WpmUpdateEvent::subscriber()`, `BatteryStateEvent::subscriber()`, `BleStateChangeEvent::subscriber()`, `BleProfileChangeEvent::subscriber()`, `ConnectionChangeEvent::subscriber()`, `SleepStateEvent::subscriber()`, `LedIndicatorEvent::subscriber()` | [ ] | |
| c | Implement conversion functions for each event: internal event -> Topic payload struct (e.g., `LayerChangeEvent` -> `LayerChangePayload`) | [ ] | |
| d | Implement `encode_topic_frame()` function: serialize Topic payload into COBS frame (discriminant + topic key + seq=0 + payload) | [ ] | |
| e | Gate BLE-related topics (`BatteryState`, `BleStateChange`, `BleProfileChange`) with `#[cfg]` | [ ] | |

### Step 7.2 — Integrate Topics into ProtocolService

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | In `ProtocolService::run()` main loop, use `embassy_futures::select` to simultaneously await transport reads and event subscribers | [ ] | |
| b | When event subscriber receives an event, call `encode_topic_frame()` and write to transport | [ ] | |
| c | Ensure endpoint request processing and topic sending don't block each other (interleave via select) | [ ] | |
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

### Step 8.2 — BLE serial `embedded_io_async` wrapper

| # | Action | Status | Notes |
|---|--------|--------|-------|
| a | Create `BleTransport` struct wrapping GATT service RX/TX channels | [ ] | |
| b | Implement `embedded_io_async::Read` for `BleTransport`: read data from RX buffer | [ ] | |
| c | Implement `embedded_io_async::Write` for `BleTransport`: send data via TX characteristic notify | [ ] | |
| d | Handle BLE connect/disconnect events: `read()` returns EOF on disconnect | [ ] | |
| e | Pass `BleTransport` to `ProtocolService::new()` — ProtocolService works without any modification | [ ] | |

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
| a | Technology decision: Tauri (desktop app) vs Rust->WASM + WebSerial (pure web) | [ ] | WebSerial easier to distribute |
| b | Set up frontend project scaffold (React/Svelte + TypeScript) | [ ] | |
| c | Implement WebSerial connection layer: connect to CDC-ACM device via browser Serial API | [ ] | |
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
| d | Implement WebHID version of protocol client (for environments without WebSerial support) | [ ] | |

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
| 3 | USB CDC-ACM Transport | Not Started |
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
| `rmk/src/host/protocol/transport.rs` | Transport adapters (CDC-ACM, BLE) |
| `rmk/src/host/protocol/topics.rs` | Event bus -> Topic bridging |
| `rmk/src/host/lock.rs` | Shared lock/unlock logic |
| `rmk/src/host/mod.rs` | Feature-gated task spawning |
| `rmk/src/storage/mod.rs` | Flash persistence (HostMessage) |
| `rmk/src/usb/mod.rs` | USB class setup (CDC-ACM) |
