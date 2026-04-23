# Final Plan: RMK Host↔Keyboard Communication Layer Redesign

## Context

RMK's input side is a clean `Runnable`-based pipeline (`InputDevice` + `Processor` + `#[event]` pub/sub + `run_all!`). The host-facing side never got that treatment:

- **HID fan-out duplication.** `UsbKeyboardWriter::write_report` (`rmk/src/usb/mod.rs:66-117`, 52 lines) and `BleHidServer::write_report` (`rmk/src/ble/ble_server.rs:102-149`, 48 lines) hold parallel 4-way `match report { ... }` — ~100 lines of the same dispatch, split by transport.
- **`rmk_protocol` stubbed.** `rmk-types` ships a complete ICD (28 base request endpoints across 10 groups, plus 3 bulk groups and 2 BLE-gated groups; 5 base + 2 BLE-gated topics), but firmware-side dispatch is `todo!()` at `rmk/src/host/mod.rs:37`. A `compile_error!` at `rmk/src/lib.rs:21-22` forbids `rmk_protocol + vial` together.
- **Conflated "no host" machinery.** `run_dummy_keyboard` (`rmk/src/ble/mod.rs:920-937`) spawns a `DummyWriter` that sets `CONNECTION_STATE = Connected` and drains the channel forever during the "no transport ready" window. This is load-bearing — matrix scanning is gated on `Connected` (`matrix.rs:86`) so profile-switch combos must keep working — but `DummyWriter` conflates build-time "no writer" stub with runtime pre-connection drain, and dual-transport still funnels through a single `RunnableHidWriter` that cannot fan out.
- **Shared service surface missing.** `process_via_packet` (229 lines) + `vial.rs` (542 lines) touch `KeyMap` / `FLASH_CHANNEL` / `publish_event` / `boot::jump_to_bootloader` directly. `rmk_protocol` needs exactly those operations; without extraction, they get duplicated.

**Goal.** Land a modular comms layer that (1) collapses the ~100-line HID duplication, (2) gives Vial and `rmk_protocol` one shared operation surface, (3) ships `rmk_protocol` over USB vendor bulk and BLE custom GATT via `postcard-rpc 0.12`, (4) keeps split on its peer-sync protocol, (5) preserves every wire format frozen in `rmk-types`, and (6) replaces `DummyWriter` + `RunnableHidWriter` + `run_dummy_keyboard` with three monomorphic free functions — `run_router_single` / `run_router_dual` / `run_report_drain`.

---

## Design Principles

1. **`Runnable` where state; free `async fn` otherwise.** Long-lived stateful services (`VialService<RW>`, `PeripheralManager`) implement `async fn run(&mut self) -> !` via `Runnable`. Stateless pumps (the three `run_router_*`, per-topic republishers) are plain `async fn` returning `!` composed via `select`/`join`. No new scheduler trait.
2. **Protocol-owned framing.** Vial (HID 32/32), `rmk_protocol` (postcard-rpc with COBS), HID reports (typed `Report` variants), split (`SplitMessage` + COBS). No unified byte trait at the service boundary — the `WireTx::send(&self, …)` vs `WireRx::receive(&mut self, …)` asymmetry is load-bearing.
3. **Shared application logic via `HostOps`.** A concrete struct borrowing `&KeyMap<'a>`; sync by default (`KeyMap` is a `RefCell` with sync methods), separate async `persist_*` surface for ops that also emit a `FLASH_CHANNEL` message.
4. **Ordered single-consumer HID reports with selective fan-out.** Keep `KEYBOARD_REPORT_CHANNEL: Channel<Report, 16>` (MPSC, ordered). Fan-out lives in three monomorphic free functions — one per shape (1 sink, 2 sinks + policy, no sink). Reports drop when no sink is ready; this preserves the pre-connection matrix-scan semantics that `DummyWriter` supplies today. Blocking on readiness would stall `keyboard.rs:278`'s `send().await`, suspend matrix scanning, and break profile-switch combos.
5. **Separate endpoints per host protocol.** Vial stays on HID raw 32/32 (USB) / HID-GATT (BLE). `rmk_protocol` gets USB vendor bulk and BLE custom GATT. Both coexist once the service and transport layers stabilise.
6. **Split stays on its own protocol.** Peer sync, not host request/response. `SplitMessage::Key` is ~3-5 bytes; postcard-rpc adds up to 13 bytes of `VarHeader` — prohibitive at ~1 kHz scan over ATT MTU 23. `define_dispatch!` assumes request/response; split is fire-and-forget. Unchanged.
7. **Low-churn module layout.** Targeted additions. No top-level `comm/` umbrella.

---

## Verified Baseline (HEAD `2e6c1821`)

### What exists

- `rmk/src/hid.rs` (161 lines): `Report` enum (`KeyboardReport`/`MouseReport`/`MediaKeyboardReport`/`SystemControlReport`), `HidWriterTrait` (async `write_report(&mut self, report: Self::ReportType) -> Result<usize, HidError>`), `HidReaderTrait`, `RunnableHidWriter::run_writer` (handles `USB_REMOTE_WAKEUP` retry on `EndpointError::Disabled`, lines 66-91), `DummyWriter` (lines 106-128).
- `rmk/src/channel.rs:17`: `static KEYBOARD_REPORT_CHANNEL: Channel<RawMutex, Report, REPORT_CHANNEL_SIZE>` (default 16). Plain MPSC.
- `rmk/src/usb/mod.rs`: `UsbKeyboardWriter<'a, 'd, D: Driver<'d>>` (lines 44-54); `HidWriterTrait` impl (52-line per-variant match). `USB_CONFIGURED: Watch<RawMutex, (), 1>` at line 252 (readiness = `contains_value()`). `USB_REMOTE_WAKEUP: Signal<RawMutex, ()>` at line 19.
- `rmk/src/ble/ble_server.rs`: `BleHidServer<'stack, 'server, 'conn, P: PacketPool>` (lines 79-85); `HidWriterTrait` impl (48-line parallel match). **The two ~100-line duplication targets.**
- `rmk/src/ble/mod.rs:920-937`: `run_dummy_keyboard` runs `DummyWriter` alongside storage during the no-connection window. `DummyWriter::run_writer` (`hid.rs:117-123`) stores `CONNECTION_STATE = Connected` then drains `KEYBOARD_REPORT_CHANNEL` forever.
- `rmk/src/ble/mod.rs:425, 481, 527`: three one-shot `let _ = KEYBOARD_REPORT_CHANNEL.receive().await;` in advertising-timeout branches. Each drops exactly one wake report.
- `rmk/src/host/mod.rs` (38 lines): `pub use via::UsbVialReaderWriter as UsbHostReaderWriter` (line 6, gated on `host` but Vial-specific); `pub(crate) use via::VialService as HostService` (line 8, `vial`-gated); `run_host_communicate_task` with `todo!()` body at line 37 when `vial` is off. `ble/mod.rs:241-242` allocates `host_reader_writer: HidReaderWriter<…, ViaReport, 32, 32>` under `#[cfg(all(not(_no_usb), host))]` — Vial-specific despite the `host` gate. Consumers at `ble/mod.rs:388, 443` and `lib.rs:267` are `host`-gated. A `host + not-vial` build compiles but panics at runtime. Phase 4 fixes the gates.
- `rmk/src/host/via/mod.rs` (360 lines): `VialService<RW>` generic over `ViaReport` reader/writer; `process_via_packet` dispatcher (229 lines across `via/mod.rs:80-308`).
- `rmk/src/host/via/vial.rs` (542 lines): `process_vial` handles Vial sub-commands.
- `rmk/src/ble/host_service/{mod.rs, vial.rs}`: `HOST_GUI_INPUT_CHANNEL: Channel<RawMutex, [u8; 32], VIAL_CHANNEL_SIZE>`; `BleVialServer` bridges GATT ↔ channel ↔ `VialService`.
- `rmk/src/keymap.rs`: `KeyMap<'a> { inner: RefCell<KeyMapInner<'a>> }`. All operations are sync `RefCell` borrows. No async on `KeyMap`.
- `rmk/src/event/mod.rs`: `publish_event(e)` (sync, line 198), `publish_event_async(e).await` (line 207). `#[event(channel_size, pubs, subs)]` selects `Channel` (subs = 0) or `PubSubChannel` (subs > 0) at macro-expansion time from `rmk-macro`; per-event counts come from `rmk-config`-emitted `constants.rs` consts referenced by the macro. Per-event defaults in `rmk-config/src/default_config/event_default.toml`.
- All `KEYBOARD_REPORT_CHANNEL.send(...).await` producers (`keyboard.rs:278`, `input_device/pointing.rs:306`, `input_device/joystick.rs:68`) use async back-pressure.
- `rmk/src/input_device/mod.rs:19-21`: `pub trait Runnable { async fn run(&mut self) -> !; }`.
- `rmk-types/src/protocol/rmk/endpoints.rs` (446 lines): 10 base groups (28 base request endpoints), 3 bulk groups (`#[cfg(feature = "bulk")]`), 2 BLE-gated groups, 1 split-and-BLE group; `ENDPOINT_LIST` pre-composed at `:271`. Snapshot tests freeze endpoint hashes.
- `rmk-types/src/protocol/rmk/topics.rs` (68 lines): 5 base + 2 BLE-gated = 7 topics.
- `rmk/Cargo.toml` features: `host = ["dep:byteorder"]`; `vial = ["host"]`; `host_security = []`; `rmk_protocol = ["host", "host_security", "dep:postcard-rpc", "rmk-types/rmk_protocol"]`; `bulk_transfer = ["rmk_protocol", "rmk-types/bulk"]`.
- `postcard-rpc 0.12.1` (declared, unused): `server::WireTx::send(&self, …)`, `WireTx::send_raw(&self, …)` (both `&self` so a `Sender<Tx>` clone can sit in every handler and topic task); `WireRx::receive(&mut self, …)` (exclusive). `Sender<Tx>` derives `Clone` where `Tx: Clone` and its `reply<E>` / `publish<T>` / `error` methods are fully generic over `Tx: WireTx`. `Server::sender(&self)` under `Tx: WireTx + Clone`. `Dispatch` trait at `server/mod.rs:515` has `type Tx: WireTx` — the `impl` may be generic over `Tx`. `define_dispatch!` (`server/dispatch_macro.rs:258`) bakes `tx_impl`/`spawn_impl`/`spawn_fn` concretely, so it cannot serve a `run<Rx, Tx>` directly; RMK wraps it with a custom `define_rmk_dispatch!` (§6) that emits a generic dispatcher. `test-utils` feature exposes `server::impls::test_channels` for in-memory tests (requires `use-std`).

### What does NOT exist (should never be referenced)

Nothing at `rmk/src/host/rynk/`, `rmk/src/ble/host/`, `HostGatt`, `SessionManager`, `ControlEndpoint`, `FrameIo`, `ByteFramedLink`, `OutputProcessor`, `rmk/src/comm/`, or any `PubSubChannel<Report>`. No postcard-rpc server runs in firmware today.

---

## Target Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│ L3  Services           awaited in run_keyboard or *_task fns     │
│     run_router_{single,dual,report_drain}   — report pumps       │
│     VialService<RW>                         — impl Runnable       │
│     run_rmk_protocol_task<Rx,Tx>            — postcard-rpc Server │
│                                               + topic republishers│
│     PeripheralManager / SplitPeripheral     — unchanged           │
├──────────────────────────────────────────────────────────────────┤
│ L2  Protocol/codec                     PROTOCOL-OWNED             │
│     HID reports : ReportSink (4 sends + default dispatch)         │
│                 : TransportStatus (is_ready)                      │
│     Vial        : HidReaderTrait<ViaReport> + HidWriterTrait<…>   │
│     rmk_protocol: postcard_rpc::server::{WireRx, WireTx, Sender}  │
│     split       : SplitReader / SplitWriter + SplitMessage        │
├──────────────────────────────────────────────────────────────────┤
│ L1  Wire                                                          │
│     embassy_usb::class::hid::{HidReaderWriter, HidWriter}         │
│     embassy_usb::driver::{EndpointIn, EndpointOut}  (bulk)        │
│     trouble_host::Characteristic<[u8; N]>                         │
└──────────────────────────────────────────────────────────────────┘

 HID report path
   keyboard.rs / pointing.rs / joystick.rs
        → KEYBOARD_REPORT_CHANNEL (Channel<Report, 16>)
        → one of { run_router_single, run_router_dual, run_report_drain }
        → UsbKeyboardWriter / BleHidServer  (both impl ReportSink + TransportStatus)

 Control plane
   Vial:          VialService<RW>      ─┐
   rmk_protocol:  run_rmk_protocol_task ┴─→ HostOps(&KeyMap) ─→ KeyMap (RefCell)
                                                              → FLASH_CHANNEL (async)
                                                              → boot::jump_to_bootloader
                                                              → LOCK_STATE (lock.rs static)
```

---

## Core Abstractions

### 1. `ReportSink` — single HID fan-out point

Add in `rmk/src/hid.rs`:

```rust
pub(crate) trait ReportSink {
    type Error;
    async fn send_keyboard(&mut self, r: &KeyboardReport)    -> Result<usize, Self::Error>;
    async fn send_mouse   (&mut self, r: &MouseReport)       -> Result<usize, Self::Error>;
    async fn send_media   (&mut self, r: &MediaKeyboardReport)-> Result<usize, Self::Error>;
    async fn send_system  (&mut self, r: &SystemControlReport)-> Result<usize, Self::Error>;

    /// Default: the only surviving Report-variant match in the tree.
    async fn send_report(&mut self, r: &Report) -> Result<usize, Self::Error> {
        match r {
            Report::KeyboardReport(x)      => self.send_keyboard(x).await,
            Report::MouseReport(x)         => self.send_mouse(x).await,
            Report::MediaKeyboardReport(x) => self.send_media(x).await,
            Report::SystemControlReport(x) => self.send_system(x).await,
        }
    }
}
```

**Impl targets.** `UsbKeyboardWriter` — keyboard → `keyboard_writer` (EP8); mouse/media/system → `other_writer` (EP9) with the right `CompositeReportType` prefix byte. `BleHidServer` — each variant notifies its own characteristic (`input_keyboard`, `mouse_report`, `media_report`, `system_report`).

**Phase 1 transition.** Phase 1 keeps the existing `impl HidWriterTrait<ReportType = Report>` blocks on both types so `RunnableHidWriter::run_writer`'s bound still holds; their bodies shrink to a single `self.send_report(&r).await`. Phase 2 deletes `RunnableHidWriter` and those `HidWriterTrait<Report>` impls in one step — after Phase 2 nothing consumes `HidWriterTrait<Report>` and the trait survives only with `ReportType = ViaReport` (for Vial). No blanket impl is introduced just to delete it later.

### 2. Report routing — three free functions

New file `rmk/src/report_router.rs`. The firmware has exactly three shapes — one sink, two sinks + policy, no sink. Each gets a monomorphic free function, not a `Router<U, B>` struct with stubs for absent transports.

```rust
#[derive(Clone, Copy)]
pub(crate) enum HidOutputPolicy {
    PreferUsb, // USB first; fall back to BLE. Matches today's CONNECTION_TYPE=Usb.
    PreferBle, // Symmetric.                                          =Ble.
}

pub(crate) trait TransportStatus {
    /// Whether this sink can accept a report right now. Checked once per
    /// report; a stale true is treated as drop-for-that-sink when send_* errors.
    fn is_ready(&self) -> bool;
}

/// One sink: every report goes to the one sink; drop on send error.
pub(crate) async fn run_router_single<S: ReportSink<Error = HidError>>(sink: &mut S) -> ! {
    loop {
        let report = KEYBOARD_REPORT_CHANNEL.receive().await;
        if let Err(e) = sink.send_report(&report).await {
            debug!("send_report failed; drop: {:?}", e);
        }
    }
}

/// Two sinks: policy-driven fan-out.
pub(crate) async fn run_router_dual<U, B>(usb: &mut U, ble: &mut B, policy: HidOutputPolicy) -> !
where U: ReportSink<Error = HidError> + TransportStatus,
      B: ReportSink<Error = HidError> + TransportStatus,
{
    loop {
        let report = KEYBOARD_REPORT_CHANNEL.receive().await;
        match policy {
            HidOutputPolicy::PreferUsb => {
                if usb.is_ready() {
                    if usb.send_report(&report).await.is_err() && ble.is_ready() {
                        let _ = ble.send_report(&report).await;
                    }
                } else if ble.is_ready() {
                    let _ = ble.send_report(&report).await;
                } // else drop — no ready sink
            }
            HidOutputPolicy::PreferBle => { /* symmetric */ }
        }
    }
}

/// No sink: pre-connection window and the _no_transport build. Takes over
/// DummyWriter's responsibility to keep CONNECTION_STATE = Connected so
/// matrix.rs:86 does not suspend scanning.
pub(crate) async fn run_report_drain() -> ! {
    CONNECTION_STATE.store(ConnectionState::Connected.into(), Ordering::Release);
    loop { let _ = KEYBOARD_REPORT_CHANNEL.receive().await; }
}
```

**`TransportStatus` impls.**
- `UsbKeyboardWriter::is_ready()` → `USB_CONFIGURED.contains_value()`.
- `BleHidServer::is_ready()` → underlying `GattConnection` reports connected. (No redundant `CONNECTION_STATE` check — if the connection is up, `CONNECTION_STATE` is already `Connected`, and in dual builds we only care whether *this sink* can send.)

**USB wakeup retry lives inside `UsbKeyboardWriter`, not in the router.** The existing `USB_REMOTE_WAKEUP`-signal-and-retry dance (`hid.rs:71-88`) moves into a private `write_with_wakeup_retry` helper in `usb/mod.rs`; every `send_*` method of `UsbKeyboardWriter` calls it. BLE-only builds don't compile the helper. Router sees a plain `Result<_, HidError>` with the retry already applied.

**Why three free functions, not one `Router<U, B>`.** The three shapes need genuinely different ergonomics; a unified struct either forces `dyn` (incompatible with `embassy-usb` / `trouble-host` non-`'static` lifetimes) or introduces a `NoTransport` stub whose `core::future::pending()` would stall the keyboard if ever called. Three monomorphic fns also mean `run_keyboard`'s `router_fut: impl Future<Output = ()>` parameter stays generic-free — no new `Router: Runnable` bound.

**Correctness.**
- *Never block on readiness.* Blocking fills the 16-slot channel → stalls producers → suspends matrix scanning → breaks profile-switch combos. Dropping is the correct existing semantic.
- *Readiness can flip between `is_ready()` and `send_*`.* A failed send is drop-for-that-sink; for `PreferUsb`/`PreferBle` the fallback path re-checks the other sink's readiness. Reports are never re-queued — matches today's notify-failure behaviour.
- *Fallback can split a report stream across hosts.* When `PreferUsb` sees USB fail mid-send then recover on the next report, one intermediate snapshot lands on BLE only; the next lands back on USB. HID reports are absolute state (full modifier mask + key codes), so no stuck-key risk — but transient "both-pressed" frames may appear on one host and not the other, which matters for macro/combo-timing validation on dual-host setups.
- *Only `run_report_drain()` writes `CONNECTION_STATE`.* `run_keyboard` already stores Connected at entry and Disconnected at exit (`lib.rs:309, 356`); router functions run inside that scope.
- *Advertising-timeout wake sites at `ble/mod.rs:425, 481, 527` stay byte-for-byte.* The one report per wake is already discarded (no host attached); rewiring them to subscribe to `KeyboardEvent` would need an event-subs bump for zero user-visible benefit.

### 3. `HostOps` — shared application surface

New file `rmk/src/host/ops.rs`. **One file, not a submodule tree** — ~700 lines of thin wrappers over two source files does not justify splitting.

Concrete struct, not a trait-with-one-impl (matches RMK's generics-first culture; trait-wrap later is non-breaking if a second backend ever lands). **Sync by default** — `FLASH_CHANNEL.send(...).await` is the only async operation; every other op is a `RefCell` borrow or an atomic lookup. Collapsing everything into `async fn` would make callers `.await` on zero-cost reads.

```rust
// Borrows only &KeyMap. LOCK_STATE lives in host/lock.rs (§4); FLASH_CHANNEL,
// USB_REMOTE_WAKEUP, CONNECTION_STATE are accessed the same way KeyMap accesses
// them today (statics). One lifetime parameter.
pub(crate) struct HostOps<'a> { keymap: &'a KeyMap<'a> }

impl<'a> HostOps<'a> {
    pub fn new(keymap: &'a KeyMap<'a>) -> Self { Self { keymap } }
}

// Wire-facing types come from rmk-types (what both Vial and rmk_protocol
// ultimately serialise). `BehaviorConfig` here is the 4-u16 Copy wire struct
// from rmk-types::protocol::rmk::system, NOT the in-memory
// rmk::config::BehaviorConfig. Individual fine-grained setters exist for
// Vial's per-field handlers; set_behavior_config is a bulk convenience that
// decomposes the wire struct into the same calls.
//
// Error model
//  sync reads  → T or Option<T> (infallible; RefCell borrow cannot fail — see Concurrency)
//  sync setters→ RmkResult when wire endpoint is RmkResult-typed (out-of-range → InvalidParameter,
//                locked → BadState, internal → InternalError); () otherwise
//  persist_*   → () (FLASH_CHANNEL.send is infallible; storage task surfaces failures)
//  submit_unlock → RmkResult (rmk_protocol handler maps 1:1; Vial continues log-and-continue)

impl<'a> HostOps<'a> {
    // KeyMap reads
    pub fn get_key_action    (&self, layer: u8, row: u8, col: u8) -> KeyAction;
    pub fn get_encoder_action(&self, layer: u8, id: u8) -> EncoderAction;
    pub fn macro_count(&self) -> u8;
    pub fn layer_count(&self) -> u8;
    pub fn get_default_layer(&self) -> u8;
    pub fn current_layer(&self) -> u8;
    pub fn get_combo(&self, idx: u8) -> Option<ComboConfig>;
    pub fn get_morse(&self, idx: u8) -> Option<Morse>;
    pub fn get_fork (&self, idx: u8) -> Option<Fork>;
    pub fn get_behavior_config(&self) -> BehaviorConfig;
    pub fn get_connection_type(&self) -> ConnectionType;
    pub fn read_macro_buffer(&self, offset: usize, dst: &mut [u8]);
    pub fn read_matrix_state(&self, dst: &mut [u8]);
    pub fn firmware_version(&self) -> u32;
    pub fn uptime_ms(&self) -> u32;

    // KeyMap mutations. publish_event emission stays inside KeyMap where it
    // already lives (LayerChangeEvent from update_tri_layer, etc.) — HostOps
    // forwards and inherits. No new publish_event sites inside HostOps.
    // Keys use (layer, row, col) matching `FlashOperationMessage::KeymapKey`.
    pub fn set_key_action    (&self, layer: u8, row: u8, col: u8, a: KeyAction) -> RmkResult;
    pub fn set_encoder_action(&self, layer: u8, id: u8, a: EncoderAction)       -> RmkResult;
    pub fn set_default_layer (&self, layer: u8)                                 -> RmkResult;
    pub fn set_combo (&self, idx: u8, c: ComboConfig) -> RmkResult;
    pub fn set_morse (&self, idx: u8, m: Morse)       -> RmkResult;
    pub fn set_fork  (&self, idx: u8, f: Fork)        -> RmkResult;
    pub fn set_behavior_config(&self, cfg: BehaviorConfig) -> RmkResult;
    pub fn set_connection_type(&self, ty: ConnectionType)  -> RmkResult;
    pub fn write_macro_buffer (&self, offset: usize, src: &[u8]);

    // Fine-grained setters used by Vial's per-field BehaviorSetting arms
    pub fn set_combo_timeout_ms        (&self, ms: u16);
    pub fn set_one_shot_timeout_ms     (&self, ms: u16);
    pub fn set_tap_interval_ms         (&self, ms: u16);
    pub fn set_tap_capslock_interval_ms(&self, ms: u16);
    pub fn set_morse_default_profile   (&self, p: MorseProfile);
    pub fn set_prior_idle_time_ms      (&self, ms: u16);

    // Boot. jump_to_bootloader is not `-> !` — on platforms without a
    // bootloader-jump, boot::jump_to_bootloader logs and returns. reboot is
    // new; cortex-m: SCB::sys_reset(); other arches: warn+return.
    pub fn jump_to_bootloader(&self);
    pub fn reboot(&self);
}

// Persistence (feature = "storage"). Each pushes one FlashOperationMessage
// via FLASH_CHANNEL.send().await (1-3 lines). persist_behavior_config
// decomposes the wire struct into per-field persist calls (same messages
// Vial already sends). No try_persist_* variant — the one caller that
// intentionally wanted non-blocking behaviour (DynamicKeymapSetBuffer at
// via/mod.rs:277-284, which loops without awaiting mid-iteration) keeps
// its inline try_send in the Vial parser arm.
#[cfg(feature = "storage")]
impl<'a> HostOps<'a> {
    pub async fn persist_key_action    (&self, layer: u8, row: u8, col: u8, a: KeyAction);
    pub async fn persist_encoder_action(&self, layer: u8, id: u8, a: EncoderAction);
    pub async fn persist_default_layer (&self, layer: u8);
    pub async fn persist_layout_options(&self, options: u32);
    pub async fn persist_macro_buffer  (&self);
    pub async fn persist_combo (&self, idx: u8, c: ComboConfig);
    pub async fn persist_morse (&self, idx: u8, m: Morse);
    pub async fn persist_fork  (&self, idx: u8, f: Fork);
    pub async fn persist_behavior_config    (&self, cfg: BehaviorConfig);
    pub async fn persist_combo_timeout_ms   (&self, ms: u16);
    pub async fn persist_one_shot_timeout_ms(&self, ms: u16);
    pub async fn persist_tap_interval_ms    (&self, ms: u16);
    pub async fn persist_tap_capslock_interval_ms(&self, ms: u16);
    pub async fn persist_morse_default_profile(&self, p: MorseProfile);
    pub async fn persist_connection_type (&self, ty: ConnectionType);
    pub async fn reset_storage           (&self);
}

// Security (feature = "host_security"). All four delegate to the single
// LOCK_STATE static in host/lock.rs (§4). Vial's unlock handshake reaches
// these through HostOps rather than touching the lock primitive.
#[cfg(feature = "host_security")]
impl<'a> HostOps<'a> {
    pub fn lock_status  (&self) -> LockStatus;
    pub fn begin_unlock (&self) -> UnlockChallenge;
    pub fn submit_unlock(&self, response: UnlockResponse) -> RmkResult;
    pub fn lock         (&self);
}

// BLE profile management (feature = "_ble")
#[cfg(feature = "_ble")]
impl<'a> HostOps<'a> {
    pub fn get_ble_status(&self) -> BleStatus;
    pub async fn switch_ble_profile(&self, profile: u8) -> RmkResult;
    pub async fn clear_ble_profile (&self, profile: u8) -> RmkResult;
}
```

**Not in Phase 3.** `peripheral_status(id)` — peripheral state lives in `PeripheralManager`, not `KeyMap`; no `rmk_protocol` endpoint consumes it in Phases 4-6. Out of scope.

**Concurrency invariant.** In the coexistence build (Phase 7+), Vial and `rmk_protocol` are separate tasks sharing the same `KeyMap: RefCell`. **No `HostOps` method holds a `RefCell` borrow across an `.await`.** All sync methods trivially satisfy this.

Mechanical enforcement (not code-review discipline): every `persist_*` is built on a private `with_keymap<F, T>(&self, f: F) -> T where F: FnOnce(&KeyMapInner<'a>) -> T` helper (plus a `with_keymap_mut` sibling). The closure is non-`async`, so `.await` inside it is a type-error by construction; `T` returned by value forces the borrow to end at the helper boundary. `persist_*` shape: `let msg = self.with_keymap(|k| …build FlashOperationMessage…); FLASH_CHANNEL.send(msg).await;` — read under borrow, drop, then await. A `#![deny(clippy::await_holding_refcell_ref)]` at the top of `ops.rs` catches any `borrow()`/`borrow_mut()` that bypasses the helper. Rule, pattern, and lint both belong in `ops.rs`'s module docstring.

**Contract preserved.** Every storage mutation still flows through `FLASH_CHANNEL`; every state update other tasks care about still publishes its event. `HostOps` owns the existing contract, it does not bypass it.

### 4. `LockState` — one object, two views

New file `rmk/src/host/lock.rs`: defines `LockState` (atomic-backed lock bit + buffered matrix state for the Vial physical-key challenge) and `pub(crate) static LOCK_STATE: LockState = LockState::new();`. The file compiles under `feature = "host"` (transitively required by both `vial_lock` and `host_security`); the *wrappers* are independently gated.

- `vial_lock` → `vial_lock::VialLock` held by `VialService`; implements the Vial matrix-state challenge by reading/writing `LOCK_STATE`.
- `host_security` → `HostOps::{lock_status, begin_unlock, submit_unlock, lock}` (§3).

**Pre-Phase 7.** `VialLock` may keep its own bit today. Phase 7 rewires `VialLock` to delegate to `LOCK_STATE`; before that, the `compile_error!` at `lib.rs:21-22` forbids the coexistence build, so the two views cannot observe each other.

### 5. Vial — dispatch bodies move to `HostOps`

`VialService<RW>` stays in `rmk/src/host/via/mod.rs`, generic over `RW: HidReaderTrait<ReportType = ViaReport> + HidWriterTrait<ReportType = ViaReport>`. The existing `pub(crate) async fn run(&mut self)` (`via/mod.rs:53-67`) becomes `impl Runnable for VialService<RW> { async fn run(&mut self) -> ! }` — the loop already returns `!`.

Every arm of `process_via_packet` / `process_vial` is rewritten to call `HostOps` methods (1-3 lines each). Vial wire format and descriptor unchanged. USB transport (`UsbVialReaderWriter`, `via/mod.rs:319-360`) and BLE transport (`BleVialServer`, `ble/host_service/vial.rs`, 68 lines) stay as-is — both already feed the same `VialService`.

### 6. `rmk_protocol` — `postcard-rpc` directly, custom `define_rmk_dispatch!` for transport-agnostic dispatch

Use `postcard_rpc::server::{Server, WireRx, WireTx, Sender}` directly. No local `ControlEndpoint` / `ByteFramedLink` shim — the `&self` TX + `&mut self` RX asymmetry is load-bearing.

`postcard_rpc::define_dispatch!` (`server/dispatch_macro.rs:258`) is **not** adopted directly: it bakes `tx_impl`/`spawn_impl`/`spawn_fn` concretely into the generated dispatcher, so one macro invocation cannot serve test channels + USB bulk + BLE GATT from a single `run_rmk_protocol_task<Rx, Tx>`. Instead, RMK ships a wrapper macro `define_rmk_dispatch!` (~150 LoC, `dispatch.rs` below) that mirrors `define_dispatch!`'s shape and key-sizing machinery but emits a generic dispatcher: `pub struct RmkDispatcher<Tx, const N: usize> { context, device_map, _tx: PhantomData<Tx> }` with `impl<Tx: WireTx> Dispatch for RmkDispatcher<Tx, N> { type Tx = Tx; … }`. The `Sender<Tx>` API (`reply<E>` / `publish<T>` / `error`, `server/mod.rs:194-251`) is already generic over `Tx`, so the dispatch body is transport-agnostic — only the `type Tx = …` binding had to be lifted. RMK commits to **async/blocking-flavor handlers only** (no `spawn` flavor); the dropped flavor is what made the Spawn machinery — the only other transport-coupled bit — disappear with the macro rewrite. The `PingEndpoint` / `GetAllSchemasEndpoint` standard-ICD arms (`dispatch_macro.rs:187-198`) are emitted verbatim by the wrapper.

```
rmk/src/host/rmk_protocol/
├── mod.rs         pub(crate) async fn run<Rx, Tx>(keymap, rx, tx) — builds the
│                  RmkDispatcher<Tx>, wraps it in Server<Tx, Rx, Buf, _>, clones
│                  Sender<Tx> for each republisher, drives Server::run() + every
│                  republisher concurrently via join.
├── dispatch.rs    define_rmk_dispatch! macro + the single invocation that emits
│                  RmkDispatcher<Tx, N>. Mirrors postcard-rpc's sizer / @matcher
│                  shape; drops tx_impl/spawn_impl/spawn_fn parameters; only
│                  supports async + blocking ep_arm/tp_arm. Consumes
│                  rmk_types::protocol::rmk::ENDPOINT_LIST as-is.
├── handlers.rs    one handler per endpoint, 1-3 lines each calling HostOps.
│                  Referenced from define_rmk_dispatch! in dispatch.rs.
├── topics.rs      one republisher struct per (source event → target topic)
│                  pair (7 total). Each holds a Sender<Tx> clone + Subscriber<E>,
│                  exposes `async fn run(&mut self) -> !`, emits via
│                  Sender::publish::<M> with a per-struct rolling u32 seq.
│                  Most mappings are identity; non-identity is 1-3 inline lines.
├── context.rs     RpcContext<'a> { ops: HostOps<'a> } — no sender field;
│                  define_rmk_dispatch! passes sender into handlers directly.
└── transport/
    ├── usb_bulk.rs  UsbBulkRx: WireRx owns EndpointOut<'static, D> exclusively.
    │                UsbBulkTx: WireTx wraps &'static Mutex<RawMutex, EndpointIn<'static, D>>
    │                so WireTx::send (&self) can be shared across cloned Sender<Tx>.
    │                'static comes from the existing embassy-usb StaticCell
    │                Builder pattern, extended in add_usb_bulk_endpoints! (§8).
    │                Derive Clone on the thin wrapper (Mutex reference is cloned;
    │                the inner EndpointIn is not). COBS framing is implemented
    │                inline — embassy-usb endpoints are packet-oriented, not
    │                embedded-io-async byte streams, so postcard-rpc's ready-
    │                made EioWireRx/EioWireTx do not fit.
    └── ble_gatt.rs  BleRpcTx: notify handle + &'static Mutex, derive Clone.
                     BleRpcRx: drains the dedicated BLE RX channel (§7);
                     accumulates chunks until COBS sentinel.
```

Handlers consume the ICD at `rmk-types/src/protocol/rmk/*.rs` as-is. `define_rmk_dispatch!` accepts RMK's pre-composed `ENDPOINT_LIST` shape (its `endpoints`/`topics_in`/`topics_out` blocks each take one `list:`, identical to `postcard_rpc::define_dispatch!`).

**Buffer sizing.** `Server<Tx, Rx, Buf, D>` takes `Buf: DerefMut<Target = [u8]>`; service uses `&'static mut [u8; PROTOCOL_RPC_SERVER_BUF_SIZE]` from `static_cell::StaticCell`. Three new `rmk-config` constants:

| Constant | Default | Phase | Purpose |
|---|---|---|---|
| `protocol_rpc_server_buf_size` | 256 (1024 when `bulk`) | 4 | Server dispatch buffer |
| `protocol_rpc_chunk_size`       | 20 (BLE ATT MTU 23 − 3) / 64 (USB bulk) | 5/6 | Per-notify / per-write chunk |
| `protocol_rpc_channel_size`     | 4 | 6 | `HOST_RPC_INPUT_CHANNEL` depth |

**Topics bridged from the existing event bus** (one `TopicRepublisher` each, all 7 run concurrently alongside the Server loop):

| Topic | Source event | Feature gate |
|---|---|---|
| `LayerChangeTopic`      | `LayerChangeEvent`       | always |
| `WpmUpdateTopic`        | `WpmUpdateEvent`         | always |
| `ConnectionChangeTopic` | `ConnectionChangeEvent`  | always |
| `SleepStateTopic`       | `SleepStateEvent`        | always |
| `LedIndicatorTopic`     | `LedIndicatorEvent`      | always |
| `BatteryStatusTopic`    | `BatteryStatusEvent`     | `_ble` |
| `BleStatusChangeTopic`  | `BleStatusChangeEvent`   | `_ble` |

(All source events are always compiled; `display`/`_ble` gates on other subscribers like `PeripheralManager` don't affect these republishers.)

**Republisher behaviour when no host is subscribed.** `Sender::publish::<M>(seq, &payload)` returns `Err` when the RPC client is absent (BLE not notified / USB bulk IN times out). Republishers `log::trace!` and continue — source events are broadcasts the firmware emits regardless; a dropped notify is equivalent to "no client subscribed", which is the common case. No retry, no buffering beyond the source `PubSubChannel` itself. Republishers never exit (`-> !`).

**Feature-conditional event-subs bumps.** Each republisher consumes one subscriber slot on its source event. Subscribing requires `subs > 0`; a bump from 0→1 changes the event's channel shape (MPSC → `PubSubChannel`) at macro-expansion time. `rmk-config` gains a merge layer in `KeyboardTomlConfig::emit_event_constants` so users don't hand-track slot counts. Merge order (later wins): `event_default.toml` → chip default → **feature-conditional bumps** (table keyed by event name; `(feature_cfg, subs_delta)` pairs summed at build time) → user `keyboard.toml [event]`. Bumps apply *before* the user override so a user who writes `subs = 5` gets exactly 5; under-sizing triggers a `static_assertions::const_assert!` emitted alongside `constants.rs` — build-time failure beats runtime `.unwrap()` on `subscriber()`. Output feeds `build.rs`-emitted `constants.rs`, which `#[event]` reads at expansion.

Bumps added by this plan:

| Event | Gate | Δsubs | Phase |
|---|---|---|---|
| `layer_change`      | `rmk_protocol`              | +1 | 4 |
| `wpm_update`        | `rmk_protocol`              | +1 | 4 |
| `connection_change` | `rmk_protocol`              | +1 | 4 |
| `sleep_state`       | `rmk_protocol`              | +1 | 4 |
| `led_indicator`     | `rmk_protocol`              | +1 | 4 |
| `battery_status`    | `all(rmk_protocol, _ble)`   | +1 | 6 |
| `ble_status_change` | `all(rmk_protocol, _ble)`   | +1 | 6 |

**`test-utils` for in-memory integration tests.** `postcard-rpc/test-utils` requires `use-std`; enable it via `[dev-dependencies]` so it only affects `#[cfg(test)]` builds and never leaks into firmware.

### 7. BLE `rmk_protocol` transport — dedicated RX channel

New `rmk/src/ble/host_service/rmk_protocol.rs`:

- RX GATT characteristic — host writes chunks; handler forwards to the dedicated RX channel.
- TX GATT characteristic — firmware notifies; `BleRpcTx` holds the notifier.
- `HOST_RPC_INPUT_CHANNEL: Channel<RawMutex, heapless::Vec<u8, PROTOCOL_RPC_CHUNK_SIZE>, PROTOCOL_RPC_CHANNEL_SIZE>` — distinct from Vial's `HOST_GUI_INPUT_CHANNEL`. Defaults from the `rmk-config` constants (§6); both tunable via `keyboard.toml`. Chunk size negotiates upward when `trouble-host` exposes the connection MTU; default 20 (ATT_MTU 23 − 3-byte header).

`rmk/src/ble/ble_server.rs` defines `#[gatt_server]` variants. After all phases, four total (two net-new across Phases 6-7):

| Variant | Gate | Introduced |
|---|---|---|
| No-host            | `not(host)`                                              | pre-existing |
| Vial-only          | `all(host, vial, not(rmk_protocol))`                     | re-gated in Phase 6 (body unchanged; today's `host` variant) |
| rmk_protocol-only  | `all(host, rmk_protocol, not(vial))`                     | Phase 6 |
| Coexistence        | `all(host, vial, rmk_protocol)`                          | Phase 7 |

### 8. USB vendor-bulk endpoint allocation

New `add_usb_bulk_endpoints!` macro in `rmk/src/usb/mod.rs`, mirroring `add_usb_reader_writer!` / `add_usb_writer!` (same static-cell discipline):

- Vendor interface class `0xFF`, subclass `0x00`, protocol `0x00`.
- Bulk OUT + bulk IN; FS max_packet_size 64.
- Gated on `cfg(feature = "rmk_protocol")` — bulk endpoints are always present when `rmk_protocol` is on (even non-bulk endpoints need a byte path).
- Interface numbers auto-allocated by `embassy_usb::Builder`; no clash with Vial's HID raw interface.
- WinUSB / MS OS 2.0 descriptors deferred — first working host target is Linux/macOS, which open vendor bulk without custom drivers. A follow-up PR adds WinUSB before a Windows release. The `rmk_protocol` and `bulk_transfer` feature doc-comments in `rmk/Cargo.toml` call out the Windows host gap so users don't silently hit unrecognized-device errors.

---

## Module Layout

Only what moves or is added:

```
rmk/src/
├── hid.rs                      # +ReportSink (P1). −DummyWriter, −RunnableHidWriter (P2).
├── report_router.rs            # NEW (P2) HidOutputPolicy, TransportStatus, run_router_{single,dual,report_drain}
├── usb/
│   └── mod.rs                  # UsbKeyboardWriter → ReportSink (P1); +TransportStatus + internal
│                               #   write_with_wakeup_retry (P2); +add_usb_bulk_endpoints! (P5)
├── ble/
│   ├── mod.rs                  # run_dummy_keyboard → run_dummy_router = select(storage.run(),
│   │                           #   run_report_drain()); swap run_writer arg at every call site
│   │                           #   for the matching run_router_* future (P2); re-gate
│   │                           #   host_reader_writer allocation + consumers host → vial (P4)
│   ├── ble_server.rs           # BleHidServer → ReportSink + TransportStatus (P1/P2);
│   │                           #   gatt_server variants re-gated + added (P6/P7)
│   └── host_service/
│       ├── mod.rs              # unchanged (HOST_GUI_INPUT_CHANNEL)
│       ├── vial.rs             # unchanged transport glue
│       └── rmk_protocol.rs     # NEW (P6) HOST_RPC_INPUT_CHANNEL + GATT glue
├── host/
│   ├── mod.rs                  # run_host_communicate_task → run_vial_task + run_rmk_protocol_task;
│   │                           #   UsbHostReaderWriter alias re-gated host → vial (P4)
│   ├── ops.rs                  # NEW (P3) HostOps with feature-gated impl blocks
│   ├── lock.rs                 # NEW (P3) LockState + static LOCK_STATE
│   ├── via/                    # unchanged structure; P3 rewrites arms to call HostOps; P7 rewires
│   │                           #   VialLock onto LOCK_STATE
│   └── rmk_protocol/           # NEW (P4-P6) see §6
│       ├── mod.rs
│       ├── dispatch.rs         # define_rmk_dispatch! macro + RmkDispatcher<Tx, N>
│       ├── handlers.rs
│       ├── topics.rs
│       ├── context.rs
│       └── transport/
│           ├── usb_bulk.rs
│           └── ble_gatt.rs
└── split/                      # unchanged
```

No `comm/` umbrella. Nothing else moves.

---

## `run_*_task` signatures and `run_keyboard` evolution

Current `rmk/src/host/mod.rs:16-38` couples the spawned task to `Rw: HidReaderTrait<ViaReport> + HidWriterTrait<ViaReport>` — fits Vial but not `rmk_protocol`. Phase 4 replaces it with **two disjoint spawn functions**:

```rust
#[cfg(feature = "vial")]
pub(crate) async fn run_vial_task<'a, Rw>(keymap: &'a KeyMap<'a>, rw: Rw, cfg: VialConfig<'static>)
where Rw: HidReaderTrait<ReportType = ViaReport> + HidWriterTrait<ReportType = ViaReport>
{
    VialService::new(keymap, cfg, rw).run().await      // Runnable::run, returns !
}

#[cfg(feature = "rmk_protocol")]
pub(crate) async fn run_rmk_protocol_task<'a, Rx, Tx>(keymap: &'a KeyMap<'a>, rx: Rx, tx: Tx)
where Rx: postcard_rpc::server::WireRx, Tx: postcard_rpc::server::WireTx + Clone
{
    rmk_protocol::run(keymap, rx, tx).await            // builds Server, joins republishers
}
```

**`run_keyboard` evolution.**
- Today (`lib.rs:287-310`): `W: RunnableHidWriter`, `keyboard_writer: W`, `Rw: HidReaderTrait<ViaReport> + HidWriterTrait<ViaReport>`, `vial_config: VialConfig`.
- **Phase 2**: drop `W`/`keyboard_writer`; add `router_fut: impl Future<Output = ()>` (same shape as the existing `communication_fut`). Call sites construct the future from whichever of the three `run_router_*` fns applies.
- **Phase 4**: drop `Rw`/`vial_config`; add `host_fut: impl Future<Output = ()>`. Vial-only callers pass `run_vial_task(keymap, rw, cfg)`; rmk_protocol-only callers pass `run_rmk_protocol_task(keymap, rx, tx)`; coexistence callers (Phase 7) pass `async { join(run_vial_task(...), run_rmk_protocol_task(...)).await; }` (both services run forever, so `join` blocks forever — the desired shape).

---

## Vial + `rmk_protocol` coexistence

Target endpoint layout:

| Features | USB interfaces | BLE services |
| --- | --- | --- |
| `vial`         | HID raw 32/32         | HID-GATT Vial service |
| `rmk_protocol` | vendor bulk IN/OUT    | custom RPC GATT service |
| both           | HID raw 32/32 + bulk  | Vial + RPC services |

`compile_error!` at `lib.rs:21-22` stays until all of these land (Phase 7):

1. USB descriptor composition adds bulk without colliding with Vial's interface (embassy-usb's sequential allocation makes this automatic; CI covers both builds).
2. BLE `#[gatt_server]` gains the coexistence variant (Phase 7).
3. `VialLock` delegates to `LOCK_STATE` so both surfaces share one lock (Phase 7).
4. CI covers `--features=vial,rmk_protocol,storage[,_ble]`.

---

## Migration Plan

Seven independently mergeable phases. Dependencies:

```
P0 ─▶ P1 ─┬─▶ P2 ──────────────▶ P7
          └─▶ P3 ─▶ P4 ─▶ P5 ─▶ P6 ─▶ P7
```

P2 and P3 are siblings (both gated on P1) and may proceed on parallel PRs. P7 requires P2 (dual-transport router) + P6.

### Phase 0 — Baseline anchors

Run `sh scripts/test_all.sh` clean. Add byte-level unit tests around `UsbKeyboardWriter::write_report` and `BleHidServer::write_report`:

- USB: assert exact buffer bytes produced for each `Report` variant by mocking `HidWriter::write`.
- BLE: assert exact buffer bytes handed to `notify` for each variant by stubbing `Characteristic::notify`.

**Verify.** `sh scripts/test_all.sh` green; `(cd rmk-types && cargo test --features rmk_protocol)` green.

### Phase 1 — `ReportSink` extraction (zero behaviour change)

- Add `ReportSink` in `hid.rs` (§1). No blanket impl.
- `UsbKeyboardWriter` → `impl ReportSink` (four per-variant `send_*`). Existing `impl HidWriterTrait<Report>` body shrinks to `self.send_report(&r).await`. The 52-line match moves into `ReportSink::send_report`'s trait default.
- `BleHidServer` → same; 48-line match becomes one line.
- **Nothing else changes.** No routing, no new crates, no feature-gate edits. `RunnableHidWriter`, `DummyWriter`, `UsbHostReaderWriter`, `host_reader_writer` all stay on their current gates; Phase 4 rewrites the host/vial gate mess consistently.

**Verify.** `test_all.sh` / `clippy_all.sh` / `check_all.sh` green. Phase 0 byte tests remain bit-identical. `cargo expand --features=vial,storage` on `UsbKeyboardWriter::write_report` shows only the outer body becoming a one-line dispatch through `send_report`.

### Phase 2 — Router functions replace `DummyWriter` + `RunnableHidWriter`

- Add `rmk/src/report_router.rs` with `HidOutputPolicy`, `TransportStatus`, and the three free functions (§2). No router struct, no `NoTransport`.
- `impl TransportStatus for UsbKeyboardWriter<'_, 'd, D>`: `USB_CONFIGURED.contains_value()`.
- `impl TransportStatus for BleHidServer<'_, '_, '_, P>`: reports connected per the underlying `GattConnection`.
- Move the `USB_REMOTE_WAKEUP` retry (`hid.rs:71-88`) into a private `write_with_wakeup_retry` helper in `usb/mod.rs`; each `send_*` on `UsbKeyboardWriter` calls it. BLE-only builds don't compile it.
- `run_keyboard`: replace `keyboard_writer: W` with `router_fut: impl Future<Output = ()>`. Pure generic removal; `communication_fut` already takes this shape. Call sites:
  - USB-only (`lib.rs:261-274`) → `run_router_single(&mut UsbKeyboardWriter::new(...))`.
  - USB+BLE dual (`ble/mod.rs:382-397, 437-449`) → `run_router_dual(&mut usb_writer, &mut ble_hid_server, policy)`.
  - `_no_usb` BLE-only → `run_router_single(&mut ble_hid_server)`.
- Initial policy hard-wired from `get_connection_type()` (`rmk/src/state.rs`): `Usb → PreferUsb`, `Ble → PreferBle`.
- Replace `run_dummy_keyboard` (`ble/mod.rs:920-937`) with `run_dummy_router` (same signature) doing `select(storage.run(), run_report_drain())`. Both call sites updated.
- Delete `RunnableHidWriter`, `DummyWriter`, and the now-unused `impl HidWriterTrait<Report>` blocks on both writers. After Phase 2, `HidWriterTrait` survives only for `ReportType = ViaReport` (Vial).
- **Advertising-timeout wake sites at `ble/mod.rs:425, 481, 527` stay unchanged.** No `rmk-config` change in Phase 2.

**Verify.**
- New `tests/report_router.rs` (mock `embassy-time` at `rmk/Cargo.toml:166-169`):
  1. Single-sink happy path — 20 reports in order; none dropped.
  2. Dual `PreferUsb` — USB ready flips `true→false→true`; every report lands on exactly one sink in order; none duplicated.
  3. Dual both-unready — 5 reports enqueued; all dropped; channel never overflows; producer `send().await` never blocks.
  4. Drain — 20 reports received-and-discarded; producer never blocks; `CONNECTION_STATE` flips to `Connected`.
- `test_all.sh` green across all BLE feature combinations.
- Hardware smoke on one BLE board: connect / pull dongle / reconnect; no stuck modifiers.
- **Example migration.** `run_keyboard` drops `W: RunnableHidWriter` / `keyboard_writer` and gains `router_fut: impl Future<Output = ()>`. `#[rmk_keyboard]`-driven `examples/use_config/*` pick up the new signature via `rmk-macro` regeneration — no user action. Hand-written `examples/use_rust/*/src/main.rs` patch their `run_keyboard(...)` call site to construct the router future inline (`run_router_single(&mut UsbKeyboardWriter::new(...))` or `run_router_dual(&mut usb, &mut ble, policy)`). A "Upgrading from pre-P2" note lands in `docs/` alongside this phase; since `run_keyboard` is the entry point third-party users write against, the note is required, not optional.

### Phase 3 — `HostOps` + `LOCK_STATE` (keystone refactor; zero new features)

- Create `rmk/src/host/ops.rs` with `HostOps<'a>` (§3). One file; four feature-gated `impl` blocks.
- Create `rmk/src/host/lock.rs` with `LockState` + `pub(crate) static LOCK_STATE` (§4). Compiled under `feature = "host"`; wrappers gated on `host_security`. Vial's existing lock path stays unchanged in Phase 3 — Phase 7 does the unification.
- Rewrite every `ViaCommand` arm in `process_via_packet` (`via/mod.rs:80-308`) to call `HostOps`. Each arm becomes 1-3 lines.
- Same treatment for `process_vial` (`via/vial.rs`).
- Every `FLASH_CHANNEL.send(...)` currently in a Vial arm moves behind `HostOps::persist_*`. The one `try_send` in `DynamicKeymapSetBuffer` (`via/mod.rs:277-284`) stays inline (cannot yield mid-loop).
- Preserve the existing `publish_event` chain — `HostOps` setters forward to `KeyMap`, which already publishes `LayerChangeEvent` / `LedIndicatorEvent` / … from inside its own methods. No new `publish_event` sites inside `HostOps`.
- `boot::jump_to_bootloader()` → `HostOps::jump_to_bootloader()` (identical body, single call site).
- Implement `Runnable for VialService<RW>` by promoting `pub(crate) async fn run(&mut self)` to `async fn run(&mut self) -> !` (existing loop already does not return).
- `peripheral_status` intentionally **not** added (§3 note).

**Verify.** `cargo nextest run --no-default-features --features=split,vial,storage,async_matrix,_ble` green. New `HostOps` unit tests cover each sync method and each `persist_*` via a mock `KeyMap`. Hardware smoke: Vial GUI round-trip on USB and BLE examples (keymap edit, macro record, combo edit, bootloader jump).

### Phase 4 — `rmk_protocol` service (in-memory validation first)

- Add `rmk/src/host/rmk_protocol/` skeleton (§6), including `dispatch.rs`.
- Implement `define_rmk_dispatch!` in `dispatch.rs` (~150 LoC). Mirror `postcard_rpc::define_dispatch!` (`server/dispatch_macro.rs:258`) — same `endpoints`/`topics_in`/`topics_out` table syntax, same `sizer` const-key-length machinery, same standard-ICD arms. Differences: drop `tx_impl`/`spawn_impl`/`spawn_fn` parameters; emit `pub struct RmkDispatcher<Tx, const N: usize> { context, device_map, _tx: PhantomData<Tx> }` and `impl<Tx: WireTx> Dispatch for RmkDispatcher<Tx, N> { type Tx = Tx; … }`; only emit `async` and `blocking` ep_arm / tp_arm bodies (no `spawn` flavor).
- Invoke `define_rmk_dispatch!` once at module scope in `dispatch.rs`, against `rmk_types::protocol::rmk::ENDPOINT_LIST`. Result: a single, transport-agnostic `RmkDispatcher<Tx>` type alias.
- Implement `run_rmk_protocol_task<Rx, Tx>(keymap, rx, tx)` that builds `Server<Tx, Rx, &'static mut [u8; PROTOCOL_RPC_SERVER_BUF_SIZE], RmkDispatcher<Tx>>`; obtains `Sender<Tx>` clones via `Server::sender()`; drives the server loop and all republishers via `join`. Same function body services USB bulk, BLE GATT, and `postcard-rpc/test-utils` test channels — `Tx` is the only thing that differs at each call site.
- Wire a minimal endpoint set first: `sys/version`, `sys/caps`, `sys/lock_status`, `sys/bootloader`, `status/layer/get`, `keymap/get`, `keymap/set`. Each handler is 1-3 lines calling `HostOps`.
- **Handler error mapping.** Handlers take `&RpcContext` (holds `HostOps<'a>`). Sync reads returning `T` / `Option<T>` map 1:1 to the endpoint response (`None` → `RmkError::InvalidParameter` where endpoint is `RmkResult`-typed). Sync/async setters returning `RmkResult` are already the wire shape — no mapping code. `()`-returning `persist_*` wraps as `Ok(())` for `RmkResult` endpoints. Single error type on the wire: `RmkError` (variants: `InvalidParameter`, `BadState`, `InternalError`).
- Implement 5 per-pair republisher structs (Phase 4 rows of the §6 topic table). Each holds `Sender<Tx>` clone + its `Subscriber`, exposes `async fn run(&mut self) -> !`, emits via `Sender::publish::<M>` with a per-struct rolling `u32` seq counter. Most mappings are the identity (payload types re-exported from `rmk-types`); non-identity is 1-3 lines inline in the run loop. No generic `TopicRepublisher<E, M, Conv>` — per-pair structs keep the mapping visible.
- Refactor `host/mod.rs` into `run_vial_task` + `run_rmk_protocol_task` (§run_*_task signatures). Delete the `todo!()` function.
- **`run_keyboard` drops `Rw`/`vial_config`, gains `host_fut: impl Future<Output = ()>`.** Vial-only callers pass `run_vial_task(...)`; rmk_protocol-only callers pass `run_rmk_protocol_task(...)`; coexistence (Phase 7) passes `async { join(run_vial_task(...), run_rmk_protocol_task(...)).await; }`.
- **Gate regating.** `host_reader_writer` allocation at `ble/mod.rs:241-242` + consumers at `388, 443` and `lib.rs:267` move `host → vial` (they use `ViaReport`). `UsbHostReaderWriter` alias at `host/mod.rs:6` moves to `vial`. A `host + rmk_protocol + not-vial` build stops allocating the unused Vial HID interface.
- **Introduce the `rmk-config` feature-conditional merge layer** (§6 bumps); register the 5 Phase 4 entries. Emit `static_assertions::const_assert!` alongside `constants.rs` so under-sized user overrides fail at build time.
- Add `postcard-rpc = { features = ["test-utils"], optional = true }` to `[dev-dependencies]` so in-memory integration tests under `#[cfg(test)]` can drive the dispatcher without affecting firmware builds.
- Mutual-exclusion `compile_error!` remains until Phase 7.

**Verify.**
- `cargo check -p rmk --no-default-features --features="storage,rmk_protocol"` builds.
- `cargo nextest run -p rmk --no-default-features --features="storage,rmk_protocol"` exercises the Phase 4 endpoint subset and `LayerChangeTopic` emission via `publish_event(LayerChangeEvent::new(3))` received on the in-memory client (test driver uses the dev-dep `postcard-rpc/test-utils`).
- `(cd rmk-types && cargo test --features rmk_protocol)` snapshot tests stay green.
- `const_assert!`s for bumped `subs` counts fire when a user override is below the required minimum.
- **Example migration.** `run_keyboard` drops `Rw: HidReaderTrait<ViaReport> + HidWriterTrait<ViaReport>` / `vial_config` and gains `host_fut: impl Future<Output = ()>`. `use_config/` callers regenerate. `use_rust/` Vial callers pass `run_vial_task(keymap, rw, cfg)`; `rmk_protocol` callers pass `run_rmk_protocol_task(keymap, rx, tx)`. The "Upgrading" note extends with the P4 signature and — looking ahead — the P7 coexistence shape `async { join(run_vial_task(...), run_rmk_protocol_task(...)).await; }`.

### Phase 5 — USB bulk transport

- Add `add_usb_bulk_endpoints!` in `usb/mod.rs` (vendor class `0xFF`; bulk IN + bulk OUT; FS max packet 64).
- Add `rmk/src/host/rmk_protocol/transport/usb_bulk.rs`:
  - `UsbBulkTx: WireTx` — `&'static Mutex<RawMutex, EndpointIn<'static, D>>`; `WireTx::send` is `&self`, so multiple cloned `Sender<Tx>` share the endpoint.
  - `UsbBulkRx: WireRx` — owns `EndpointOut<'static, D>` exclusively.
  - COBS encode on `send` / `send_raw` (emit sentinel after each frame); COBS decode on `receive` (accumulate until sentinel). Buffer sizing from `rmk-types` protocol `MaxSize` bounds (`protocol_max_bulk_size`, `protocol_macro_chunk_size`); 256-512 B TX, 256 B RX.
- Wire the full base endpoint list (28 base endpoints) through `define_rmk_dispatch!`. Bulk-group endpoints gated on `bulk_transfer`. The macro invocation in `dispatch.rs` grows; `run_rmk_protocol_task<Rx, Tx>` body is unchanged — `UsbBulkTx` / `UsbBulkRx` plug in as the concrete `Tx` / `Rx` at the call site.

**Verify.** `cargo check -p rmk --no-default-features --features="rmk_protocol,bulk_transfer,storage"` builds. Hardware smoke on rp2040 or stm32: a `postcard-rpc` host tool round-trips `GetKeymapBulk` / `SetKeymapBulk` and edits stick through reset.

### Phase 6 — BLE `rmk_protocol` transport

- Add `rmk/src/ble/host_service/rmk_protocol.rs` (RX/TX GATT chars + `HOST_RPC_INPUT_CHANNEL`).
- Add `rmk/src/host/rmk_protocol/transport/ble_gatt.rs`:
  - `BleRpcTx: WireTx` — notifier + `&'static Mutex<…>`; derive `Clone`.
  - `BleRpcRx: WireRx` — drains `HOST_RPC_INPUT_CHANNEL`; accumulates chunks until COBS sentinel.
- Re-gate today's `#[cfg(feature = "host")]` Server variant to `#[cfg(all(_ble, vial, not(rmk_protocol)))]` (body unchanged). Add a new `#[cfg(all(_ble, rmk_protocol, not(vial)))]` Server variant composing HID + battery + device-info + RPC. Three variants total after Phase 6; Phase 7 adds the fourth.
- Publish BLE-conditional topics (`BatteryStatusTopic`, `BleStatusChangeTopic`) via `TopicRepublisher`.
- Register the two BLE-gated `subs` bumps (§6 table).
- Default chunk 20 (ATT_MTU 23 − 3); negotiate larger when `trouble-host` exposes the connection MTU.

**Verify.** `examples/use_rust/nrf52840_ble` builds with `rmk_protocol,_ble,storage`. Manual BLE client round-trips `GetKeyAction`; subscribes `LayerChangeTopic` and observes push on layer switch; subscribes `BleStatusChangeTopic` and observes push when another profile connects.

### Phase 7 — Vial + `rmk_protocol` coexistence

- Rewire `vial_lock::VialLock` to delegate to `LOCK_STATE` (created in Phase 3). Both views now share one static. No behaviour change in either single-feature build.
- Compose USB Vial HID raw + vendor bulk in one device. `embassy-usb::Builder`'s sequential interface allocation handles numbering; verified by `lsusb -v` diff on both single-feature and combo builds.
- Add the fourth `#[gatt_server]` variant under `cfg(all(_ble, vial, rmk_protocol))` (HID + Vial + RPC + battery + device-info).
- In `run_keyboard`'s coexistence path, `host_fut` is `async { join(run_vial_task(...), run_rmk_protocol_task(...)).await; }`. Each service owns its own transport; they share only `HostOps`/`LOCK_STATE` via `KeyMap`.
- **Remove `compile_error!` at `lib.rs:21-22`.**
- CI: `--features=vial,rmk_protocol,storage` and `--features=vial,rmk_protocol,storage,_ble`.

**Verify.**
- Full `test_all.sh` matrix green including the two new combos.
- Hardware: the same firmware answers a Vial GUI client and a `postcard-rpc` client; keymap edit via one is visible to the other after round-trip (both read from `KeyMap`, both persist via `FLASH_CHANNEL`).
- Locked state: both Vial `Unlock` and `rmk_protocol::UnlockRequest` operate on the shared `LOCK_STATE`; reads while locked succeed; writes return `RmkResult::Err(BadState)` on both surfaces.

### Deferred beyond this plan

`SessionManager` replacing the two connection atomics; rewriting split as postcard-rpc; spawn-flavor handler support in `define_rmk_dispatch!` (would require adding a `Spawner` trait abstraction; no current endpoint needs it); WinUSB / MS OS 2.0 descriptors for Windows host support.

---

## Rejected Ideas

| Idea | Why rejected |
|---|---|
| Unified `ControlEndpoint` / `ByteFramedLink` for Vial + `rmk_protocol` | `postcard_rpc::WireTx::send(&self, …)` vs `WireRx::receive(&mut self, …)` asymmetry is load-bearing. A unified trait either forces `&mut` on send (breaks topic publishing) or hides the invariant (impl authors get it wrong). |
| `PubSubChannel<Report>` for multi-sink fan-out | `#[event]`-generated PubSub drops oldest on full — a dropped key-release = stuck key. MPSC `Channel<Report, 16>` + router keeps ordering on the happy path, drops only when no sink is ready. |
| `HostOps` every method `async` | Nearly all ops are sync `RefCell` borrows; only `FLASH_CHANNEL.send` is async. Making everything async makes callers `.await` on atomic reads and obscures which calls yield. Split into sync + `persist_*` async. |
| `HostOps` as trait + single impl | Vtable per op with no second impl in sight. Concrete struct matches RMK's generics-first culture; trait-wrap later is non-breaking. |
| `HostOps` submodule tree on day 1 | ~700 lines of thin wrappers don't justify 4-5 files. One file with feature-gated `impl` blocks. |
| `HostOps::peripheral_status` | State lives in `PeripheralManager`, not `KeyMap`. No Phase 4-6 endpoint consumes it; listing a method with no consumer invites stub code. Out of scope. |
| Generic `ReportRouter<U, B>` + `NoTransport` stub | Three shapes need genuinely different ergonomics; `NoTransport::send_*` doing `core::future::pending()` would stall the keyboard if ever called. Three free fns match the shapes exactly — no stub, no coherence tax, no new `Router: Runnable` bound on `run_keyboard`. |
| Router blocking on `wait_ready()` | Fills the channel → stalls producer `send().await` → suspends matrix scanning → breaks profile-switch combos. Current drop-on-no-sink semantic is load-bearing. |
| USB wakeup retry in the router | `USB_REMOTE_WAKEUP` is USB-only. Keep inside `UsbKeyboardWriter`; router sees plain `Result<_, HidError>`. |
| Rewriting the 3 adv-timeout wake sites to subscribe to `KeyboardEvent` | Each drains one wake report (no host attached anyway). Net benefit zero; cost a `subs` bump. |
| Permanent `vial` / `rmk_protocol` mutual exclusion | Separate endpoints in hardware; a compile-time choice is artificial. |
| Hand-rolled match over `REQ_KEY` instead of any macro | Parallel path to the ICD; loses const-time key-size optimization. `define_rmk_dispatch!` keeps both. |
| `postcard_rpc::define_dispatch!` adopted directly | Bakes `tx_impl`/`spawn_impl`/`spawn_fn` concretely (`server/dispatch_macro.rs:163-164, 441, 86`), so a generic `run_rmk_protocol_task<Rx, Tx>` cannot host one dispatcher across test channels + USB bulk + BLE GATT. `define_rmk_dispatch!` (custom, ~150 LoC) emits a generic `RmkDispatcher<Tx, N>` instead — the `Sender<Tx>` API is already generic, and dropping spawn-flavor support removes the only remaining transport coupling. |
| Per-transport `define_dispatch!` modules (one dispatcher per transport) | Three near-identical macro invocations duplicating the endpoint table; coexistence (Phase 7) doubles each. The wrapper macro is the same effort and yields one dispatcher type. |
| Caller-supplied `Server<Tx, Rx, Buf, D>` (push macro invocation up to call sites) | Leaks postcard-rpc internals (Server construction, dispatcher type) into every transport setup; republisher set still needs orchestration inside `run_rmk_protocol_task`. The `define_rmk_dispatch!` approach keeps the call site to `(rx, tx)` only. |
| Applying `subs` bumps *after* the user override | A user who writes `subs = 5` expects 5, not `5 + feature-adjusted`. Apply before; `const_assert!` on under-size. |
| One `HidReaderTrait<ViaReport>`-bounded `run_host_communicate_task` | Cannot accommodate postcard-rpc's `WireRx`/`WireTx`. Two disjoint fns gated by feature is the minimal fix. |
| Generic `TopicRepublisher<E, M, Conv>` with a conversion closure | 7 republishers total; most mappings are identity. A closure obscures the payload; per-pair structs keep it visible. |

---

## Critical Files

### Modified

- `rmk/src/hid.rs` — +`ReportSink` (P1); −`DummyWriter`, −`RunnableHidWriter` (P2). Keep `Report`, `HidError`, `HidReaderTrait`, `HidWriterTrait` (still used by Vial).
- `rmk/src/usb/mod.rs` — `UsbKeyboardWriter` → `impl ReportSink` (P1); `impl TransportStatus` + internalised `USB_REMOTE_WAKEUP` retry helper (P2); `add_usb_bulk_endpoints!` macro (P5).
- `rmk/src/ble/ble_server.rs` — `BleHidServer` → `impl ReportSink` (P1); `impl TransportStatus` (P2); re-gate existing `host` Server variant to `vial`-only + add `rmk_protocol`-only variant (P6); add coexistence variant (P7).
- `rmk/src/ble/mod.rs` — `run_dummy_keyboard` → `run_dummy_router` (P2); swap `run_writer` arg at every `run_keyboard` call site for the matching `run_router_*` future (P2); re-gate `host_reader_writer` allocation + consumers `host → vial` (P4). Adv-timeout wake sites (425, 481, 527) unchanged.
- `rmk/src/host/mod.rs` — replace `run_host_communicate_task` with `run_vial_task` + `run_rmk_protocol_task`; re-gate `UsbHostReaderWriter` alias `host → vial` (P4).
- `rmk/src/host/via/mod.rs` — rewrite `process_via_packet` arms to call `HostOps` (P3).
- `rmk/src/host/via/vial.rs` — rewrite `process_vial` to call `HostOps` (P3); rewire `VialLock` onto `LOCK_STATE` (P7).
- `rmk/src/lib.rs` — `run_keyboard`: replace `W: RunnableHidWriter`/`keyboard_writer` with `router_fut: impl Future<Output = ()>` (P2); drop `Rw`/`vial_config`, add `host_fut: impl Future<Output = ()>` (P4). Remove `compile_error!` at lines 21-22 (P7).
- `rmk/Cargo.toml` — `postcard-rpc = { features = ["test-utils"], optional = true }` under `[dev-dependencies]` (P4).
- `rmk-config/src/lib.rs` — `protocol_rpc_server_buf_size` (P4), `protocol_rpc_chunk_size`, `protocol_rpc_channel_size` (P5/P6); feature-conditional merge layer in `emit_event_constants` + `const_assert!` emission (P4).

### Added

- `rmk/src/report_router.rs` — `HidOutputPolicy`, `TransportStatus`, three free functions (P2).
- `rmk/src/host/ops.rs` — `HostOps<'a>` with feature-gated `impl` blocks (P3).
- `rmk/src/host/lock.rs` — `LockState` + `pub(crate) static LOCK_STATE` (P3).
- `rmk/src/host/rmk_protocol/{mod.rs, dispatch.rs, handlers.rs, topics.rs, context.rs, transport/{usb_bulk.rs, ble_gatt.rs}}` (P4-P6). `dispatch.rs` ships `define_rmk_dispatch!` (custom transport-generic wrapper around `postcard_rpc::define_dispatch!`'s shape) and the single invocation that emits `RmkDispatcher<Tx, N>`.
- `rmk/src/ble/host_service/rmk_protocol.rs` — BLE RPC GATT glue + `HOST_RPC_INPUT_CHANNEL` (P6).

### Untouched (reused, do not reinvent)

- `rmk/src/input_device/mod.rs` — `Runnable`, `run_all!`. (`join_all!` lives in `rmk/src/helper_macro.rs:5-23`.)
- `rmk/src/processor.rs` — `Processor` (input-only; no `OutputProcessor`).
- `rmk/src/event/` — `publish_event`, `publish_event_async`, `#[event]`, `SubscribableEvent`.
- `rmk/src/channel.rs` — `KEYBOARD_REPORT_CHANNEL`, `FLASH_CHANNEL`, `LED_SIGNAL`.
- `rmk/src/split/` — `SplitReader`, `SplitWriter`, `SplitMessage`, `PeripheralManager`.
- `rmk/src/ble/host_service/mod.rs` — `HOST_GUI_INPUT_CHANNEL` (Vial only; don't reuse for RPC).
- `rmk-types/src/protocol/rmk/` — ICD stays frozen.
- `postcard_rpc::server::{Server, WireRx, WireTx, Sender, Dispatch}` — used directly. `postcard_rpc::define_dispatch!` is **not** invoked (transport-coupling forces a custom wrapper, see §6); RMK's `define_rmk_dispatch!` provides the equivalent surface against the same `Dispatch` trait.
- `rmk/src/split/serial/` COBS — unchanged; `rmk_protocol`'s COBS frame accumulator is a separate inline implementation.

---

## Verification

Every phase must pass:

```bash
# Primary dev loop from rmk/
cargo nextest run --no-default-features --features=split,vial,storage,async_matrix,_ble

# Full feature matrix (~40s clean)
sh scripts/test_all.sh
sh scripts/clippy_all.sh
sh scripts/check_all.sh

# Wire-format stability — must stay green forever
(cd rmk-types && cargo test --features rmk_protocol)
(cd rmk-types && cargo test --features "rmk_protocol,bulk")
(cd rmk-types && cargo test --features "rmk_protocol,_ble")
(cd rmk-types && cargo test --features "rmk_protocol,_ble,split")
```

`rmk_protocol`-only firmware (Phase 4+):

```bash
cd rmk
cargo nextest run --no-default-features --features="rmk_protocol,storage"
cargo nextest run --no-default-features --features="rmk_protocol,storage,_ble"
cargo nextest run --no-default-features --features="rmk_protocol,storage,_ble,split"
```

Coexistence (Phase 7+):

```bash
cd rmk
cargo nextest run --no-default-features --features="vial,rmk_protocol,storage"
cargo nextest run --no-default-features --features="vial,rmk_protocol,storage,_ble"
cargo nextest run --no-default-features --features="vial,rmk_protocol,bulk_transfer,storage,_ble"
```

Per-phase gates:

- **P0**: baseline matrix green; byte-level anchors in place.
- **P1**: byte tests unchanged; `cargo expand` shows each `write_report` body reduced to a one-line dispatch; `rmk-types` snapshots unchanged.
- **P2**: `tests/report_router.rs` four scenarios green; hardware reconnect smoke, no stuck keys.
- **P3**: `HostOps` unit tests cover every sync + every `persist_*`; Vial GUI round-trip unchanged on USB + BLE.
- **P4**: in-memory transport exercises the Phase 4 endpoint subset + `LayerChangeTopic`; `const_assert!`s fire when a user override is under-sized; snapshots green.
- **P5**: host tool round-trips `GetKeymapBulk` / `SetKeymapBulk` on rp2040 or stm32 with `bulk_transfer`; USB descriptor dump matches the vendor interface spec.
- **P6**: nRF52840 BLE client round-trips `GetKeyAction`; receives `LayerChangeTopic`, `BleStatusChangeTopic`.
- **P7**: cross-client keymap-edit visibility (Vial ↔ `rmk_protocol`); locked-state behaviour (reads OK, writes blocked) identical on both surfaces; `LOCK_STATE` is the single source of truth.

Binary size gate: `cargo size --release` on `examples/use_rust/nrf52840_ble` per phase; watch for regressions. Phase 2 is expected net-zero-to-negative (`DummyWriter` + `RunnableHidWriter` removal pays for the router code).
