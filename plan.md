# Plan: implement `rmk_protocol`

## Context

RMK currently exposes runtime keymap configuration to a host PC via Vial — a
byte-oriented protocol carried over a 32-byte HID report pair (USB and BLE).
Vial is opaque: the firmware re-derives layout state from raw bytes via custom
deserialization (`rmk-types/src/protocol/vial.rs`, ~26 KB of hand-written
parsing), and the matching host code lives entirely outside the project.

We want a second host-communication protocol, `rmk_protocol`, that:

1. uses RMK's canonical types (`KeyAction`, `Combo`, `Morse`, `Fork`,
   `EncoderAction`, `BatteryStatus`, `BleStatus`) on the wire instead of bytes;
2. lets firmware *and* host share those types from one `rmk-types` crate;
3. is meaningfully faster than Vial's 32-byte HID reports;
4. stays transport-agnostic (USB + BLE today, more later).

The Interface Control Document (ICD) for this protocol — every endpoint,
topic, request/response type, max-size bound, schema-hash snapshot, and
versioning policy — has already been built out in
`rmk-types/src/protocol/rmk/`. The Cargo features (`rmk_protocol`,
`bulk_transfer`, `host_security`) are already declared and `vial` /
`rmk_protocol` are mutually exclusive. `rmk-host-tool/` is an empty
placeholder. **Everything that remains is the firmware-side server, the two
transport adapters, the host-side library + CLI, and one example flip.**

This plan delivers those pieces end-to-end, keeping Vial fully working when
its feature is selected.

## The protocol

The ICD lives in `rmk-types/src/protocol/rmk/` and is the source of truth.
What follows is a self-contained recap so the rest of this plan reads
without context-switching to that crate.

**Wire model.** postcard-rpc endpoint dispatch over a COBS-framed byte
stream (USB bulk, BLE notify/write). Every endpoint carries dual 8-byte
schema hashes (request + response) derived from the postcard schema, not
from the Rust path. The hash list is locked in
`rmk-types/src/protocol/rmk/snapshots/` and verified by
`endpoint_keys_*_locked` / `topic_keys_*_locked` tests; regen requires
`UPDATE_SNAPSHOTS=1`.

**Handshake.** `sys/version` is the single immortal endpoint — its path
and `ProtocolVersion` shape never change, even across major bumps. Hosts
call `GetVersion` first, bail on `major` mismatch or `minor` >
supported, then call `GetCapabilities` to learn layout dimensions and
feature flags, then gate every subsequent call on those flags.

**Versioning rule.**
- `minor` bump: new endpoint, new field appended to a wire struct, new
  variant in a wire enum (including `RmkError`).
- `major` bump: endpoint removed/retyped, struct field reshaped, enum
  variant renamed/renumbered.
- No bump: no wire change.

**Lock model.** The ICD declares a three-phase physical-key challenge —
`GetLockStatus` → `UnlockRequest` returns an `UnlockChallenge` listing
≤2 key positions → device transitions to `unlocked` once those keys are
physically held. **v1 ships always-unlocked**: the ICD endpoints stay
declared (their wire keys are frozen in snapshots), but the firmware
stubs them — `GetLockStatus` returns `locked: false`, `UnlockRequest`
returns an empty challenge, `LockRequest` is a no-op, and writes are
not gated. Resurrecting the gate is a focused follow-up that lifts the
existing `vial_lock.rs` state machine into `host/lock.rs` and threads a
shared `Mutex<HostLock>` through both Servers.

**Endpoint registry.** 28 endpoints across system / keymap / encoder /
macro_data / combo / morse / fork / behavior / connection / status / ble.
Bulk variants for keymap, combo, and morse are gated by Cargo feature
`bulk_transfer` (linearized row-major with `start_row, start_col,
count`); BLE-only endpoints are gated by `_ble`.

**Topics.** Server→client push, 5 base + 2 BLE-conditional:
`LayerChange`, `WpmUpdate`, `ConnectionChange`, `SleepState`,
`LedIndicator`, plus `BatteryStatus` and `BleStatusChange` under `_ble`.

**Errors.** `RmkError { InvalidParameter, BadState, InternalError }`;
write endpoints return `RmkResult = Result<(), RmkError>`.

## Firmware-side server design

The firmware realizes the protocol above as a system of per-transport
servers sharing a common context. The seven design choices below
interlock — they are presented in dependency order.

1. **Per-transport `Server`, shared context.** Each active transport
   (USB, BLE) owns its own `Server` instance with its own RX buffer and
   dispatch table. Both share a single `&KeyMap` by reference; in v1
   that is the *entire* shared state — no router, no demultiplexer, no
   lock state (see §3.7). `Sender<Tx>` is `Clone`
   (`postcard-rpc/src/server/mod.rs:193`) and `WireTx::send` already
   serializes via an internal `Mutex`, so per-transport `Sender`s share
   no state and don't contend.

   The split is forced, not stylistic: `Server::run` owns its `Rx` and
   the dispatch buffer exclusively
   (`~/.cargo/registry/src/*/postcard-rpc-0.12.1/src/server/mod.rs:455-491`),
   so a single `Server` cannot `select!` over two `WireRx` sources
   without breaking buf reuse. Splitting also gives a clean failure
   boundary — a stalled USB Tx (host disconnected mid-transfer) cannot
   block BLE handlers and vice versa. `KeyMap` (a
   `RefCell<KeyMapInner>`, `rmk/src/keymap.rs:79-81`) exposes only sync
   `&self` methods that borrow-mutate-drop within a single call; under
   embassy's cooperative scheduler, two Servers calling
   `keymap.set_action_at(...)` interleave only at `.await` boundaries
   and never panic. Handlers MUST NOT introduce code paths that hold a
   `RefCell` borrow across `.await` (documented at the top of
   `host/rmk_protocol/handlers/mod.rs`).

2. **Transport adapter contract.** Each transport implements
   `WireTx` / `WireRx` and exposes a `Runnable` task that joins the
   embassy executor pool. The postcard-rpc 0.12.1 stock adapters pin
   `embassy-usb 0.5` + `embassy-sync 0.7`; RMK is on `0.6 / 0.8`. The
   `Server` core and `define_dispatch!` are version-agnostic, so we
   roll our own thin adapters (~250–400 LoC across two transport
   modules) templated from the registry's
   `~/.cargo/registry/src/*/postcard-rpc-0.12.1/src/server/impls/embassy_usb_v0_5.rs`
   (the `Mutex<TxInner>` pattern), adjusted to `embassy-sync 0.8`.

   `Server::run` is `pub async fn run(&mut self)`
   (`postcard-rpc-0.12.1/src/server/mod.rs:455`) and returns whenever
   `WireTx` produces `Timeout` / `ConnectionClosed` or `WireRx`
   produces `ConnectionClosed`
   (`postcard-rpc-0.12.1/src/server/mod.rs:468-488`). Because it
   borrows `&mut self`, each Server is wrapped in
   `loop { server.run().await; transport_ready.wait().await }` — the
   one Server instance is reused across reconnects, and only the
   inner `WireRx` / `WireTx` state needs to re-`wait_connection` on
   reentry (the loop body in `Server::run` itself already calls
   `rx.wait_connection().await; tx.tx.wait_connection().await` per
   iteration, so no extra reset is required for the buffers). For
   USB, `transport_ready` is the existing `UsbDeviceState::Configured`
   signal. For BLE, **a new `Signal` (`BLE_RMK_PROTOCOL_READY`) is
   added in `rmk/src/channel.rs` and signaled from the new
   `cfg(feature = "rmk_protocol")` arm in `gatt_events_task`
   (`ble/mod.rs:245-364`) on the first CCCD subscribe to
   `input_data`** — subscribe-to-CCCD is not currently exposed as a
   public event, so this signal is the plumbing the BLE Server's
   outer loop awaits. BLE notify back-pressure (trouble's outgoing
   queue full) `.await`s on the Tx `Mutex` rather than dropping;
   embassy-sync's async `Mutex` is FIFO-fair so topic publishes can't
   starve replies.

3. **USB transport: vendor-class bulk pair.** Class `0xFF`, single
   bulk-IN + bulk-OUT, 64 B FS / 512 B HS max packet, COBS framing so
   RPC frames span multiple packets. The vendor function carries an
   Interface Association Descriptor — the existing builder is
   configured `composite_with_iads = true`
   (`rmk/src/usb/mod.rs:158-161`), and Windows pairs a bare vendor
   interface with the wrong function without one. WinUSB MSOS 2.0
   descriptors emitted via `embassy_usb::msos`; the host CLI ships a
   matching GUID and a Linux udev rule. Chosen over HID-raw for
   ~10–100× the throughput, accepting the cross-platform driver tax.

4. **BLE transport: dedicated GATT primary service.** Separate UUID
   (NOT under HID UUID `0x1812`); two characteristics —
   `output_data` (write, write-without-response) and `input_data`
   (notify) — both sized to MTU − 3 (`[u8; 244]` for the typical 247 B
   MTU). COBS-framed; frames may span notifies. Existing BLE bonds
   remain valid; clients with cached GATT attribute handles
   re-discover once.

5. **Handler contract.** Async fn shape
   `async fn h(ctx: &mut Ctx, hdr: VarHeader, req: Req) -> Resp`. The
   `RefCell`-borrow-across-`.await` ban is enforced by `KeyMap`'s
   sync-only API (see §3.1); the rule is written down for the rare
   handler that touches `KeyMapInner` directly. v1 handlers do not
   gate writes on lock state — that gate is deferred to v2 (see
   §3.7). Bulk-write handlers `await FLASH_OPERATION_FINISHED` between
   chunks rather than `try_send`, to apply back-pressure rather than
   drop under load. The full list of `KeyMap` accessors and
   `FLASH_CHANNEL` operations to reuse lives under "Critical files to
   modify".

6. **Topic publisher contract.** One task per active transport
   (`usb_topic_pub`, `ble_topic_pub`) — *not* one task per topic. Each
   task owns one typed `EventSubscriber` per topic via the
   `#[event]`-generated associated function
   (`LayerChangeEvent::subscriber()` etc., as used in
   `rmk/src/split/driver.rs:92`) and `select!`s across them in a loop,
   minting its own wrapping `u32` `VarSeq` and calling
   `sender.publish::<TopicTy>(seq, &msg).await.ok()` on each event.
   Per-task seq counters mean USB and BLE keep independent sequence
   spaces — no cross-transport coordination needed. Subscribed events:
   `LayerChangeEvent` / `ConnectionChangeEvent` / `SleepStateEvent` /
   `LedIndicatorEvent` / `WpmUpdateEvent`, plus
   `BatteryStatusEvent` / `BleStatusChangeEvent` under `_ble`. No new
   event channels are introduced; per-feature subscriber bumps are
   added to `rmk-config/src/default_config/subscriber_default.toml`
   (the existing feature-aware `[[subscriber]]` mechanism processed by
   `apply_feature_subscriber_bumps` in
   `rmk-config/src/resolved/build_constants.rs:186`), so a USB-only
   build only pays for the USB publisher and a `_ble`-disabled build
   doesn't grow `battery_status` / `ble_status_change`. Exact entries
   in "Critical files".

7. **Lock deferred to v2.** v1 ships always-unlocked. Lock-related
   endpoints are stubbed at the handler layer (`GetLockStatus` →
   `LockStatus { locked: false, awaiting_keys: false, remaining_keys: 0 }`,
   `UnlockRequest` → empty `UnlockChallenge`, `LockRequest` →
   `Ok(())`); writes are unconditional. No `host/lock.rs` module is
   introduced; Vial's `vial_lock.rs` stays exactly where it is and
   keeps its current feature gating. The `host_security` Cargo
   feature is **not** pulled into `rmk_protocol`'s feature set in v1
   — currently `rmk/Cargo.toml:123` includes it, that dependency is
   dropped, and the comment on `rmk/Cargo.toml:119` is updated from
   "shared between vial_lock and rmk_protocol" to "used by vial_lock;
   rmk_protocol will re-pull it in v2 when the host lock gate
   lands". The follow-up that resurrects the gate lifts
   `vial_lock.rs` into `host/lock.rs`, threads a single shared
   `Mutex<HostLock>` through both Servers, and re-adds the
   `host_security` dependency.

## Build order

Each phase realizes one or more sections of the firmware-side design.
File paths and line numbers live in "Critical files to modify" — not
re-listed inline.

- **Phase 1: Transport plumbing** — realizes §3.2–3.4. New
  `rmk/src/host/rmk_protocol/{mod,wire_usb,wire_ble,spawn}.rs`;
  modifications to `rmk/src/usb/mod.rs` and
  `rmk/src/ble/{ble_server,mod}.rs` to expose the bulk endpoints / GATT
  service. `define_dispatch! { app: RmkProtocolApp; … }` in `mod.rs`
  registers the 28 endpoints + 7 topics.
- **Phase 2: Handlers** — realizes §3.5. Add 11 handler modules under
  `rmk_protocol/handlers/` (system, keymap, encoder, macro_data,
  combo, morse, fork, behavior, connection, status, ble). Lock
  endpoints (`GetLockStatus`, `UnlockRequest`, `LockRequest`) are
  stubbed in `system.rs` per §3.7 — no shared lock state, no gate on
  writes.
- **Phase 3: Topic publishers** — realizes §3.6. Add
  `rmk_protocol/topics.rs` with one `select!`-driven task per
  transport; add feature-gated `[[subscriber]]` entries to
  `subscriber_default.toml` so each active transport contributes +1
  per topic (USB → `rmk_protocol`; BLE → `rmk_protocol + _ble`).
- **Phase 4: Config + macro wiring** — `HostConfig.rmk_protocol_enabled`,
  orchestrator feature/config cross-check, entry-point codegen branch,
  split-peripheral guard (peripherals don't host the protocol).
- **Phase 5: Host crate** — initialize `rmk-host-tool/` (currently
  empty) as a self-contained Cargo workspace: add
  `rmk-host-tool/Cargo.toml` with `[workspace] members = ["rmk-host",
  "rmk-cli"]`, then create the two sub-crates: `rmk-host/` (library —
  wraps `postcard_rpc::HostClient`, centralizes the §2 handshake in
  `Client::connect()`, exposes typed per-domain wrappers) and
  `rmk-cli/` (clap binary with `info`, `dump-keymap`, `set-key`,
  `bootloader`, `reset`, `monitor` subcommands; non-`info`
  subcommands gate on capability flags). **No `lock` / `unlock`
  subcommands in v1** — §3.7 stubs the underlying endpoints, so a
  CLI surface for them would silently no-op; the v2 follow-up that
  resurrects the gate also adds the CLI.
- **Phase 6: Tests, example, docs** — std-feature loopback integration
  test, COBS reframer unit tests, flip `examples/use_rust/rp2040/` to
  `rmk_protocol`, add the matrix variant to `scripts/test_all.sh`,
  write `docs/docs/main/docs/features/rmk_protocol.md`.

## Critical files to modify

**Firmware — new files**
- `rmk/src/host/rmk_protocol/{mod,wire_usb,wire_ble,spawn,topics}.rs`.
- `rmk/src/host/rmk_protocol/handlers/{system,keymap,encoder,macro_data,combo,morse,fork,behavior,connection,status,ble}.rs`.

**Firmware — modifications**
- `rmk/src/host/mod.rs` — add `rmk_protocol` submodule + `HostService`
  re-export branch.
- `rmk/src/usb/mod.rs` — gate the existing `host_rw` HID pair on
  `cfg(feature = "vial")` (currently `cfg(feature = "host")`, see
  `usb/mod.rs:199, 232`); under `cfg(feature = "rmk_protocol")` add
  `Builder::function(0xFF, 0x00, 0x00).interface().alt_setting(…)
  .endpoint_bulk_in(None, 64) / .endpoint_bulk_out(None, 64)` and the
  WinUSB MSOS descriptors via `embassy_usb::msos::{self, windows_version}`
  (template: `embassy_usb_v0_5.rs:140-151` from the postcard-rpc
  registry copy).
- `rmk/src/ble/ble_server.rs` — under `cfg(feature = "rmk_protocol")`
  replace `VialService` GATT struct with `RmkProtocolService` carrying
  the larger characteristics; gate Vial-style fields on
  `cfg(feature = "vial")`. Update both `Server` variants
  (with-host / without-host).
- `rmk/src/ble/mod.rs:245-364` — `gatt_events_task` arm under
  `cfg(feature = "rmk_protocol")` pushing write payloads into a
  larger-frame channel.
- `rmk/src/channel.rs` — under `cfg(feature = "rmk_protocol")` add
  (a) `RMK_PROTOCOL_REQUEST_CHANNEL: Channel<RawMutex,
  heapless::Vec<u8, RMK_PROTOCOL_FRAME_MAX>, RMK_PROTOCOL_CHANNEL_SIZE>`
  to carry MTU-sized BLE write payloads from `gatt_events_task` to
  the BLE `WireRx` (sized to one MTU − 3 = 244 bytes; channel depth
  4 — one in-flight + headroom); (b) `BLE_RMK_PROTOCOL_READY:
  Signal<RawMutex, ()>` set by `gatt_events_task` on the first CCCD
  subscribe and awaited by the BLE Server's outer reentry loop (per
  §3.2). The USB transport reads bulk-OUT directly via
  `embassy_usb::driver::EndpointOut`, so it does *not* use the
  request channel. Vial's 32-byte `HOST_REQUEST_CHANNEL` stays
  untouched.

**Reused functions / channels (do not re-implement in handlers)**
- Layout reads/writes: `KeyMap::get_action_at` / `set_action_at` /
  `get_keymap_config` / `get_action_by_flat_index` /
  `set_action_by_flat_index` (`rmk/src/keymap.rs:430, 451, 487, 626, 635`).
- Encoder: `KeyMap::get_encoder_action` / `set_encoder_clockwise` /
  `set_encoder_counter_clockwise` (`keymap.rs:644, 652, 664`).
- Morse: `KeyMap::get_morse` / `with_morse_mut` (`keymap.rs:575, 579`).
- Combo / fork: `KeyMap::with_combos_mut` / `with_forks`
  (`keymap.rs:585, 595`).
- Macro buffer: `KeyMap::read_macro_buffer` / `write_macro_buffer` /
  `get_macro_sequences` (`keymap.rs:683, 692, 705`).
- Behavior config: `KeyMap::set_combo_timeout` /
  `set_one_shot_timeout` / `set_tap_interval` /
  `set_tap_capslock_interval` (`keymap.rs:549-563`).
- Persistence: `FLASH_CHANNEL.send(FlashOperationMessage::*)` for
  `KeymapKey`, `Encoder`, `Combo`, `Fork`, `Morse`, `MacroData`,
  `LayoutOptions`, `DefaultLayer`, `Reset`, `ResetLayout` (variants
  declared in `storage/mod.rs:92-150`; executor match arms in
  `storage/mod.rs:673-820`). Bulk handlers `await
  FLASH_OPERATION_FINISHED` (`storage/mod.rs:35`) between chunks.
- Boot: `boot::reboot_keyboard()` / `boot::jump_to_bootloader()`
  (`rmk/src/boot.rs`) — *not* bare
  `cortex_m::peripheral::SCB::sys_reset()`.

**Config / macro layer**
- `rmk-config/src/lib.rs` — `HostConfig.rmk_protocol_enabled: bool`
  (default false), parallel to `vial_enabled`.

> **Naming note.** The rmk crate's `host = ["dep:byteorder"]`
> feature (rmk/Cargo.toml:111) and rmk-types' `host = ["rmk_protocol",
> "bulk", "_ble", "split"]` feature (rmk-types/Cargo.toml:44) are
> *unrelated* despite the shared name. `rmk_protocol = ["host", ...,
> "rmk-types/rmk_protocol"]` pulls only `rmk-types/rmk_protocol`, not
> `rmk-types/host`. Don't confuse the two when reading Cargo.toml.
- `rmk-config/src/default_config/subscriber_default.toml` — add
  feature-gated `[[subscriber]]` entries (consumed by
  `apply_feature_subscriber_bumps` in
  `rmk-config/src/resolved/build_constants.rs:186`). Two entries:
  - `features = ["rmk_protocol"]` → events `layer_change`,
    `connection_change`, `sleep_state`, `wpm_update`, `led_indicator`
    (one slot each for the USB topic publisher).
  - `features = ["rmk_protocol", "_ble"]` → events `layer_change`,
    `connection_change`, `sleep_state`, `wpm_update`,
    `led_indicator`, `battery_status`, `ble_status_change` (one slot
    each for the BLE topic publisher; the BLE-only topics are listed
    here, not in the previous entry).
  - Verify the toml-to-const-generic path: `subs` reaches
    `PubSubChannel<_, _, _, _, SUBS>` via the `SUBSCRIBER_COUNT`
    constants emitted by `rmk-types/build.rs`. If any channel is
    declared with literal numbers, fix those too.
- `rmk-macro/src/codegen/orchestrator.rs:79-100` — extend the
  cross-check so `host.rmk_protocol_enabled` and the `rmk_protocol`
  Cargo feature must agree (mirrors existing `vial_enabled` validation).
- `rmk-macro/src/codegen/orchestrator.rs:253` `host_service_init` —
  produce `RmkProtocolService::new` when `rmk_protocol_enabled`.
- `rmk-macro/src/codegen/entry.rs:79-95` — extend the existing
  `host_service_task` match: when `vial_enabled`, keep today's
  `host_service.run()`. When `rmk_protocol_enabled`, push
  `usb_rmk_protocol_server.run()` *only if* the resolved
  `communication` includes USB, and `ble_rmk_protocol_server.run()`
  *only if* it includes BLE — the two task pushes mirror the
  existing `transport_setup(communication)` gating, so a USB-only
  board doesn't get a no-op BLE server task and vice versa. `vial`
  and `rmk_protocol` remain mutually exclusive, and the entire
  rmk_protocol branch is wrapped in
  `cfg(not(feature = "_split_peripheral"))`.

**Host crate (new sub-crates inside `rmk-host-tool/`)**
- `rmk-host-tool/rmk-host/Cargo.toml` — deps:
  `rmk-types = { path = "../../rmk-types", features = ["host"] }`,
  `postcard-rpc = "0.12"` with `use-std + raw-nusb + cobs-serial`,
  `nusb`, `tokio`, `thiserror`, `anyhow`.
- `rmk-host-tool/rmk-host/src/lib.rs` — `Client` wrapping
  `postcard_rpc::HostClient<…>`, `Transport` enum
  (`Usb(UsbTransport)`, `BleSerial(Box<dyn AsyncRead + AsyncWrite>)`),
  typed wrappers per endpoint group
  (`client.get_keymap`, `set_keymap`, `dump_keymap`, `lock`/`unlock`,
  `subscribe_layers`). Handshake (`GetVersion` → bail on `major`
  mismatch + `minor` > supported → `GetCapabilities` → cache caps for
  feature-gating) is centralized in `Client::connect()`. WinUSB MSOS
  descriptor GUID matches the firmware's; Linux udev rule snippet
  documented in README.
- `rmk-host-tool/rmk-cli/` — `clap` binary, subcommands listed under
  Phase 5. `info` exercises the version+capability handshake; the rest
  gate on capability flags.

**Examples + scripts + docs**
- `examples/use_rust/rp2040/Cargo.toml` — the example currently has
  no `[features]` section and pulls Vial transitively via the rmk
  crate's `default = ["defmt", "storage", "vial", "vial_lock"]`. Flip
  the existing `rmk = { path = "../../../rmk", features = ["rp2040"] }`
  line to `rmk = { path = "../../../rmk", default-features = false,
  features = ["rmk_protocol", "bulk_transfer", "rp2040", "defmt",
  "storage", "async_matrix"] }` — `defmt` and `storage` must be
  re-added by hand because turning off defaults drops them. Verify
  it builds. Keep the Vial-default examples untouched.
- `scripts/test_all.sh` — add a `rmk_protocol,_ble,split,storage`
  matrix variant.
- `docs/docs/main/docs/features/rmk_protocol.md` (the docs tree
  lives under `docs/docs/main/docs/`, not `docs/docs/main/`) —
  model overview, versioning rule, capability discovery,
  lock/unlock (note v1-stubbed status), USB driver installation per
  OS, Vial migration notes. Source the prose from the module-level
  doc in `rmk-types/src/protocol/rmk/mod.rs`. Add the page to
  `docs/docs/main/docs/_meta.json` so it appears in the nav.

**Tests**
- `rmk/tests/rmk_protocol_loopback.rs` — std-feature integration test
  using Tokio-channel-backed `WireTx`/`WireRx` (template:
  `~/.cargo/registry/src/*/postcard-rpc-0.12.1/src/server/impls/test_channels.rs`).
  Round-trips `GetVersion`, `GetCapabilities`,
  `GetKeyAction` / `SetKeyAction`, the lock-stub endpoints
  (`GetLockStatus` → `locked: false`, `UnlockRequest` → empty
  `UnlockChallenge`, `LockRequest` → `Ok(())`), and `Reboot`-as-no-op
  (mock).
- COBS reframer unit tests in `wire_usb.rs` / `wire_ble.rs`:
  partial-frame across packet boundaries, zero-byte across boundary,
  oversized frame discarded.

## Verification

End-to-end smoke test once Phases 1–5 land:

1. **Firmware build matrix** — `sh scripts/test_all.sh` passes
   including the new `rmk_protocol` row.
2. **Unit + integration tests** — from `rmk/`:
   `cargo nextest run --no-default-features --features=rmk_protocol,bulk_transfer,_ble,split,storage,async_matrix,std`.
3. **Wire-format snapshots stable** — from `rmk-types/`:
   `cargo test --features rmk_protocol,bulk` passes the
   `endpoint_keys_*_locked` and `topic_keys_*_locked` snapshots
   without an `UPDATE_SNAPSHOTS` regen. Confirms no schema drift while
   wiring up the firmware.
4. **Flashable example boots** — flash the modified
   `examples/use_rust/rp2040`; verify USB enumerates with the new
   vendor bulk interface (Linux: `lsusb -v` shows class FF and two
   bulk endpoints).
5. **Loopback CLI smoke**:
   - `cargo run -p rmk-cli -- info` returns
     `ProtocolVersion { major: 1, minor: 0 }` + a populated
     `DeviceCapabilities`.
   - `rmk-cli dump-keymap` round-trips bytes that match the firmware's
     compiled-in default.
   - `rmk-cli set-key 0 0 0 KC_A` flips a key; reboot the keyboard;
     `dump-keymap` shows the persisted change.
   - The lock stub is exercised by the loopback integration test
     (`GetLockStatus` → `locked: false`), not by a CLI command —
     v1 ships no `lock`/`unlock` subcommands per §3.7.
   - `rmk-cli monitor layers` prints layer-change events while the
     device toggles layers physically.
6. **Vial regression** — re-flash one Vial-default example
   (e.g. `examples/use_rust/nrf52840_ble`) and confirm Vial.app still
   pairs and edits keymaps. Mutually exclusive features must remain
   mutually exclusive.

If any of (1)–(3) fails, stop and fix before moving on; (4)–(6) require
a physical device.
