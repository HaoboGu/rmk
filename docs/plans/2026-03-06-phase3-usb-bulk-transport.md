# Phase 3: USB Raw Bulk Transport — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement a raw vendor-class USB bulk endpoint transport so `ProtocolService` (Phase 2) can communicate with a host PC over USB.

**Architecture:** Create a vendor-class (0xFF) USB function with one bulk IN and one bulk OUT endpoint alongside the existing HID composite device. Implement postcard-rpc `WireTx`/`WireRx` traits for these endpoints. Wire the transport into `ProtocolService` through the existing `run_keyboard` → `run_host_communicate_task` call chain. Add MS OS 2.0 descriptors for automatic WinUSB driver binding on Windows.

**Tech Stack:** `embassy-usb 0.5` (USB device stack), `postcard-rpc 0.12` (WireTx/WireRx traits, Sender, VarHeader), `embassy-sync 0.7` (Mutex for interior mutability)

**Prerequisites completed:**
- Phase 1: ICD types defined in `rmk-types/src/protocol/rmk/` ✅
- Phase 2.1: Feature gates (`rmk_protocol`, `host_security`) ✅
- Phase 2.2–2.3: `ProtocolService` struct and dispatch loop in `rmk/src/host/protocol/mod.rs` ✅ (generic over `Tx: WireTx, Rx: WireRx`)
- Phase 2.5: `FlashOperationMessage::HostMessage` rename ✅

---

### Task 1: Increase USB descriptor buffer sizes

**Files:**
- Modify: `rmk/src/usb/mod.rs:133-135`

**Context:** Current `BOS_DESC` and `MSOS_DESC` buffers are 16 bytes each — too small for MS OS 2.0 descriptors. The MS OS 2.0 platform capability (in BOS) needs ~28 bytes, and the MS OS 2.0 descriptor set (compatible ID + registry property with GUID) needs ~160+ bytes. `CONFIG_DESC` also needs more space for the additional vendor interface descriptor.

**Step 1: Increase buffer sizes**

In `rmk/src/usb/mod.rs`, change lines 133-135:

```rust
// Before:
static BOS_DESC: StaticCell<[u8; 16]> = StaticCell::new();
static MSOS_DESC: StaticCell<[u8; 16]> = StaticCell::new();

// After:
static BOS_DESC: StaticCell<[u8; 64]> = StaticCell::new();
static MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
```

Also update the `Builder::new()` call (lines 139-146) to use the new sizes:

```rust
// Before:
&mut BOS_DESC.init([0; 16])[..],
&mut MSOS_DESC.init([0; 16])[..],

// After:
&mut BOS_DESC.init([0; 64])[..],
&mut MSOS_DESC.init([0; 256])[..],
```

**Step 2: Verify compilation**

Run: `cd rmk && cargo check --no-default-features --features=vial,storage`
Expected: compiles OK (buffer sizes are backward-compatible)

**Step 3: Commit**

```bash
git add rmk/src/usb/mod.rs
git commit -m "feat(usb): increase BOS/MSOS descriptor buffers for MS OS 2.0 support"
```

---

### Task 2: Add vendor bulk endpoint creation to USB module

**Files:**
- Modify: `rmk/src/usb/mod.rs`

**Context:** We need a function that adds MS OS 2.0 descriptors for WinUSB automatic binding, creates a vendor-class interface (class 0xFF) with bulk IN and OUT endpoints, and returns the endpoint handles. This follows the same pattern as `add_usb_reader_writer!` but for vendor bulk instead of HID.

**Step 1: Add MSOS + vendor bulk macro**

Add after the `add_usb_reader_writer!` macro (around line 227) in `rmk/src/usb/mod.rs`:

```rust
#[cfg(feature = "rmk_protocol")]
macro_rules! add_usb_vendor_bulk {
    ($usb_builder:expr) => {{
        use embassy_usb::msos::{self, windows_version};

        $usb_builder.msos_descriptor(windows_version::WIN8_1, 0);
        $usb_builder.msos_feature(msos::CompatibleIdFeatureDescriptor::new("WINUSB", ""));
        $usb_builder.msos_feature(msos::RegistryPropertyFeatureDescriptor::new(
            "DeviceInterfaceGUIDs",
            msos::PropertyData::RegMultiSz(&["{CDB53450-4E39-4F7E-9F61-4DEF2E5C1C3B}"]),
        ));

        let mut function = $usb_builder.function(0xFF, 0, 0);
        let mut interface = function.interface();
        let mut alt = interface.alt_setting(0xFF, 0, 0, None);
        let ep_out = alt.endpoint_bulk_out(64);
        let ep_in = alt.endpoint_bulk_in(64);
        drop(function);

        (ep_out, ep_in)
    }};
}

#[cfg(feature = "rmk_protocol")]
pub(crate) use add_usb_vendor_bulk;
```

**Step 2: Verify compilation**

Run: `cd rmk && cargo check --no-default-features --features=rmk_protocol,storage`
Expected: compiles (macro is defined but not yet called)

**Step 3: Commit**

```bash
git add rmk/src/usb/mod.rs
git commit -m "feat(usb): add vendor bulk endpoint creation macro with WinUSB MSOS descriptors"
```

---

### Task 3: Implement `WireTx` for USB bulk IN

**Files:**
- Create: `rmk/src/host/protocol/transport.rs`

**Context:** postcard-rpc's `WireTx` trait requires `send()`, `send_raw()`, `send_log_str()`, `send_log_fmt()` methods. `send()` takes `&self`, so we need interior mutability (use `Mutex`). The implementation serializes VarHeader + postcard payload into a buffer, then writes to the USB bulk IN endpoint in chunks of 64 bytes (USB Full Speed max packet size). A short final packet (or ZLP for exact multiples of 64) terminates the USB transfer.

Reference: `postcard-rpc/src/server/impls/embassy_usb_v0_5.rs` lines 246-545.

**Step 1: Create the transport module file**

Create `rmk/src/host/protocol/transport.rs`:

```rust
use core::fmt::Arguments;

use embassy_sync::mutex::Mutex;
use embassy_usb::driver::{Driver, Endpoint, EndpointError, EndpointIn, EndpointOut};
use postcard_rpc::header::{VarHeader, VarKey, VarKeyKind, VarSeq};
use postcard_rpc::server::{WireRx, WireRxErrorKind, WireTx, WireTxErrorKind};
use postcard_rpc::standard_icd::LoggingTopic;
use postcard_rpc::Topic;
use serde::Serialize;

use crate::RawMutex;

const TX_BUF_SIZE: usize = 256;
const MAX_PACKET_SIZE: usize = 64;

pub(crate) struct UsbBulkTx<D: Driver<'static>> {
    inner: Mutex<RawMutex, UsbBulkTxInner<D>>,
}

struct UsbBulkTxInner<D: Driver<'static>> {
    ep_in: D::EndpointIn,
    tx_buf: [u8; TX_BUF_SIZE],
}

impl<D: Driver<'static>> UsbBulkTx<D> {
    pub(crate) fn new(ep_in: D::EndpointIn) -> Self {
        Self {
            inner: Mutex::new(UsbBulkTxInner {
                ep_in,
                tx_buf: [0u8; TX_BUF_SIZE],
            }),
        }
    }
}

async fn send_all<D: Driver<'static>>(
    ep_in: &mut D::EndpointIn,
    data: &[u8],
) -> Result<(), WireTxErrorKind> {
    if data.is_empty() {
        return Ok(());
    }

    for chunk in data.chunks(MAX_PACKET_SIZE) {
        ep_in
            .write(chunk)
            .await
            .map_err(|_| WireTxErrorKind::ConnectionClosed)?;
    }

    if data.len() % MAX_PACKET_SIZE == 0 {
        ep_in
            .write(&[])
            .await
            .map_err(|_| WireTxErrorKind::ConnectionClosed)?;
    }

    Ok(())
}

impl<D: Driver<'static>> WireTx for UsbBulkTx<D> {
    type Error = WireTxErrorKind;

    async fn wait_connection(&self) {
        let mut inner = self.inner.lock().await;
        inner.ep_in.wait_enabled().await;
    }

    async fn send<T: Serialize + ?Sized>(
        &self,
        hdr: VarHeader,
        msg: &T,
    ) -> Result<(), Self::Error> {
        let mut inner = self.inner.lock().await;
        let UsbBulkTxInner { ep_in, tx_buf } = &mut *inner;

        let (hdr_used, remain) = hdr.write_to_slice(tx_buf).ok_or(WireTxErrorKind::Other)?;
        let body_used =
            postcard::to_slice(msg, remain).map_err(|_| WireTxErrorKind::Other)?;
        let total = hdr_used.len() + body_used.len();

        send_all::<D>(ep_in, &tx_buf[..total]).await
    }

    async fn send_raw(&self, buf: &[u8]) -> Result<(), Self::Error> {
        let mut inner = self.inner.lock().await;
        send_all::<D>(&mut inner.ep_in, buf).await
    }

    async fn send_log_str(&self, kkind: VarKeyKind, s: &str) -> Result<(), Self::Error> {
        let mut inner = self.inner.lock().await;
        let UsbBulkTxInner { ep_in, tx_buf } = &mut *inner;

        let key = match kkind {
            VarKeyKind::Key1 => VarKey::Key1(LoggingTopic::TOPIC_KEY1),
            VarKeyKind::Key2 => VarKey::Key2(LoggingTopic::TOPIC_KEY2),
            VarKeyKind::Key4 => VarKey::Key4(LoggingTopic::TOPIC_KEY4),
            VarKeyKind::Key8 => VarKey::Key8(LoggingTopic::TOPIC_KEY),
        };
        let hdr = VarHeader {
            key,
            seq_no: VarSeq::Seq1(0),
        };
        let (hdr_used, remain) = hdr.write_to_slice(tx_buf).ok_or(WireTxErrorKind::Other)?;
        let body_used =
            postcard::to_slice::<str>(s, remain).map_err(|_| WireTxErrorKind::Other)?;
        let total = hdr_used.len() + body_used.len();

        send_all::<D>(ep_in, &tx_buf[..total]).await
    }

    async fn send_log_fmt<'a>(
        &self,
        kkind: VarKeyKind,
        _a: Arguments<'a>,
    ) -> Result<(), Self::Error> {
        // Simplified: delegate to send_log_str with a placeholder
        self.send_log_str(kkind, "<fmt>").await
    }
}
```

**Step 2: Register transport module**

At the top of `rmk/src/host/protocol/mod.rs`, add:

```rust
pub(crate) mod transport;
```

**Step 3: Verify compilation**

Run: `cd rmk && cargo check --no-default-features --features=rmk_protocol,storage`
Expected: compiles (WireTx impl)

**Step 4: Commit**

```bash
git add rmk/src/host/protocol/transport.rs rmk/src/host/protocol/mod.rs
git commit -m "feat(protocol): implement WireTx for USB bulk IN endpoint"
```

---

### Task 4: Implement `WireRx` for USB bulk OUT

**Files:**
- Modify: `rmk/src/host/protocol/transport.rs`

**Context:** `WireRx::receive()` reads USB bulk OUT packets into a buffer. USB bulk transfers end when a packet smaller than `max_packet_size` (64) is received. We accumulate reads until we see a short packet, then return the filled portion of the buffer.

Reference: `postcard-rpc/src/server/impls/embassy_usb_v0_5.rs` lines 617-666.

**Step 1: Add WireRx implementation**

Append to `rmk/src/host/protocol/transport.rs`:

```rust
pub(crate) struct UsbBulkRx<D: Driver<'static>> {
    ep_out: D::EndpointOut,
}

impl<D: Driver<'static>> UsbBulkRx<D> {
    pub(crate) fn new(ep_out: D::EndpointOut) -> Self {
        Self { ep_out }
    }
}

impl<D: Driver<'static>> WireRx for UsbBulkRx<D> {
    type Error = WireRxErrorKind;

    async fn wait_connection(&mut self) {
        self.ep_out.wait_enabled().await;
    }

    async fn receive<'a>(&mut self, buf: &'a mut [u8]) -> Result<&'a mut [u8], Self::Error> {
        let buf_len = buf.len();
        let mut window = &mut buf[..];

        while !window.is_empty() {
            let n = match self.ep_out.read(window).await {
                Ok(n) => n,
                Err(EndpointError::BufferOverflow) => {
                    return Err(WireRxErrorKind::ReceivedMessageTooLarge);
                }
                Err(EndpointError::Disabled) => {
                    return Err(WireRxErrorKind::ConnectionClosed);
                }
            };

            let (_filled, rest) = window.split_at_mut(n);
            window = rest;

            if n != MAX_PACKET_SIZE {
                let remaining = window.len();
                let len = buf_len - remaining;
                return Ok(&mut buf[..len]);
            }
        }

        // Buffer full — drain remaining USB packets
        loop {
            match self.ep_out.read(buf).await {
                Ok(n) if n == MAX_PACKET_SIZE => continue,
                Ok(_) => return Err(WireRxErrorKind::ReceivedMessageTooLarge),
                Err(EndpointError::BufferOverflow) => {
                    return Err(WireRxErrorKind::ReceivedMessageTooLarge);
                }
                Err(EndpointError::Disabled) => {
                    return Err(WireRxErrorKind::ConnectionClosed);
                }
            }
        }
    }
}
```

**Step 2: Verify compilation**

Run: `cd rmk && cargo check --no-default-features --features=rmk_protocol,storage`
Expected: compiles (WireRx impl)

**Step 3: Commit**

```bash
git add rmk/src/host/protocol/transport.rs
git commit -m "feat(protocol): implement WireRx for USB bulk OUT endpoint"
```

---

### Task 5: Update `run_host_communicate_task` for `rmk_protocol`

**Files:**
- Modify: `rmk/src/host/mod.rs`

**Context:** The `rmk_protocol` variant of `run_host_communicate_task` currently does `pending().await`. We need it to accept WireTx/WireRx transport and create+run ProtocolService. The function still receives the (unused) `Rw` reader_writer for signature compatibility with `run_keyboard`.

**Step 1: Update the rmk_protocol variant**

In `rmk/src/host/mod.rs`, replace lines 37-52 (the `rmk_protocol` variant):

```rust
// Before:
#[cfg(all(feature = "rmk_protocol", not(feature = "vial")))]
pub(crate) async fn run_host_communicate_task<
    'a,
    Rw: HidReaderTrait<ReportType = ViaReport> + HidWriterTrait<ReportType = ViaReport>,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    _keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    _reader_writer: Rw,
) {
    // Phase 3 will create USB bulk transport and instantiate ProtocolService here.
    core::future::pending::<()>().await
}

// After:
#[cfg(all(feature = "rmk_protocol", not(feature = "vial")))]
pub(crate) async fn run_host_communicate_task<
    'a,
    Rw: HidReaderTrait<ReportType = ViaReport> + HidWriterTrait<ReportType = ViaReport>,
    Tx: postcard_rpc::server::WireTx,
    Rx: postcard_rpc::server::WireRx,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    _reader_writer: Rw,
    wire_tx: Tx,
    wire_rx: Rx,
) {
    let mut service = protocol::ProtocolService::new(keymap, wire_tx, wire_rx);
    service.run().await
}
```

**Step 2: Verify compilation**

Run: `cd rmk && cargo check --no-default-features --features=rmk_protocol,storage`
Expected: compiles (the function isn't called with concrete types yet, but the types should resolve)

**Step 3: Commit**

```bash
git add rmk/src/host/mod.rs
git commit -m "feat(protocol): wire ProtocolService into run_host_communicate_task"
```

---

### Task 6: Thread transport through `run_keyboard`

**Files:**
- Modify: `rmk/src/lib.rs:337-407` (`run_keyboard` function)

**Context:** `run_keyboard` needs to accept the `WireTx`/`WireRx` transport when `rmk_protocol` is enabled, and pass them to `run_host_communicate_task`. This follows the same `#[cfg]`-gated parameter pattern already used for `vial_config`.

**Step 1: Add generic params and function params**

In `rmk/src/lib.rs`, modify the `run_keyboard` function signature to add `rmk_protocol`-gated generics and parameters:

```rust
pub(crate) async fn run_keyboard<
    #[cfg(feature = "host")] 'a,
    R: HidReaderTrait<ReportType = LedIndicator>,
    W: RunnableHidWriter,
    #[cfg(feature = "storage")] F: AsyncNorFlash,
    #[cfg(feature = "host")] Rw: HidReaderTrait<ReportType = ViaReport> + HidWriterTrait<ReportType = ViaReport>,
    #[cfg(feature = "rmk_protocol")] Tx: postcard_rpc::server::WireTx,
    #[cfg(feature = "rmk_protocol")] Rx: postcard_rpc::server::WireRx,
    #[cfg(any(feature = "storage", feature = "host"))] const ROW: usize,
    #[cfg(any(feature = "storage", feature = "host"))] const COL: usize,
    #[cfg(any(feature = "storage", feature = "host"))] const NUM_LAYER: usize,
    #[cfg(any(feature = "storage", feature = "host"))] const NUM_ENCODER: usize,
>(
    #[cfg(feature = "storage")] storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    #[cfg(feature = "host")] keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    #[cfg(feature = "host")] reader_writer: Rw,
    #[cfg(feature = "vial")] vial_config: VialConfig<'static>,
    #[cfg(feature = "rmk_protocol")] wire_tx: Tx,
    #[cfg(feature = "rmk_protocol")] wire_rx: Rx,
    communication_fut: impl Future<Output = ()>,
    mut led_reader: R,
    mut keyboard_writer: W,
) {
```

**Step 2: Update host_fut creation inside run_keyboard**

Replace the single `host_fut` creation block (currently around line 377-383):

```rust
// Before:
#[cfg(feature = "host")]
let host_fut = run_host_communicate_task(
    keymap,
    reader_writer,
    #[cfg(feature = "vial")]
    vial_config,
);

// After:
#[cfg(feature = "vial")]
let host_fut = run_host_communicate_task(
    keymap,
    reader_writer,
    vial_config,
);

#[cfg(all(feature = "rmk_protocol", not(feature = "vial")))]
let host_fut = run_host_communicate_task(
    keymap,
    reader_writer,
    wire_tx,
    wire_rx,
);

#[cfg(all(feature = "host", not(feature = "vial"), not(feature = "rmk_protocol")))]
let host_fut = run_host_communicate_task(keymap, reader_writer);
```

**Step 3: Verify compilation with vial (regression check)**

Run: `cd rmk && cargo check --no-default-features --features=vial,storage`
Expected: compiles (vial path unchanged)

**Step 4: Commit**

```bash
git add rmk/src/lib.rs
git commit -m "feat(protocol): thread WireTx/WireRx transport through run_keyboard"
```

---

### Task 7: Create vendor bulk endpoints in USB-only path

**Files:**
- Modify: `rmk/src/lib.rs:270-328` (USB-only keyboard setup block)

**Context:** In the `#[cfg(all(not(feature = "_no_usb"), not(feature = "_ble")))]` block in `run_rmk_keyboard`, we need to create vendor bulk endpoints from the USB builder when `rmk_protocol` is enabled, wrap them in `UsbBulkTx`/`UsbBulkRx`, and pass them to `run_keyboard`.

**Step 1: Add vendor bulk endpoint creation**

In `rmk/src/lib.rs`, inside the USB-only block (around line 270), after the HID endpoint creation but before `usb_builder.build()`:

```rust
// After line 276 (host_reader_writer creation) and before line 288 (usb_builder.build()):
#[cfg(feature = "rmk_protocol")]
let (vendor_ep_out, vendor_ep_in) =
    crate::usb::add_usb_vendor_bulk!(&mut usb_builder);
```

**Step 2: Create WireTx/WireRx wrappers and pass to run_keyboard**

After `usb_builder.build()`, create the transport types and pass them to `run_keyboard`:

```rust
#[cfg(feature = "rmk_protocol")]
let wire_tx =
    crate::host::protocol::transport::UsbBulkTx::<D>::new(vendor_ep_in);
#[cfg(feature = "rmk_protocol")]
let wire_rx =
    crate::host::protocol::transport::UsbBulkRx::<D>::new(vendor_ep_out);
```

Update the `run_keyboard` call to pass the new parameters:

```rust
run_keyboard(
    #[cfg(feature = "storage")]
    storage,
    #[cfg(feature = "host")]
    keymap,
    #[cfg(feature = "host")]
    crate::host::UsbHostReaderWriter::new(&mut host_reader_writer),
    #[cfg(feature = "vial")]
    rmk_config.vial_config,
    #[cfg(feature = "rmk_protocol")]
    wire_tx,
    #[cfg(feature = "rmk_protocol")]
    wire_rx,
    usb_task,
    UsbLedReader::new(&mut keyboard_reader),
    UsbKeyboardWriter::new(&mut keyboard_writer, &mut other_writer),
)
```

**Step 3: Add necessary imports at top of lib.rs**

Ensure `postcard_rpc` is accessible (it's behind `rmk_protocol` feature gate):

```rust
#[cfg(all(not(feature = "_no_usb"), not(feature = "_ble"), feature = "rmk_protocol"))]
use crate::usb::add_usb_vendor_bulk;
```

**Step 4: Verify compilation**

Run: `cd rmk && cargo check --no-default-features --features=rmk_protocol,storage`
Expected: compiles successfully

**Step 5: Commit**

```bash
git add rmk/src/lib.rs
git commit -m "feat(protocol): create USB vendor bulk endpoints and wire into ProtocolService"
```

---

### Task 8: Wire vendor bulk endpoints in BLE+USB path

**Files:**
- Modify: `rmk/src/ble/mod.rs`

**Context:** BLE keyboards with USB (`not(feature = "_no_usb")`) also create a USB builder and call `run_keyboard`. We need to add vendor bulk endpoint creation and threading in the BLE path too, for when `rmk_protocol` is active. `run_keyboard` is called in 3 places within `ble/mod.rs` — all need the additional parameters.

**Step 1: Add vendor bulk endpoints to the BLE+USB setup**

In `rmk/src/ble/mod.rs`, after the existing USB endpoint setup (around line 143-144), add:

```rust
#[cfg(all(not(feature = "_no_usb"), feature = "rmk_protocol"))]
let (mut vendor_ep_out, mut vendor_ep_in) =
    add_usb_vendor_bulk!(&mut _usb_builder);
```

After `usb_builder.build()` (around line 151), add:

```rust
#[cfg(all(not(feature = "_no_usb"), feature = "rmk_protocol"))]
let wire_tx =
    crate::host::protocol::transport::UsbBulkTx::<D>::new(vendor_ep_in);
#[cfg(all(not(feature = "_no_usb"), feature = "rmk_protocol"))]
let wire_rx =
    crate::host::protocol::transport::UsbBulkRx::<D>::new(vendor_ep_out);
```

**Step 2: Update all 3 `run_keyboard` call sites in ble/mod.rs**

Each call to `run_keyboard` (at approximately lines 279, 337, and 878) needs the additional `#[cfg]`-gated parameters:

```rust
#[cfg(feature = "rmk_protocol")]
wire_tx,
#[cfg(feature = "rmk_protocol")]
wire_rx,
```

Note: For the BLE-only path (line 878), `wire_tx`/`wire_rx` are not USB endpoints — they would be BLE serial transport (Phase 8). For now, this call site uses BLE transport which doesn't have `rmk_protocol` vendor bulk endpoints. This call happens under `#[cfg(feature = "_no_usb")]` conditions where the vendor endpoints don't exist.

Verify which `run_keyboard` calls are under USB-available contexts. The first two (lines 279, 337) are in USB-connected states — add the parameters. The third (line 878) is BLE-only — it needs a placeholder or should be gated differently.

For BLE-only boards (`_no_usb`), `rmk_protocol` transport isn't available over USB. The `run_host_communicate_task` should fall back to `pending()`. Handle this by:
- Only pass `wire_tx`/`wire_rx` at USB-connected `run_keyboard` calls
- For BLE-only `run_keyboard` calls, add `#[cfg(all(feature = "rmk_protocol", not(feature = "_no_usb")))]` on the parameters

This requires the `run_keyboard` `wire_tx`/`wire_rx` parameters to be gated with `#[cfg(all(feature = "rmk_protocol", not(feature = "_no_usb")))]` instead of just `#[cfg(feature = "rmk_protocol")]`. Go back to Task 6 and adjust the gate accordingly. This ensures boards without USB don't need to provide the parameters.

**Step 3: Verify compilation**

Run: `cd rmk && cargo check --no-default-features --features=rmk_protocol,storage,nrf52840_ble`
Expected: compiles (BLE + USB path)

Run: `cd rmk && cargo check --no-default-features --features=rmk_protocol,storage,esp32c3_ble`
Expected: compiles (BLE-only path, no USB — `_no_usb` is set)

**Step 4: Commit**

```bash
git add rmk/src/ble/mod.rs rmk/src/lib.rs
git commit -m "feat(protocol): wire USB vendor bulk transport in BLE+USB path"
```

---

### Task 9: Handle `_no_usb` boards gracefully

**Files:**
- Modify: `rmk/src/host/mod.rs`
- Modify: `rmk/src/lib.rs` (run_keyboard)

**Context:** When `rmk_protocol` is enabled on a BLE-only board (`_no_usb`), there are no USB bulk endpoints. The BLE serial transport (Phase 8) is not yet implemented. For now, `run_host_communicate_task` should pend on these boards.

**Step 1: Adjust feature gates**

The `wire_tx`/`wire_rx` parameters throughout the call chain should be gated with:
```
#[cfg(all(feature = "rmk_protocol", not(feature = "_no_usb")))]
```
instead of just `#[cfg(feature = "rmk_protocol")]`.

This means in `run_keyboard`, the `Tx`/`Rx` generics and `wire_tx`/`wire_rx` parameters are only present when USB is available.

**Step 2: Add a fallback for rmk_protocol without USB**

In `rmk/src/host/mod.rs`, add:

```rust
#[cfg(all(feature = "rmk_protocol", not(feature = "vial"), feature = "_no_usb"))]
pub(crate) async fn run_host_communicate_task<
    'a,
    Rw: HidReaderTrait<ReportType = ViaReport> + HidWriterTrait<ReportType = ViaReport>,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    _keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    _reader_writer: Rw,
) {
    // BLE serial transport will be added in Phase 8
    core::future::pending::<()>().await
}
```

And update the USB variant gate to:

```
#[cfg(all(feature = "rmk_protocol", not(feature = "vial"), not(feature = "_no_usb")))]
```

**Step 3: Adjust run_keyboard host_fut creation**

```rust
#[cfg(all(feature = "rmk_protocol", not(feature = "vial"), not(feature = "_no_usb")))]
let host_fut = run_host_communicate_task(
    keymap,
    reader_writer,
    wire_tx,
    wire_rx,
);

#[cfg(all(feature = "rmk_protocol", not(feature = "vial"), feature = "_no_usb"))]
let host_fut = run_host_communicate_task(keymap, reader_writer);
```

**Step 4: Verify compilation**

Run: `cd rmk && cargo check --no-default-features --features=rmk_protocol,storage`
Expected: USB path compiles

Run: `cd rmk && cargo check --no-default-features --features=rmk_protocol,esp32c3_ble`
Expected: BLE-only path compiles (pends)

**Step 5: Commit**

```bash
git add rmk/src/host/mod.rs rmk/src/lib.rs
git commit -m "feat(protocol): handle _no_usb boards gracefully with pending fallback"
```

---

### Task 10: Full compilation verification and Vial regression test

**Files:** (no changes — verification only)

**Step 1: Verify rmk_protocol USB builds**

Run: `cd rmk && cargo check --no-default-features --features=rmk_protocol,storage`
Expected: PASS

**Step 2: Verify vial still works (regression)**

Run: `cd rmk && cargo test --no-default-features --features=split,vial,storage,async_matrix,_ble`
Expected: All tests pass (411+)

**Step 3: Verify rmk_protocol with BLE**

Run: `cd rmk && cargo check --no-default-features --features=rmk_protocol,storage,nrf52840_ble`
Expected: PASS (BLE + USB path)

**Step 4: Run clippy**

Run: `cd rmk && cargo clippy --no-default-features --features=rmk_protocol,storage -- -D warnings`
Expected: No warnings

**Step 5: Commit (if any fixups were needed)**

```bash
git add -A
git commit -m "fix: address clippy warnings and compilation issues from Phase 3"
```

---

### Task 11: Update ROADMAP.md

**Files:**
- Modify: `ROADMAP.md`

**Step 1: Mark Phase 3 steps as complete**

Update all Step 3.1–3.3 checkboxes from `[ ]` to `[x]` with implementation notes.

Update the Progress Summary table:
```markdown
| 3 | USB Raw Bulk Transport | **Complete** |
```

Update Phase 2 Step 2.2f:
```markdown
| f | Ensure `rmk_protocol` and `vial` are mutually exclusive | [x] | Enforced via #[cfg] gates in host/mod.rs |
```

**Step 2: Commit**

```bash
git add ROADMAP.md
git commit -m "docs: mark Phase 3 (USB Raw Bulk Transport) as complete in ROADMAP"
```

---

## Key Technical Decisions

### Why not reuse postcard-rpc's `EUsbWireTx`/`EUsbWireRx` directly?

postcard-rpc's USB implementation uses renamed crate imports (`embassy_usb_0_5`, `embassy_usb_driver_0_2`) which could cause type incompatibility with RMK's direct `embassy-usb` dependency. Enabling the `embassy-usb-0_5-server` feature would also pull in `embassy-executor` and other dependencies not currently used by RMK. Writing our own simplified implementation (~120 lines total) avoids these risks while following the exact same pattern.

### Why keep ViaReport HID when rmk_protocol is enabled?

When `rmk_protocol` implies `host`, and `host` gates ViaReport HID creation, the HID interface is still created but unused. This wastes one USB interface slot. Fixing this requires changing the `host` feature semantics (gate HID creation on `vial` instead of `host`). This is deferred to avoid a larger refactoring — the USB composite device handles the unused interface gracefully.

### Why gate wire_tx/wire_rx on `not(_no_usb)` instead of `rmk_protocol`?

BLE-only boards have no USB hardware, so USB bulk endpoints cannot be created. Phase 8 will provide BLE serial transport for these boards. For now, `_no_usb` + `rmk_protocol` boards get a pending future (protocol unavailable).

### Why `Mutex` instead of `RefCell` for WireTx interior mutability?

`WireTx::send()` takes `&self`, requiring interior mutability. While embassy's single-threaded executor makes `RefCell` safe in practice, `Mutex<CriticalSectionRawMutex, ...>` is consistent with postcard-rpc's pattern and works correctly with `Send` bounds that may be required by async executors.

---

## Files Changed Summary

| File | Action | Description |
|------|--------|-------------|
| `rmk/src/usb/mod.rs` | Modify | Increase BOS/MSOS buffers; add `add_usb_vendor_bulk!` macro |
| `rmk/src/host/protocol/transport.rs` | Create | `UsbBulkTx` (WireTx) and `UsbBulkRx` (WireRx) implementations |
| `rmk/src/host/protocol/mod.rs` | Modify | Add `pub(crate) mod transport;` |
| `rmk/src/host/mod.rs` | Modify | Update `run_host_communicate_task` to accept and use transport |
| `rmk/src/lib.rs` | Modify | Create vendor bulk endpoints, thread through `run_keyboard` |
| `rmk/src/ble/mod.rs` | Modify | Thread vendor bulk endpoints in BLE+USB path |
| `ROADMAP.md` | Modify | Mark Phase 3 as complete |
