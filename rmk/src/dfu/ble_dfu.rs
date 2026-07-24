//! BLE DFU — adapts the blocking flash used by the bootloader into an async
//! partition that `nrf-dfu-target` can write to.
//!
//! ## Why async flash?
//!
//! embassy-boot partitions the flash with `BlockingPartition`, which uses
//! `embedded-storage` (blocking/sync) trait implementations.  The BLE DFU
//! protocol handler (`nrf-dfu-target`) expects `embedded-storage-async`
//! (async) traits because it runs inside an async GATT event loop.
//!
//! [`DfuPartition`] wraps a mutex-protected blocking flash and implements
//! the async flash traits.  It also performs erase-on-write per 4 KiB sector
//! (for RP2040 and nRF), which is why its async `erase()` is intentionally a
//! no‑op — all erasure happens inside `write()`.
//!
//! ## Flow
//!
//! ```text
//! GATT write (DFU Control Point / Packet)
//!   → BleDfuHandler::handle_control_point / handle_packet
//!     → nrf-dfu-target process()
//!       → DfuPartition (async Flash)
//!         → Mutex<RefCell<BlockingPartition>>
//! ```
//!
//! [`DfuPartition`] resets its per-sector erase tracking when a new `Data`
//! object is created (via [`EraseStateReset`]).
use core::cell::RefCell;
#[cfg(feature = "dfu_lock")]
use core::sync::atomic::Ordering;

use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embedded_storage::nor_flash::{ErrorType, NorFlash, ReadNorFlash};
use embedded_storage_async::nor_flash::{NorFlash as AsyncNorFlash, ReadNorFlash as AsyncReadNorFlash};
use nrf_dfu_target::prelude::*;

const BLE_MTU: usize = 244;

/// Reset per-sector erase tracking when a new DFU `Data` object is created.
///
/// [`DfuPartition`] erases flash sectors inside `write()`, not inside
/// `erase()`.  When a new firmware upload starts (`Create { obj_type: Data }`),
/// the partition's sector tracking must be cleared so the first write of the
/// new firmware re‑erases the flash.
pub(crate) trait EraseStateReset {
    fn reset_erase_state(&mut self);
}

/// Async flash partition wrapping a mutex-guarded blocking flash.
///
/// Maps an offset/size sub‑region of the underlying flash and provides
/// `embedded-storage-async` trait implementations so `nrf-dfu-target` can
/// write firmware inside an async GATT handler.
///
/// Erasure is done per 4 KiB sector inside `write()` to avoid a separate
/// blocking erase call; the async `erase()` is therefore a no‑op.
pub(crate) struct DfuPartition<Flash: 'static> {
    flash_mutex: &'static Mutex<CriticalSectionRawMutex, RefCell<Flash>>,
    offset: u32,
    size: u32,
    last_erased_sector: u32,
}

impl<Flash: 'static> DfuPartition<Flash> {
    pub(crate) fn new(
        flash_mutex: &'static Mutex<CriticalSectionRawMutex, RefCell<Flash>>,
        offset: u32,
        size: u32,
    ) -> Self {
        Self {
            flash_mutex,
            offset,
            size,
            last_erased_sector: u32::MAX,
        }
    }
}

impl<Flash> EraseStateReset for DfuPartition<Flash> {
    fn reset_erase_state(&mut self) {
        self.last_erased_sector = u32::MAX;
    }
}

impl<Flash: ErrorType> ErrorType for DfuPartition<Flash> {
    type Error = Flash::Error;
}

impl<Flash: ReadNorFlash> AsyncReadNorFlash for DfuPartition<Flash> {
    const READ_SIZE: usize = Flash::READ_SIZE;

    async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.flash_mutex
            .lock(|f| f.borrow_mut().read(self.offset + offset, bytes))
    }

    fn capacity(&self) -> usize {
        self.size as usize
    }
}

impl<Flash: NorFlash> AsyncNorFlash for DfuPartition<Flash> {
    const WRITE_SIZE: usize = Flash::WRITE_SIZE;
    const ERASE_SIZE: usize = Flash::ERASE_SIZE;

    async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        const SECTOR: u32 = 4096;
        let start_sector = offset / SECTOR * SECTOR;
        let end_sector = (offset + bytes.len() as u32 - 1) / SECTOR * SECTOR;
        let mut sector = start_sector;
        while sector <= end_sector {
            if sector != self.last_erased_sector {
                self.flash_mutex.lock(|f| {
                    f.borrow_mut()
                        .erase(self.offset + sector, self.offset + sector + SECTOR)
                })?;
                self.last_erased_sector = sector;
            }
            sector += SECTOR;
        }
        self.flash_mutex
            .lock(|f| f.borrow_mut().write(self.offset + offset, bytes))
    }

    /// Erase-on-write happens inside `write()` per sector, so
    /// `AsyncNorFlash::erase` is intentionally a no-op here — the trait needs
    /// it, but we never call it externally.
    async fn erase(&mut self, _from: u32, _to: u32) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// BLE DFU state machine that owns a `nrf-dfu-target` `DfuTarget` and the
/// async flash partition.
///
/// Receives decoded DFU requests from the GATT event loop and delegates to
/// `DfuTarget::process()`.  Tracks completion via `self.complete` so the
/// caller can finalize (mark updated + reset) on disconnect.
pub(crate) struct BleDfuHandler<F: AsyncNorFlash> {
    target: DfuTarget<BLE_MTU>,
    flash: Option<F>,
    complete: bool,
}

impl<F: AsyncNorFlash + EraseStateReset> BleDfuHandler<F> {
    pub(crate) fn new(flash: F, flash_size: u32, flash_offset: u32) -> Self {
        let hw_info = HardwareInfo {
            // this info is for the nRF Connect app, which does not work for us anyway
            part: 0,
            variant: 0,
            rom_size: 0,
            rom_page_size: 0,
            ram_size: 0,
        };
        let fw_info = FirmwareInfo {
            ftype: FirmwareType::Application,
            version: 0,
            addr: flash_offset,
            len: flash_size,
        };
        Self {
            target: DfuTarget::new(flash_size, fw_info, hw_info),
            flash: Some(flash),
            complete: false,
        }
    }

    pub(crate) fn is_some(&self) -> bool {
        self.flash.is_some()
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.complete
    }

    pub(crate) async fn handle_control_point(&mut self, data: &[u8]) -> Option<DfuResponseState> {
        #[cfg(feature = "dfu_lock")]
        if !crate::dfu::is_dfu_unlocked() {
            crate::dfu::DFU_UNLOCK_SIGNAL.signal(());
            debug!("ble dfu: dfu_lock: DFU rejected (locked), waking unlock task");
            let opcode = data.first().copied().unwrap_or(0);
            let mut buf = [0u8; 64];
            buf[0] = 0x60;
            buf[1] = opcode;
            buf[2] = 0x06;
            return Some(DfuResponseState::Pending {
                response: buf,
                response_len: 3,
            });
        }
        #[cfg(feature = "dfu_lock")]
        crate::dfu::DFU_STARTED.store(true, Ordering::Release);
        crate::event::publish_event(crate::event::DfuStatusEvent::new(rmk_types::dfu::DfuStatus::Started));
        let mut rest = data;
        let mut last_response = None;

        while !rest.is_empty() {
            let (req, remaining) = match DfuRequest::decode(rest) {
                Ok(v) => v,
                Err(_e) => {
                    debug!("ble dfu: DfuRequest::decode failed");
                    return last_response;
                }
            };
            debug!("ble dfu: decoded request");
            rest = remaining;

            // Reset flash erase state on new Data object
            if matches!(
                &req,
                DfuRequest::Create {
                    obj_type: ObjectType::Data,
                    ..
                }
            ) {
                if let Some(ref mut f) = self.flash {
                    f.reset_erase_state();
                }
            }

            let Some(ref mut flash) = self.flash else {
                return last_response;
            };

            let (resp, status) = self.target.process(req, flash).await;

            if matches!(status, DfuStatus::DoneReset) {
                info!("ble dfu: Execute returned DoneReset");
            }

            let mut buf = [0u8; 64];
            let len = match resp.encode(&mut buf) {
                Ok(n) => n,
                Err(_) => continue,
            };

            let mut resp_buf = [0u8; 64];
            resp_buf[..len].copy_from_slice(&buf[..len]);

            if status == DfuStatus::DoneReset {
                self.complete = true;
                return Some(DfuResponseState::Pending {
                    response: resp_buf,
                    response_len: len,
                });
            }

            last_response = Some(DfuResponseState::Pending {
                response: resp_buf,
                response_len: len,
            });
        }

        last_response
    }

    pub(crate) async fn handle_packet(&mut self, data: &[u8]) -> Option<DfuResponseState> {
        #[cfg(feature = "dfu_lock")]
        if !crate::dfu::is_dfu_unlocked() {
            debug!("ble dfu: dfu_lock: packet rejected (locked)");
            let mut buf = [0u8; 64];
            buf[0] = 0x60;
            buf[1] = 0x08;
            buf[2] = 0x06;
            return Some(DfuResponseState::Pending {
                response: buf,
                response_len: 3,
            });
        }
        #[cfg(feature = "dfu_lock")]
        crate::dfu::DFU_STARTED.store(true, Ordering::Release);
        let Some(ref mut flash) = self.flash else {
            return None;
        };
        crate::event::publish_event(crate::event::DfuStatusEvent::new(
            rmk_types::dfu::DfuStatus::Downloading,
        ));

        // Forward Write to DfuTarget for flash write + CRC tracking
        let request = DfuRequest::Write { data };
        let (resp, status) = self.target.process(request, flash).await;

        let mut buf = [0u8; 64];
        let len = match resp.encode(&mut buf) {
            Ok(n) => n,
            Err(_) => return None,
        };

        let mut resp_buf = [0u8; 64];
        resp_buf[..len].copy_from_slice(&buf[..len]);

        if status == DfuStatus::DoneReset {
            self.complete = true;
        }
        Some(DfuResponseState::Pending {
            response: resp_buf,
            response_len: len,
        })
    }
}

/// Successfully decoded DFU response waiting to be re‑encoded and sent as a
/// GATT notification.
pub(crate) enum DfuResponseState {
    Pending { response: [u8; 64], response_len: usize },
}

impl DfuResponseState {
    pub(crate) fn response_data(&self) -> &[u8] {
        match self {
            DfuResponseState::Pending { response, response_len } => &response[..*response_len],
        }
    }
}

/// Human‑readable name for a DFU opcode, used in debug logs.
pub(crate) fn dfu_op_name(op: u8) -> &'static str {
    match op {
        0x01 => "Create",
        0x02 => "SetPRN",
        0x03 => "CRC",
        0x04 => "Execute",
        0x06 => "Select",
        _ => "?",
    }
}
