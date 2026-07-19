use core::cell::RefCell;
use core::sync::atomic::{AtomicUsize, Ordering};

use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_usb::class::dfu::consts::Status;
use embassy_usb::class::dfu::dfu_mode::{self};
use heapless;

// ---------------------------------------------------------------------------
// Firmware update data registry
// ---------------------------------------------------------------------------

/// A reference-counted firmware binary and its pre-computed CRC-32.
struct FirmwareSlot {
    data: &'static [u8],
    hash: u32,
}

/// Maximum number of peripherals with registered firmware.
const MAX_FW_SLOTS: usize = 4;

/// Global registry: peripheral ID → (`FirmwareSlot`).
///
/// Populated via [`set_firmware_update_data`].
/// Looked up by [`PeripheralManager`] on
/// connection to decide whether an update is needed.
static FW_SLOTS: Mutex<CriticalSectionRawMutex, RefCell<heapless::Vec<(usize, FirmwareSlot), MAX_FW_SLOTS>>> =
    Mutex::new(RefCell::new(heapless::Vec::new()));

/// Register a peripheral firmware binary for automatic dfu_split updates.
///
/// The central calls this (typically at startup) so that
/// `PeripheralManager` can verify and, if needed, update the peripheral's
/// firmware when the split link is established.
///
/// `id` must match the peripheral index in `[[split.peripheral]]` (or the
/// `id` argument of `run_peripheral_manager`).  `hash` is the CRC-32 of
/// the firmware binary — typically computed via [`crate::crc32::crc32`].
/// `id` must be unique; if a slot for the same `id` already exists, it will be replaced.
/// Every peripheral has only one firmware slot, given by its unique `id`.
///
/// Returns `Err(())` if the registry is full (max `MAX_FW_SLOTS` entries).
pub fn set_firmware_update_data(id: usize, firmware: &'static [u8], hash: u32) -> Result<(), ()> {
    FW_SLOTS.lock(|cell| {
        let slots = &mut cell.borrow_mut();
        if let Some(slot) = slots.iter_mut().find(|(i, _)| *i == id) {
            *slot = (id, FirmwareSlot { data: firmware, hash });
        } else {
            slots
                .push((id, FirmwareSlot { data: firmware, hash }))
                .map_err(|_| ())?;
        }
        Ok(())
    })
}

/// Retrieve the firmware binary and its expected CRC-32 for a given
/// peripheral ID, if one has been registered.
pub fn get_firmware_update_data(id: usize) -> Option<(&'static [u8], u32)> {
    FW_SLOTS.lock(|cell| {
        let slots = cell.borrow();
        slots.iter().find(|(i, _)| *i == id).map(|(_, s)| (s.data, s.hash))
    })
}

/// ── PASSTHROUGH QUEUE: USB ISR → async PeripheralManager ─────────
///
/// The central gets 512B DNLOAD blocks from the host, while the peripheral gets
/// 256B chunks over the split link.  The USB ISR splits each block into two chunks
/// and in order to smooth out the flow, a small FIFO queue is used to decouple
/// the two contexts. To prevent the host from sending more data than the queue
/// can hold, the GETSTATUS reply is modified to return `dfuDNBUSY` while the
/// queue is not empty.
///
/// ```text
///   USB Host                          Central MCU
///   ────────                          ──────────
///
///   dfu-util -a 1 -D fw.bin
///        │
///        │  USB Control Transfer (DNLOAD, 512 bytes)
///        v
///   ┌──────────────────────────┐
///   │  PassthroughDfuHandler   │  USB ISR (synchronous)
///   │  .write(data)            │  splits 512B → 2 × 256B chunks
///   │                          │  calls passthrough_push() each
///   └──────────────────────────┘
///        │
///        │  passthrough_push(Chunk{offset, data})
///        v
///   ┌──────────────────────────┐
///   │     PASSTHROUGH_CMD      │  heapless::Vec<Command, 4>
///   │  [  Chunk(0)   ]         │  FIFO-queue
///   │  [  Chunk(256) ]         │  max 4 entries (QUEUE_SIZE)
///   │  [   ...       ]         │  protected by CriticalSectionMutex
///   │  [   free      ]         │
///   └──────────────────────────┘
///        │
///        │  PASSTHROUGH_TARGET = peripheral_id  (doorbell)
///        v
///   ┌──────────────────────────┐
///   │   PeripheralManager      │  async event loop (every 5 ms)
///   │   .handle_passthrough()  │
///   │                          │  while passthrough_pending(id):
///   │  1. passthrough_take()   │    cmd = queue.pop()
///   │  2. send() over split    │    send(FirmwareChunk) to peripheral
///   │  3. wait for Ack         │    wait(FirmwareChunkAck)
///   │  4. clear doorbell       │    if queue.empty(): target = MAX
///   └──────────────────────────┘
///        │
///        │  UART
///        │  (SplitMessage::FirmwareChunk)
///        v
///   ┌──────────────────────────┐
///   │      Peripheral          │
///   │  SplitDfuHandler         │
///   │  .write_chunk()          │  flash erase + write, send Ack
///   └──────────────────────────┘
///
///
/// ── FLOW CONTROL ────────────────────────────────────────────────
///
///   While PASSTHROUGH_TARGET != MAX every GETSTATUS reply has
///   state = dfuDNBUSY (4).  The host polls again after 50 ms.
///   Once the queue is drained and passthrough_done_if_empty()
///   clears the target, the real DFU state is returned and the
///   host sends the next DNLOAD block.
///
/// ```

/// DFU `Handler` used for **passthrough** alternate settings on the
/// central's USB DFU interface.
///
/// Runs inside the USB interrupt.  Each incoming DNLOAD block is split
/// into 256-byte chunks and pushed into [`PASSTHROUGH_CMD`].  The
/// async `PeripheralManager` task drains the queue and forwards chunks
/// to the peripheral over the split link.  GETSTATUS flow control
/// (cf. [`PASSTHROUGH_TARGET`]) ensures the host waits when the queue
/// is not empty.
pub(crate) struct PassthroughDfuHandler {
    /// Which peripheral this handler forwards to.
    pub target_id: usize,
    /// Accumulated byte count (used as flash offset on the peripheral).
    pub written: u32,
}

impl dfu_mode::Handler for PassthroughDfuHandler {
    fn start(&mut self) -> Result<(), Status> {
        self.written = 0;
        Ok(())
    }

    fn write(&mut self, data: &[u8]) -> Result<(), Status> {
        for chunk in data.chunks(256) {
            let mut buf = [0u8; 256];
            buf[..chunk.len()].copy_from_slice(chunk);
            if passthrough_push(PassthroughCommand::Chunk(PassthroughChunk {
                offset: self.written,
                len: chunk.len() as u16,
                data: buf,
            }))
            .is_err()
            {
                error!("dfu_split: passthrough queue full");
                return Err(Status::ErrUnknown);
            }
            self.written += chunk.len() as u32;
        }
        PASSTHROUGH_TARGET.store(self.target_id, Ordering::Release);
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Status> {
        if passthrough_push(PassthroughCommand::Finish).is_err() {
            error!("dfu_split: passthrough queue full at finish");
            return Err(Status::ErrUnknown);
        }
        PASSTHROUGH_TARGET.store(self.target_id, Ordering::Release);
        Ok(())
    }

    fn system_reset(&mut self) {}
}

/// A single chunk of firmware data queued for passthrough.
pub(crate) struct PassthroughChunk {
    /// Flash offset where this chunk should be written.
    pub offset: u32,
    /// Raw data (zero-padded to 256 bytes).
    pub data: [u8; 256],
    /// Actual number of meaningful bytes in `data`.
    pub len: u16,
}

/// Commands flowing from the USB ISR
/// ([`PassthroughDfuHandler`]) to the async
/// [`PeripheralManager`](crate::split::driver::PeripheralManager).
pub(crate) enum PassthroughCommand {
    /// A firmware chunk to be forwarded.
    Chunk(PassthroughChunk),
    /// Signal that all chunks have been sent; triggers end-to-end CRC
    /// verification.
    Finish,
}

/// Maximum number of pending chunks in the fire-and-forget queue.
const PASSTHROUGH_QUEUE_SIZE: usize = 4;

/// Fire-and-forget command queue.
///
/// The USB DFU handler (ISR context) pushes; the async
/// `PeripheralManager` pops.  Protected by a critical-section mutex
/// so it is safe from both contexts.
static PASSTHROUGH_CMD: Mutex<
    CriticalSectionRawMutex,
    RefCell<heapless::Vec<PassthroughCommand, PASSTHROUGH_QUEUE_SIZE>>,
> = Mutex::new(RefCell::new(heapless::Vec::new()));

/// Doorbell atomic: set to a peripheral ID when there is work in
/// [`PASSTHROUGH_CMD`], `usize::MAX` when idle.
///
/// Read by [`RmkDfuInterface::control_in`] to inject `dfuDNBUSY` into
/// the GETSTATUS response (adaptive host-side flow control).
pub(crate) static PASSTHROUGH_TARGET: AtomicUsize = AtomicUsize::new(usize::MAX);

/// Check whether a passthrough command is pending for the given
/// peripheral ID.
pub(crate) fn passthrough_pending(id: usize) -> bool {
    PASSTHROUGH_TARGET.load(Ordering::Acquire) == id
}

/// Push a command into the queue (ISR-safe).
fn passthrough_push(cmd: PassthroughCommand) -> Result<(), ()> {
    PASSTHROUGH_CMD.lock(|c| c.borrow_mut().push(cmd).map_err(|_| ()))
}

/// Pop the next pending command (async task).
pub(crate) fn passthrough_take_command() -> Option<PassthroughCommand> {
    PASSTHROUGH_CMD.lock(|c| {
        let v = &mut *c.borrow_mut();
        if !v.is_empty() { Some(v.remove(0)) } else { None }
    })
}

/// Clear the target doorbell if the queue is empty.
///
/// Called after every command is processed.  If the queue still has
/// items the target stays set, keeping the host in `dfuDNBUSY` until
/// the PeripheralManager catches up.
pub(crate) fn passthrough_done_if_empty() {
    let empty = PASSTHROUGH_CMD.lock(|c| c.borrow().is_empty());
    if empty {
        PASSTHROUGH_TARGET.store(usize::MAX, Ordering::Release);
    }
}

/// Drain all pending passthrough commands (e.g. on disconnect).
pub(crate) fn drain_passthrough() {
    PASSTHROUGH_CMD.lock(|c| c.borrow_mut().clear());
    PASSTHROUGH_TARGET.store(usize::MAX, Ordering::Release);
}
