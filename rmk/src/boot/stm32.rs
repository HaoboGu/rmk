//! STM32 ROM bootloader (DFU) support for the `Bootloader` keycode.

/// Request handoff from [`request_bootloader`] to the next boot: the magic and its
/// complement.
///
/// `.uninit` keeps cortex-m-rt startup from zeroing it, so it survives the reset.
#[unsafe(link_section = ".uninit.RMK_BOOTLOADER_REQUEST")]
static mut BOOTLOADER_REQUEST: core::mem::MaybeUninit<[u32; 2]> = core::mem::MaybeUninit::uninit();

/// Same magic QMK's `stm32_dfu.c` uses
const BOOTLOADER_REQUEST_MAGIC: u32 = 0xDEAD_BEEF;

fn request_ptr() -> *mut u32 {
    (&raw mut BOOTLOADER_REQUEST) as *mut u32
}

/// Complement of `x`, computed in asm.
///
/// As a constant it would sit next to the magic in the literal pool, putting a valid
/// pair in the firmware image. The ROM bootloader buffers the image in RAM during DFU,
/// which can leave that pair in the request words and send the board straight back into
/// DFU.
fn opaque_complement(x: u32) -> u32 {
    let out;
    unsafe { core::arch::asm!("mvns {o}, {i}", o = lateout(reg) out, i = in(reg) x, options(pure, nomem, nostack)) };
    out
}

/// Writes the request words back to SRAM if the D-cache is on.
///
/// A system reset invalidates the D-cache without writing it back. `DC` reads 0 where
/// there is no cache.
fn clean_request_dcache() {
    const SCB_CCR: *const u32 = 0xE000_ED14 as *const u32;
    const SCB_CCR_DC: u32 = 1 << 16;
    const CBP_DCCMVAC: *mut u32 = 0xE000_EF68 as *mut u32;

    if unsafe { core::ptr::read_volatile(SCB_CCR) } & SCB_CCR_DC == 0 {
        return;
    }

    // One clean per word, the pair can span two cache lines
    let p = request_ptr();
    cortex_m::asm::dsb();
    unsafe {
        core::ptr::write_volatile(CBP_DCCMVAC, p as u32);
        core::ptr::write_volatile(CBP_DCCMVAC, p.add(1) as u32);
    }
    cortex_m::asm::dsb();
}

/// Stores a bootloader request and resets the chip, mirroring `bootloader_jump` in
/// QMK's `stm32_dfu.c`.
///
/// [`enter_bootloader_if_requested`] consumes the request on the next boot, so the
/// reset that leaves DFU (dfu-util's `:leave`) boots the firmware.
///
/// Registered by the `rmk_keyboard` macro for known STM32 chips, otherwise pass it to
/// [`register_bootloader_jump`](super::register_bootloader_jump).
pub fn request_bootloader() -> ! {
    let p = request_ptr();
    unsafe {
        core::ptr::write_volatile(p, BOOTLOADER_REQUEST_MAGIC);
        core::ptr::write_volatile(p.add(1), opaque_complement(BOOTLOADER_REQUEST_MAGIC));
    }
    clean_request_dcache();
    cortex_m::peripheral::SCB::sys_reset()
}

/// Jumps to the ROM bootloader if the last reset came from [`request_bootloader`],
/// consuming the request either way.
///
/// `system_memory` is the chip's system memory base address from AN2606, `0x1FFF_0000`
/// on STM32F4.
///
/// Must run before `embassy_stm32::init`, while the chip is still in its reset state.
/// Emitted by the `rmk_keyboard` macro for known STM32 chips.
pub fn enter_bootloader_if_requested(system_memory: u32) {
    let p = request_ptr();
    let (w0, w1) = unsafe { (core::ptr::read_volatile(p), core::ptr::read_volatile(p.add(1))) };

    // Consume unconditionally, a stale or partial pair must not reach a later boot
    unsafe {
        core::ptr::write_volatile(p, 0);
        core::ptr::write_volatile(p.add(1), 0);
    }

    if w0 != BOOTLOADER_REQUEST_MAGIC || w1 != opaque_complement(BOOTLOADER_REQUEST_MAGIC) {
        return;
    }

    // Don't mask interrupts: the ROM bootloader expects PRIMASK in its reset state and
    // never clears it, so masking here stops USB enumeration. Nothing is enabled this
    // early anyway
    unsafe { cortex_m::asm::bootload(system_memory as *const u32) }
}
