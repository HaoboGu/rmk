use core::cell::RefCell;
#[cfg(feature = "dfu_lock")]
use core::sync::atomic::AtomicBool;
#[cfg(any(feature = "dfu_lock", feature = "dfu_split"))]
use core::sync::atomic::Ordering;

use embassy_sync::blocking_mutex::Mutex;
#[cfg(feature = "dfu_lock")]
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::raw::RawMutex;
#[cfg(feature = "dfu_lock")]
use embassy_sync::signal::Signal;
use embassy_usb::control::{InResponse, OutResponse, Request};
use embassy_usb::driver::Driver;
use embassy_usb::types::{InterfaceNumber, StringIndex};
use embassy_usb::{Builder, Handler};
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
use embassy_usb_dfu::{ResetImmediate, dfu::FirmwareHandler};
use static_cell::StaticCell;

#[cfg(feature = "dfu_lock")]
use crate::core_traits::Runnable;

// ---------------------------------------------------------------------------
// Chip modules
// ---------------------------------------------------------------------------

#[cfg(feature = "dfu_nrf")]
mod nrf;
#[cfg(feature = "dfu_rp")]
mod rp;
#[cfg(feature = "dfu_nrf")]
pub use self::nrf::{FlashType, MutexType, PartitionType};
#[cfg(feature = "dfu_rp")]
pub use self::rp::FLASH_SIZE;
#[cfg(feature = "dfu_rp")]
pub use self::rp::{FlashType, MutexType, PartitionType};
#[cfg(feature = "dfu_nrf")]
use self::nrf::{build_driver, FlashDriver};
#[cfg(feature = "dfu_rp")]
use self::rp::{build_driver, FlashDriver};
// ---------------------------------------------------------------------------
// Type-erased DFU partition — internal flash or external (SPI) flash
// ---------------------------------------------------------------------------

/// Write/erase granularity exposed by [`DfuPartition`].
///
/// `NorFlash` constants are compile-time constants of the type, so [`DfuPartition`]
/// advertises a single size pair that is valid for every supported flash:
/// 256-byte writes are a multiple of every flash's WRITE_SIZE (RP2040: 256,
/// nRF NVMC: 4, 25-series SPI NOR: 4) and 4 KiB erases match every supported
/// flash's sector. The concrete flash driver enforces its own alignment on top.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
pub const DFU_WRITE_SIZE: usize = 256;
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
const DFU_ERASE_SIZE: usize = 4096;

/// Object-safe view of a blocking NOR flash.
///
/// `embedded_storage`'s `NorFlash` cannot be used as a trait object (it is not
/// dyn-compatible) and `embassy_sync`'s `Mutex` offers no unsizing coercion,
/// so the concrete external flash of a `dfu_ext` build is erased behind this
/// trait. The blanket impl covers any flash parked in a `'static` mutex; each
/// access is serialized by that mutex. The mutex must use a `Sync` raw mutex
/// (e.g. [`CriticalSectionRawMutex`]) so the trait object itself is `Sync`.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
trait DfuFlashOps: Sync {
    fn capacity(&self) -> usize;
    fn write_size(&self) -> usize;
    fn erase_size(&self) -> usize;
    fn read(&self, offset: u32, buf: &mut [u8]) -> Result<(), DfuPartitionError>;
    fn erase(&self, from: u32, to: u32) -> Result<(), DfuPartitionError>;
    fn write(&self, offset: u32, buf: &[u8]) -> Result<(), DfuPartitionError>;
}

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
impl<M: RawMutex + Sync, F: NorFlash + Send> DfuFlashOps for Mutex<M, RefCell<F>> {
    fn capacity(&self) -> usize {
        self.lock(|cell| cell.borrow().capacity())
    }

    fn write_size(&self) -> usize {
        <F as NorFlash>::WRITE_SIZE
    }

    fn erase_size(&self) -> usize {
        <F as NorFlash>::ERASE_SIZE
    }

    fn read(&self, offset: u32, buf: &mut [u8]) -> Result<(), DfuPartitionError> {
        self.lock(|cell| cell.borrow_mut().read(offset, buf))
            .map_err(|e| DfuPartitionError::from(e.kind()))
    }

    fn erase(&self, from: u32, to: u32) -> Result<(), DfuPartitionError> {
        self.lock(|cell| cell.borrow_mut().erase(from, to))
            .map_err(|e| DfuPartitionError::from(e.kind()))
    }

    fn write(&self, offset: u32, buf: &[u8]) -> Result<(), DfuPartitionError> {
        self.lock(|cell| cell.borrow_mut().write(offset, buf))
            .map_err(|e| DfuPartitionError::from(e.kind()))
    }
}

/// Error type for [`DfuPartition`].
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
#[derive(Debug)]
pub enum DfuPartitionError {
    /// The arguments are not properly aligned.
    NotAligned,
    /// The arguments are out of bounds.
    OutOfBounds,
    /// Error specific to the underlying flash driver.
    Flash,
}

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
impl From<NorFlashErrorKind> for DfuPartitionError {
    fn from(kind: NorFlashErrorKind) -> Self {
        match kind {
            NorFlashErrorKind::NotAligned => Self::NotAligned,
            NorFlashErrorKind::OutOfBounds => Self::OutOfBounds,
            _ => Self::Flash,
        }
    }
}

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
impl NorFlashError for DfuPartitionError {
    fn kind(&self) -> NorFlashErrorKind {
        match self {
            Self::NotAligned => NorFlashErrorKind::NotAligned,
            Self::OutOfBounds => NorFlashErrorKind::OutOfBounds,
            Self::Flash => NorFlashErrorKind::Other,
        }
    }
}

/// The DFU download partition of a keyboard.
///
/// Either the internal DFU partition of the boot layout ([`DfuPartition::Internal`])
/// or the external DFU flash with which the [`DfuFlashManager`] was built
/// ([`DfuPartition::External`], set up by
/// [`init_flash_from_linkerscript_with_external_dfu`]).
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
#[derive(Clone)]
pub enum DfuPartition {
    /// Partition over the internal flash, laid out by the linker script.
    Internal(PartitionType),
    /// Partition over an external (SPI) flash.
    External(ExternalDfuPartition),
}

/// A partition over the external DFU flash of a `dfu_ext` build.
///
/// The external flash occupies its full address space as the DFU download
/// partition. The concrete driver is kept behind a trait object.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
#[derive(Clone)]
pub struct ExternalDfuPartition {
    ops: &'static dyn DfuFlashOps,
    size: u32,
}

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
impl ExternalDfuPartition {
    fn check_bounds(&self, offset: u32, len: u32) -> Result<(), DfuPartitionError> {
        if offset.saturating_add(len) > self.size {
            Err(DfuPartitionError::OutOfBounds)
        } else {
            Ok(())
        }
    }

    fn check_erase(&self, from: u32, to: u32) -> Result<(), DfuPartitionError> {
        if from > to {
            return Err(DfuPartitionError::OutOfBounds);
        }
        self.check_bounds(from, to - from)?;
        if from % DFU_ERASE_SIZE as u32 != 0 || (to - from) % DFU_ERASE_SIZE as u32 != 0 {
            return Err(DfuPartitionError::NotAligned);
        }
        Ok(())
    }
}

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
impl ErrorType for DfuPartition {
    type Error = DfuPartitionError;
}

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
impl ReadNorFlash for DfuPartition {
    const READ_SIZE: usize = 1;

    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        match self {
            DfuPartition::Internal(part) => part.read(offset, bytes).map_err(|e| DfuPartitionError::from(e.kind())),
            DfuPartition::External(ext) => {
                ext.check_bounds(offset, bytes.len() as u32)?;
                ext.ops.read(offset, bytes)
            }
        }
    }

    fn capacity(&self) -> usize {
        match self {
            DfuPartition::Internal(part) => part.capacity(),
            DfuPartition::External(ext) => ext.size as usize,
        }
    }
}

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
impl NorFlash for DfuPartition {
    const WRITE_SIZE: usize = DFU_WRITE_SIZE;
    const ERASE_SIZE: usize = DFU_ERASE_SIZE;

    fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        match self {
            DfuPartition::Internal(part) => part.erase(from, to).map_err(|e| DfuPartitionError::from(e.kind())),
            DfuPartition::External(ext) => {
                ext.check_erase(from, to)?;
                ext.ops.erase(from, to)
            }
        }
    }

    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        match self {
            DfuPartition::Internal(part) => part.write(offset, bytes).map_err(|e| DfuPartitionError::from(e.kind())),
            DfuPartition::External(ext) => {
                ext.check_bounds(offset, bytes.len() as u32)?;
                if offset % DFU_WRITE_SIZE as u32 != 0 || bytes.len() % DFU_WRITE_SIZE != 0 {
                    return Err(DfuPartitionError::NotAligned);
                }
                ext.ops.write(offset, bytes)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Chip-specific type aliases
// ---------------------------------------------------------------------------

/// DFU transfer block size in bytes. Larger values speed up firmware
/// downloads. Must match the USB control buffer size used by the host.
pub const BLOCK_SIZE_DFU: usize = 512;

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
use embassy_boot::BlockingFirmwareState;
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
use embassy_embedded_hal::flash::partition::BlockingPartition;
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
use embassy_sync::once_lock::OnceLock;
use embedded_storage::nor_flash::{ErrorType, NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash};

// ---------------------------------------------------------------------------
// Flash init — chip-independent. The chip module only provides the flash
// types; everything else (mutex, manager, linker-script layout) lives here.
// ---------------------------------------------------------------------------

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
static FLASH_CELL: StaticCell<MutexType> = StaticCell::new();
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
static MANAGER: OnceLock<DfuFlashManager> = OnceLock::new();

/// Read the DFU partition layout from the rmk-boot.x linker script.
///
/// # Safety
///
/// Reads linker-defined absolute symbols. The symbols must be present in the
/// linked binary or the firmware will not link.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
fn read_boot_layout() -> [u32; 6] {
    unsafe extern "C" {
        static __rmk_boot_state_offset: u8;
        static __rmk_boot_state_size: u8;
        static __rmk_boot_dfu_offset: u8;
        static __rmk_boot_dfu_size: u8;
        static __rmk_boot_storage_offset: u8;
        static __rmk_boot_storage_size: u8;
    }
    // SAFETY: linker-defined symbols — reading their addresses is safe.
    [
        core::ptr::addr_of!(__rmk_boot_storage_offset) as usize as u32,
        core::ptr::addr_of!(__rmk_boot_storage_size) as usize as u32,
        core::ptr::addr_of!(__rmk_boot_state_offset) as usize as u32,
        core::ptr::addr_of!(__rmk_boot_state_size) as usize as u32,
        core::ptr::addr_of!(__rmk_boot_dfu_offset) as usize as u32,
        core::ptr::addr_of!(__rmk_boot_dfu_size) as usize as u32,
    ]
}

/// Initialize the blocking flash, create the DFU manager and store it
/// globally, using partition offsets from the linker (rmk-boot.x).
///
/// `flash` is the raw flash peripheral — `p.FLASH` on RP2040, `p.NVMC` on
/// nRF. The flash is parked in a `'static` mutex, the [`DfuFlashManager`]
/// is stored globally and the storage partition is returned.
///
/// Requires `rmk-boot.x` to be linked into the firmware binary
/// (e.g. `-Trmk-boot.x` in `.cargo/config.toml`).
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
pub fn init_flash_from_linkerscript(flash: FlashType) -> PartitionType {
    init_flash_with_layout(build_driver(flash))
}

/// Initialize flash using partition offsets from the linker (rmk-boot.x) and
/// build the DFU manager with the external (SPI) flash as the DFU download
/// partition.
///
/// Combines [`init_flash_from_linkerscript`] with external-flash DFU setup:
/// `dfu_flash` (wrapped in a `'static` mutex) becomes the manager's DFU
/// partition — `mgr.dfu_partition()` returns it — while the state and storage
/// partitions stay on the internal flash. Returns the internal storage
/// partition.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
pub fn init_flash_from_linkerscript_with_external_dfu<M: RawMutex + Sync, DFU: NorFlash + Send>(
    flash: FlashType,
    dfu_flash: &'static Mutex<M, RefCell<DFU>>,
) -> PartitionType {
    init_flash_with_layout_with_external_dfu(build_driver(flash), dfu_flash)
}

/// Shared implementation: park the constructed internal flash driver in a
/// `'static` mutex, build the [`DfuFlashManager`] from the linker layout and
/// store it globally.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
pub(crate) fn init_flash_with_layout(flash: FlashDriver) -> PartitionType {
    let [
        storage_offset,
        storage_size,
        state_offset,
        state_size,
        dfu_offset,
        dfu_size,
    ] = read_boot_layout();
    let flash_mutex: &'static MutexType = FLASH_CELL.init(Mutex::new(RefCell::new(flash)));
    let dfu_part = DfuPartition::Internal(BlockingPartition::new(flash_mutex, dfu_offset, dfu_size));
    let mgr = DfuFlashManager::new(
        flash_mutex,
        storage_offset,
        storage_size,
        state_offset,
        state_size,
        dfu_part,
    );
    let partition = mgr.storage_partition();
    MANAGER.init(mgr).ok();
    partition
}

/// Mark firmware boot as successful so the bootloader doesn't revert on next
/// reset.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
pub fn mark_booted() {
    if let Some(mgr) = get_manager() {
        let state_part = mgr.state_partition();
        static ALIGNED: StaticCell<[u8; DFU_WRITE_SIZE]> = StaticCell::new();
        let aligned: &'static mut [u8] = ALIGNED.init([0; DFU_WRITE_SIZE]);
        let mut state = BlockingFirmwareState::new(state_part, aligned);
        state.mark_booted().ok();
    }
}

/// Get a reference to the global DFU flash manager.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
pub fn get_manager() -> Option<&'static DfuFlashManager> {
    MANAGER.try_get()
}

// ---------------------------------------------------------------------------
// dfu_split sub-module (behind feature flag)
// ---------------------------------------------------------------------------

#[cfg(feature = "dfu_split")]
mod split;
#[cfg(feature = "dfu_split")]
use self::split::PassthroughDfuHandler;
#[cfg(feature = "dfu_split")]
pub(crate) use self::split::{
    PASSTHROUGH_SIGNAL, PASSTHROUGH_TARGET, PassthroughCommand, passthrough_done_if_empty, passthrough_pending,
    passthrough_take_command,
};
#[cfg(feature = "dfu_split")]
pub use self::split::{
    SplitDfuHandler, get_firmware_update_data, read_embedded_firmware_hash, set_firmware_update_data,
};

// ---------------------------------------------------------------------------
// DfuFlashManager — shared by RP2040 and nRF
// ---------------------------------------------------------------------------

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
pub struct DfuFlashManager {
    flash_mutex: &'static MutexType,
    state_offset: u32,
    state_size: u32,
    dfu_partition: DfuPartition,
    storage_offset: u32,
    storage_size: u32,
}

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
impl DfuFlashManager {
    pub(super) fn new(
        flash_mutex: &'static MutexType,
        storage_offset: u32,
        storage_size: u32,
        state_offset: u32,
        state_size: u32,
        dfu_partition: DfuPartition,
    ) -> Self {
        Self {
            flash_mutex,
            state_offset,
            state_size,
            dfu_partition,
            storage_offset,
            storage_size,
        }
    }

    pub fn state_partition(&self) -> PartitionType {
        BlockingPartition::new(self.flash_mutex, self.state_offset, self.state_size)
    }

    /// The DFU download partition.
    ///
    /// If the manager was built by
    /// [`init_flash_from_linkerscript_with_external_dfu`], this is the external
    /// flash; otherwise it is the internal DFU partition of the boot layout.
    pub fn dfu_partition(&self) -> DfuPartition {
        self.dfu_partition.clone()
    }

    pub fn storage_partition(&self) -> PartitionType {
        BlockingPartition::new(self.flash_mutex, self.storage_offset, self.storage_size)
    }
}

// ---------------------------------------------------------------------------
// DfuStringProvider
// ---------------------------------------------------------------------------

struct DfuStringProvider {
    string_idx: StringIndex,
    string_val: &'static str,
}

impl Handler for DfuStringProvider {
    fn control_out(&mut self, _req: Request, _data: &[u8]) -> Option<OutResponse> {
        None
    }
    fn control_in<'a>(&'a mut self, _req: Request, _buf: &'a mut [u8]) -> Option<InResponse<'a>> {
        None
    }
    fn get_string(&mut self, index: StringIndex, _lang_id: u16) -> Option<&'static str> {
        (index == self.string_idx).then_some(self.string_val)
    }
}

// ---------------------------------------------------------------------------
// DFU lock state
// ---------------------------------------------------------------------------

#[cfg(feature = "dfu_lock")]
static DFU_LOCKED: AtomicBool = AtomicBool::new(true);
#[cfg(feature = "dfu_lock")]
static DFU_STARTED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "dfu_lock")]
static DFU_UNLOCK_SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

#[cfg(feature = "dfu_lock")]
pub fn is_dfu_unlocked() -> bool {
    !DFU_LOCKED.load(Ordering::Acquire)
}

// ---------------------------------------------------------------------------
// RmkDfuHandler
// ---------------------------------------------------------------------------

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
use embassy_usb::class::dfu::dfu_mode::DfuState;
#[cfg(feature = "dfu")]
use embassy_usb::class::dfu::{
    consts::Status,
    dfu_mode::{self},
};
#[cfg(any(feature = "dfu", feature = "dfu_lock"))]
use rmk_types::dfu::DfuStatus;

/// DFU handler wrapper that blinks an LED during transfer and checks the
/// DFU lock (if `dfu_lock` feature is enabled).
#[cfg(any(feature = "dfu", feature = "dfu_lock"))]
use crate::event::publish_event;

#[cfg(feature = "dfu")]
pub struct RmkDfuHandler<H> {
    inner: H,
    target_id: Option<usize>,
}

#[cfg(feature = "dfu")]
impl<H: dfu_mode::Handler> dfu_mode::Handler for RmkDfuHandler<H> {
    fn start(&mut self) -> Result<(), Status> {
        #[cfg(feature = "dfu_lock")]
        if !is_dfu_unlocked() {
            DFU_UNLOCK_SIGNAL.signal(());
            info!("dfu_lock: DFU download rejected — keys not unlocked");
            return Err(Status::ErrVendor);
        }
        #[cfg(feature = "dfu_lock")]
        DFU_STARTED.store(true, Ordering::Release);
        match self.target_id {
            Some(id) => info!("dfu: DFU download started (passthrough peripheral {})", id),
            None => info!("dfu: DFU download started (central)"),
        }
        publish_event(crate::event::DfuStatusEvent::new(DfuStatus::Started));
        self.inner.start()
    }

    fn write(&mut self, data: &[u8]) -> Result<(), Status> {
        publish_event(crate::event::DfuStatusEvent::new(DfuStatus::Downloading));
        self.inner.write(data)
    }

    fn finish(&mut self) -> Result<(), Status> {
        let res = self.inner.finish();
        publish_event(crate::event::DfuStatusEvent::new(if res.is_ok() {
            DfuStatus::Finished
        } else {
            DfuStatus::Error
        }));
        res
    }

    fn system_reset(&mut self) {
        self.inner.system_reset()
    }
}

// ---------------------------------------------------------------------------
// RmkDfuInterface — single USB handler for all DFU alt settings
// ---------------------------------------------------------------------------

/// Max passthrough alt settings supported on a single DFU interface.
#[cfg(feature = "dfu_split")]
const MAX_PASSTHROUGH_ALTS: usize = 4;

/// Single USB `Handler` that owns all DFU alternate settings.
///
/// Alt 0 is always the central's own DFU flash.  Alt 1..N are passthrough
/// slots for split peripherals (requires `dfu_split`).
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
struct RmkDfuInterface {
    central: Option<
        &'static mut DfuState<
            RmkDfuHandler<FirmwareHandler<'static, DfuPartition, PartitionType, ResetImmediate, BLOCK_SIZE_DFU>>,
        >,
    >,
    #[cfg(feature = "dfu_split")]
    passthrough: [Option<DfuState<RmkDfuHandler<PassthroughDfuHandler>>>; MAX_PASSTHROUGH_ALTS],
    #[cfg(feature = "dfu_split")]
    num_passthrough: usize,
    current_alt: u8,
}

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
impl Handler for RmkDfuInterface {
    fn set_alternate_setting(&mut self, _iface: InterfaceNumber, alternate_setting: u8) {
        self.current_alt = alternate_setting;
    }

    fn control_out(&mut self, req: Request, data: &[u8]) -> Option<OutResponse> {
        match self.current_alt {
            0 => self.central.as_mut().and_then(|c| c.control_out(req, data)),
            #[cfg(feature = "dfu_split")]
            n => {
                let idx = (n as usize).saturating_sub(1);
                self.passthrough_slots(idx).and_then(|s| s.control_out(req, data))
            }
            #[cfg(not(feature = "dfu_split"))]
            _ => None,
        }
    }

    fn control_in<'a>(&'a mut self, req: Request, buf: &'a mut [u8]) -> Option<InResponse<'a>> {
        match self.current_alt {
            0 => self.central.as_mut().and_then(|c| c.control_in(req, buf)),
            #[cfg(feature = "dfu_split")]
            n => {
                let idx = (n as usize).saturating_sub(1);
                let buf_ptr = buf.as_mut_ptr();
                let resp = self.passthrough_slots(idx).and_then(|s| s.control_in(req, buf));
                // ── Flow control: dfuDNBUSY override ──────────────────────
                // Byte 4 of the GETSTATUS response (6 bytes) is the `state`
                // field.  While PASSTHROUGH_TARGET is set (!= usize::MAX) we override
                // it with 4 (= dfuDNBUSY).  dfu-util then waits 50 ms and polls again
                // — adaptive back-pressure without a fixed timeout.
                // Uses a volatile store because `buf` is still borrowed by
                // `resp` at this point.
                if resp.is_some() && PASSTHROUGH_TARGET.load(Ordering::Acquire) != usize::MAX {
                    unsafe {
                        core::ptr::write_volatile(buf_ptr.add(4), 4u8);
                    }
                }
                resp
            }
            #[cfg(not(feature = "dfu_split"))]
            _ => None,
        }
    }
}

#[cfg(feature = "dfu_split")]
impl RmkDfuInterface {
    fn passthrough_slots(&mut self, idx: usize) -> Option<&mut DfuState<RmkDfuHandler<PassthroughDfuHandler>>> {
        self.passthrough.get_mut(idx)?.as_mut()
    }
}

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
static RMK_DFU_INTERFACE: StaticCell<RmkDfuInterface> = StaticCell::new();

// ---------------------------------------------------------------------------
// Central DFU state — built from internal or external flash
// ---------------------------------------------------------------------------

/// Build the central DFU state for a DFU download partition.
///
/// `dfu_part` is either the internal partition (`mgr.dfu_partition()`) or a
/// partition over an external flash; `state_part` always lives on the internal
/// flash.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
fn build_dfu_state<P: NorFlash>(
    dfu_part: P,
    state_part: PartitionType,
) -> DfuState<RmkDfuHandler<FirmwareHandler<'static, P, PartitionType, ResetImmediate, BLOCK_SIZE_DFU>>> {
    use embassy_boot::{BlockingFirmwareUpdater, FirmwareUpdaterConfig};
    use embassy_usb::class::dfu::consts::DfuAttributes;

    let config = FirmwareUpdaterConfig {
        dfu: dfu_part,
        state: state_part,
    };
    static ALIGNED: StaticCell<[u8; DFU_WRITE_SIZE]> = StaticCell::new();
    let aligned: &'static mut [u8] = ALIGNED.init([0; DFU_WRITE_SIZE]);
    let updater = BlockingFirmwareUpdater::new(config, aligned);
    let handler = RmkDfuHandler {
        inner: FirmwareHandler::new(updater, ResetImmediate),
        target_id: None,
    };
    let attrs = DfuAttributes::CAN_DOWNLOAD | DfuAttributes::WILL_DETACH;
    DfuState::new(handler, attrs)
}

/// Shared implementation for the external-flash DFU path: like
/// [`init_flash_with_layout`], but the external `dfu_flash` (wrapped in a
/// `'static` mutex) becomes the manager's DFU partition — `mgr.dfu_partition()`
/// returns it — while the state and storage partitions stay on the internal
/// flash.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
pub(crate) fn init_flash_with_layout_with_external_dfu<M: RawMutex + Sync, DFU: NorFlash + Send>(
    flash: FlashDriver,
    dfu_flash: &'static Mutex<M, RefCell<DFU>>,
) -> PartitionType {
    let capacity = dfu_flash.lock(|cell| cell.borrow().capacity()) as u32;
    assert!(
        capacity % DFU_ERASE_SIZE as u32 == 0,
        "[dfu] external flash capacity ({}) must be a multiple of the DFU erase size ({})",
        capacity,
        DFU_ERASE_SIZE
    );
    let dfu_part = DfuPartition::External(ExternalDfuPartition {
        ops: dfu_flash,
        size: capacity,
    });
    let [storage_offset, storage_size, state_offset, state_size, ..] = read_boot_layout();
    let flash_mutex: &'static MutexType = FLASH_CELL.init(Mutex::new(RefCell::new(flash)));
    let mgr = DfuFlashManager::new(
        flash_mutex,
        storage_offset,
        storage_size,
        state_offset,
        state_size,
        dfu_part,
    );
    let partition = mgr.storage_partition();
    MANAGER.init(mgr).ok();
    partition
}

// ---------------------------------------------------------------------------
// dfu_split sub-module (behind feature flag)
// ---------------------------------------------------------------------------
// register_dfu_interface — register DFU interface with central + passthrough alts
// ---------------------------------------------------------------------------

/// Register a DFU interface on the USB builder.
///
/// Alt 0 is the central's own DFU flash — the external one if the manager was
/// built by [`init_flash_from_linkerscript_with_external_dfu`], otherwise the
/// internal DFU partition.
/// Alt 1..N are passthrough slots for split peripherals (requires `dfu_split`).
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
pub fn register_dfu_interface<D: Driver<'static>>(
    builder: &mut Builder<'static, D>,
    mgr: &'static DfuFlashManager,
    product_name: &'static str,
    #[cfg(feature = "dfu_split")] num_peripherals: usize,
) {
    use embassy_usb::class::dfu::consts::DfuAttributes;

    let central_attrs = DfuAttributes::CAN_DOWNLOAD | DfuAttributes::WILL_DETACH;
    let central_attrs_bits = central_attrs.bits();

    // Alt 0: Central DFU state.
    static CENTRAL: StaticCell<
        DfuState<RmkDfuHandler<FirmwareHandler<'static, DfuPartition, PartitionType, ResetImmediate, BLOCK_SIZE_DFU>>>,
    > = StaticCell::new();
    let central: &'static mut DfuState<
        RmkDfuHandler<FirmwareHandler<'static, DfuPartition, PartitionType, ResetImmediate, BLOCK_SIZE_DFU>>,
    > = CENTRAL.init(build_dfu_state(mgr.dfu_partition(), mgr.state_partition()));

    // Alt 1..N: Passthrough
    #[cfg(feature = "dfu_split")]
    let passthrough_count = num_peripherals.min(MAX_PASSTHROUGH_ALTS);
    #[cfg(feature = "dfu_split")]
    let passthrough = {
        let mut arr: [Option<DfuState<RmkDfuHandler<PassthroughDfuHandler>>>; MAX_PASSTHROUGH_ALTS] =
            Default::default();
        for id in 0..passthrough_count {
            let state = DfuState::new(
                RmkDfuHandler {
                    inner: PassthroughDfuHandler {
                        target_id: id,
                        written: 0,
                    },
                    target_id: Some(id),
                },
                DfuAttributes::CAN_DOWNLOAD,
            );
            arr[id] = Some(state);
        }
        arr
    };

    let string_idx = builder.string();

    let mut func = builder.function(0x00, 0x00, 0x00);
    let mut iface = func.interface();
    let mut alt = iface.alt_setting(0xFE, 0x01, 0x02, Some(string_idx));
    alt.descriptor(
        0x21,
        &[
            central_attrs_bits,
            0xc4,
            0x09,
            (BLOCK_SIZE_DFU & 0xff) as u8,
            ((BLOCK_SIZE_DFU >> 8) & 0xff) as u8,
            0x10,
            0x01,
        ],
    );

    #[cfg(feature = "dfu_split")]
    for _ in 0..passthrough_count {
        let mut alt = iface.alt_setting(0xFE, 0x01, 0x02, Some(string_idx));
        alt.descriptor(
            0x21,
            &[
                DfuAttributes::CAN_DOWNLOAD.bits(),
                0xc4,
                0x09,
                (BLOCK_SIZE_DFU & 0xff) as u8,
                ((BLOCK_SIZE_DFU >> 8) & 0xff) as u8,
                0x10,
                0x01,
            ],
        );
    }
    drop(func);

    let iface_ref = RMK_DFU_INTERFACE.init(RmkDfuInterface {
        central: Some(central),
        #[cfg(feature = "dfu_split")]
        passthrough,
        #[cfg(feature = "dfu_split")]
        num_passthrough: passthrough_count,
        current_alt: 0,
    });
    builder.handler(iface_ref);

    static STRING_PROVIDER: StaticCell<DfuStringProvider> = StaticCell::new();
    let string_provider = STRING_PROVIDER.init(DfuStringProvider {
        string_idx,
        string_val: product_name,
    });
    builder.handler(string_provider);
}

// ---------------------------------------------------------------------------
// dfu_lock
// ---------------------------------------------------------------------------

/// DfuLock state machine that checks a physical key combination to unlock DFU.
#[cfg(feature = "dfu_lock")]
pub struct DfuLock<'a> {
    unlocked: AtomicBool,
    unlock_keys: &'a [(u8, u8)],
    keymap: &'a crate::keymap::KeyMap<'a>,
}

#[cfg(feature = "dfu_lock")]
impl<'a> DfuLock<'a> {
    pub fn new(unlock_keys: &'a [(u8, u8)], keymap: &'a crate::keymap::KeyMap<'a>) -> Self {
        Self {
            unlocked: AtomicBool::new(false),
            unlock_keys,
            keymap,
        }
    }

    pub(crate) async fn process_unlock(&self) {
        DFU_UNLOCK_SIGNAL.wait().await;

        info!("dfu_lock: DFU activity detected, unlock window open for 10 s");
        info!("dfu_lock: waiting for unlock keys");
        publish_event(crate::event::DfuStatusEvent::new(DfuStatus::LockWaiting));
        let deadline = embassy_time::Instant::now() + embassy_time::Duration::from_secs(10);
        loop {
            let all_pressed = self
                .unlock_keys
                .iter()
                .all(|(row, col)| self.keymap.read_matrix_key(*row, *col));
            if all_pressed {
                self.unlocked.store(true, Ordering::Release);
                DFU_LOCKED.store(false, Ordering::Release);
                info!("dfu_lock: unlock keys pressed, DFU unlocked for 10 s");
                publish_event(crate::event::DfuStatusEvent::new(DfuStatus::LockUnlocked));
                break;
            }
            if embassy_time::Instant::now() >= deadline {
                info!("dfu_lock: unlock window expired (10 s timeout)");
                DFU_LOCKED.store(true, Ordering::Release);
                publish_event(crate::event::DfuStatusEvent::new(DfuStatus::Idle));
                return;
            }
            embassy_time::Timer::after_millis(50).await;
        }

        info!("dfu_lock: unlocked, waiting for DFU download");
        let deadline = embassy_time::Instant::now() + embassy_time::Duration::from_secs(10);
        loop {
            if DFU_STARTED.load(Ordering::Acquire) {
                info!("dfu_lock: DFU download started, staying unlocked");
                break;
            }
            if embassy_time::Instant::now() >= deadline {
                info!("dfu_lock: unlock expired (10 s timeout)");
                DFU_LOCKED.store(true, Ordering::Release);
                self.unlocked.store(false, Ordering::Release);
                publish_event(crate::event::DfuStatusEvent::new(DfuStatus::Idle));
                break;
            }
            embassy_time::Timer::after_millis(200).await;
        }
    }
}

#[cfg(feature = "dfu_lock")]
impl<'a> Runnable for DfuLock<'a> {
    async fn run(&mut self) -> ! {
        loop {
            self.process_unlock().await;
        }
    }
}
