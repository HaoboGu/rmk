//! LCDC1 SPI bus + lcd-async `Interface` adapter for the SuperKey 3-screen module.
//!
//! sifli-hal's stock `Lcdc::new_qspi` constructor reserves PA07 / PA08 for the
//! LCDC's DIO2 / DIO3 alt-functions, but PA07 is wired to the LCD 3V3 enable
//! GPIO on this board. The displays are driven in 4-line single-data SPI mode
//! (DIO0 = data, DIO1 = DCX), so DIO2 / DIO3 are unused — we configure the
//! LCDC1 peripheral via the PAC directly and skip those pins.
//!
//! Pin map (matches `SiFliSparks/SuperKey`):
//!  - PA00 — RST (shared, GPIO)
//!  - PA01 — CS for screen 3 (GPIO)
//!  - PA02 — CS for screen 2 (GPIO)
//!  - PA03 — CS for screen 1 (GPIO)
//!  - PA04 — LCDC1_SPI_CLK (alt-1)
//!  - PA05 — LCDC1_SPI_DIO0 — data (alt-1)
//!  - PA06 — LCDC1_SPI_DIO1 — DCX (alt-1)
//!  - PA07 — 3V3_EN (GPIO, drive high)
//!
//! The 3 panels share the LCDC1 bus through an `embassy_sync::Mutex`. Each panel
//! gets its own `LockingLcdcInterface`, which implements `lcd_async::Interface`
//! and snoops CASET (0x2A) / RASET (0x2B) commands so it can program LCDC1's
//! DMA window when `send_data_slice` streams pixels.

use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::mutex::{Mutex, MutexGuard};
use embassy_time::{Duration, Timer};
use embedded_hal::digital::OutputPin;
use lcd_async::interface::{Interface, InterfaceKind};
use sifli_hal::gpio::Output;
use sifli_hal::pac;
use sifli_hal::pac::lcdc::vals::{
    AlphaSel, LayerFormat, LcdFormat, LcdIntfSel, Polarity, SingleAccessType, SpiAccessLen, SpiClkInit, SpiClkPol,
    SpiLcdFormat, SpiLineMode, SpiRdMode, TargetLcd,
};
use sifli_hal::peripherals::LCDC1;
use sifli_hal::rcc::enable_and_reset;

/// Display panel resolution.
pub const LCD_W: u16 = 128;
pub const LCD_H: u16 = 128;

/// Number of pixel bytes in one display framebuffer (RGB565).
pub const FB_BYTES: usize = (LCD_W as usize) * (LCD_H as usize) * 2;

const PIN_AF_LCDC1_SPI: u8 = 1;

/// 4-byte-aligned framebuffer storage so LCDC1's layer-0 DMA can stream from it.
#[repr(align(4))]
pub struct AlignedFb(pub [u8; FB_BYTES]);

impl AlignedFb {
    pub const fn new() -> Self {
        Self([0; FB_BYTES])
    }
}

impl Default for AlignedFb {
    fn default() -> Self {
        Self::new()
    }
}

impl AsRef<[u8]> for AlignedFb {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for AlignedFb {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

/// LCDC1 SPI bus driving the 3 displays. CS is handled externally.
pub struct LcdcBus {
    _peri: LCDC1,
}

impl LcdcBus {
    /// Initialize LCDC1 in 4-line single-data SPI mode.
    ///
    /// Consumes the `LCDC1` peripheral and the `PA4`/`PA5`/`PA6` pin singletons
    /// (CLK / DIO0 / DIO1) so they cannot be repurposed afterwards.
    pub fn new(
        peri: LCDC1,
        _clk: sifli_hal::peripherals::PA4,
        _dio0: sifli_hal::peripherals::PA5,
        _dio1: sifli_hal::peripherals::PA6,
    ) -> Self {
        enable_and_reset::<LCDC1>();

        let pinmux = pac::HPSYS_PINMUX;
        // PA04 → LCDC1_SPI_CLK, PA05 → LCDC1_SPI_DIO0, PA06 → LCDC1_SPI_DIO1
        // No pull, drive strength DS1+DS0 high so the SPI line meets ~30 MHz
        // edges without ringing across the shared bus to all 3 displays.
        for pin in [4, 5, 6] {
            pinmux.pad_pa0_38(pin).modify(|w| {
                w.set_fsel(PIN_AF_LCDC1_SPI);
                w.set_pe(false);
                w.set_ds0(true);
                w.set_ds1(true);
            });
        }

        let regs = pac::LCDC1;
        regs.setting().modify(|w| w.set_auto_gate_en(true));

        regs.lcd_conf().modify(|w| {
            w.set_lcd_intf_sel(LcdIntfSel::Spi);
            w.set_target_lcd(TargetLcd::LcdPanel0);
            w.set_lcd_format(LcdFormat::Rgb565);
            w.set_spi_lcd_format(SpiLcdFormat::Rgb565);
        });

        regs.spi_if_conf().modify(|w| {
            w.set_line(SpiLineMode::FourLine);
            w.set_spi_cs_pol(Polarity::ActiveLow);
            w.set_spi_clk_pol(SpiClkPol::Normal);
            w.set_spi_clk_init(SpiClkInit::Low);
            w.set_spi_clk_auto_dis(true);
            w.set_spi_cs_no_idle(true);
            w.set_dummy_cycle(0);
            w.set_clk_div(8); // ~30 MHz at 240 MHz HCLK
        });

        regs.te_conf().write(|w| w.set_enable(false));
        regs.lcd_if_conf().modify(|w| w.set_lcd_rstb(true));

        Self { _peri: peri }
    }

    pub fn wait_busy(&self) {
        let regs = pac::LCDC1;
        while regs.status().read().lcd_busy() || regs.lcd_single().read().lcd_busy() {}
    }

    fn wait_single_busy(&self) {
        while pac::LCDC1.lcd_single().read().lcd_busy() {}
    }

    /// Send an 8-bit command byte.
    pub fn write_cmd(&mut self, cmd: u8) {
        self.wait_busy();
        let regs = pac::LCDC1;
        regs.spi_if_conf().modify(|w| {
            w.set_spi_rd_mode(SpiRdMode::Normal);
            w.set_spi_cs_auto_dis(true);
            w.set_wr_len(SpiAccessLen::Bytes1);
        });
        regs.lcd_wr().write(|w| w.set_data(cmd as u32));
        regs.lcd_single().write(|w| {
            w.set_wr_trig(true);
            w.set_type_(SingleAccessType::Command);
        });
    }

    /// Send a single 8-bit data parameter.
    pub fn write_data(&mut self, byte: u8) {
        self.wait_single_busy();
        let regs = pac::LCDC1;
        regs.spi_if_conf().modify(|w| {
            w.set_spi_cs_auto_dis(true);
            w.set_wr_len(SpiAccessLen::Bytes1);
        });
        regs.lcd_wr().write(|w| w.set_data(byte as u32));
        regs.lcd_single().write(|w| {
            w.set_wr_trig(true);
            w.set_type_(SingleAccessType::Data);
        });
    }

    /// Stream a pixel buffer via the LCDC layer-0 DMA.
    ///
    /// The buffer must be 4-byte aligned and contain RGB565 pixels in big-endian
    /// order (high byte first), matching the display's expected wire format.
    pub async fn write_pixels(&mut self, x0: u16, y0: u16, x1: u16, y1: u16, buffer: &[u8]) {
        debug_assert!(
            (buffer.as_ptr() as usize).is_multiple_of(4),
            "framebuffer must be 4-byte aligned"
        );

        unsafe {
            let mut cp = cortex_m::Peripherals::steal();
            cp.SCB.clean_dcache_by_slice(buffer);
        }

        self.wait_busy();

        let regs = pac::LCDC1;
        let width = x1 - x0 + 1;

        regs.canvas_tl_pos().write(|w| {
            w.set_x0(x0);
            w.set_y0(y0);
        });
        regs.canvas_br_pos().write(|w| {
            w.set_x1(x1);
            w.set_y1(y1);
        });

        regs.layer0_config().write(|w| {
            w.set_active(true);
            w.set_format(LayerFormat::RGB565);
            w.set_alpha(255);
            w.set_alpha_sel(AlphaSel::Layer);
            w.set_prefetch_en(true);
            w.set_v_mirror(false);
            w.set_width(width * 2);
        });

        regs.layer0_tl_pos().write(|w| {
            w.set_x0(x0);
            w.set_y0(y0);
        });
        regs.layer0_br_pos().write(|w| {
            w.set_x1(x1);
            w.set_y1(y1);
        });

        regs.spi_if_conf().modify(|w| w.set_spi_cs_auto_dis(true));

        let addr = sifli_hal::to_system_bus_addr(buffer.as_ptr() as usize) as u32;
        regs.layer0_src().write(|w| w.set_addr(addr));

        // Clear status flags
        regs.irq().write(|w| {
            w.set_eof_stat(true);
            w.set_dpi_udr_stat(true);
            w.set_icb_of_stat(true);
        });

        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

        regs.command().write(|w| w.set_start(true));

        // Poll for EOF, yielding so other Embassy tasks can run.
        while !pac::LCDC1.irq().read().eof_raw_stat() {
            embassy_futures::yield_now().await;
        }
        pac::LCDC1.irq().write(|w| w.set_eof_stat(true));
    }
}

/// One-shot LCD power + reset pulse.
///
/// Power is driven high by the caller before this runs (the `lcd_power` pin is
/// initialized at `Level::High`). Pulsing reset wakes all 3 GC9107 panels at
/// once; after this, the per-panel `lcd-async` `Builder::init` runs the chip
/// init sequence sequentially under the shared bus mutex.
pub async fn power_cycle(rst: &mut Output<'_>) {
    Timer::after(Duration::from_millis(20)).await;
    rst.set_low();
    Timer::after(Duration::from_millis(20)).await;
    rst.set_high();
    Timer::after(Duration::from_millis(120)).await;
}

/// `lcd-async` [`Interface`] adapter that owns one CS pin and shares an
/// `LcdcBus` through a mutex.
///
/// The GC9107 expects the entire `CASET → RASET → RAMWR → pixels` sequence
/// inside a single CS-low window: deasserting CS between RAMWR and the pixel
/// stream causes the panel to drop the address window, and pixels land in the
/// default `(0, 0)` location — visible as a "shifted" image.
///
/// To match the working SuperKey C driver behaviour, this adapter acquires the
/// bus mutex and asserts CS low when CASET (`0x2A`) arrives, holds them across
/// RASET/RAMWR, and releases them only at the end of the following
/// `send_data_slice`. Standalone commands (init sequence, `0x29`, etc.) lock
/// and release per-call as before.
///
/// CASET/RASET are also snooped so `send_data_slice` can program LCDC1's
/// hardware DMA window — the chip and the LCDC must agree on the active rect.
pub struct LockingLcdcInterface<'a, M, CS>
where
    M: RawMutex,
    CS: OutputPin,
{
    bus: &'a Mutex<M, LcdcBus>,
    cs: CS,
    /// Last address window programmed via CASET/RASET, in (x0, y0, x1, y1) order.
    window: (u16, u16, u16, u16),
    /// Mutex guard held across a `CASET → RASET → RAMWR → pixels` sequence.
    /// `Some` once CASET is received, `None` again after `send_data_slice`.
    held: Option<MutexGuard<'a, M, LcdcBus>>,
}

impl<'a, M, CS> LockingLcdcInterface<'a, M, CS>
where
    M: RawMutex,
    CS: OutputPin,
{
    pub fn new(bus: &'a Mutex<M, LcdcBus>, cs: CS) -> Self {
        Self {
            bus,
            cs,
            window: (0, 0, LCD_W - 1, LCD_H - 1),
            held: None,
        }
    }
}

impl<'a, M, CS> Interface for LockingLcdcInterface<'a, M, CS>
where
    M: RawMutex,
    CS: OutputPin,
{
    type Word = u8;
    type Error = core::convert::Infallible;
    const KIND: InterfaceKind = InterfaceKind::Serial4Line;

    async fn send_command(&mut self, command: u8, args: &[u8]) -> Result<(), Self::Error> {
        // Snoop CASET / RASET coordinates for LCDC1's DMA window. Both carry 4
        // big-endian bytes: start_hi, start_lo, end_hi, end_lo.
        match command {
            0x2A if args.len() >= 4 => {
                self.window.0 = u16::from_be_bytes([args[0], args[1]]);
                self.window.2 = u16::from_be_bytes([args[2], args[3]]);
            }
            0x2B if args.len() >= 4 => {
                self.window.1 = u16::from_be_bytes([args[0], args[1]]);
                self.window.3 = u16::from_be_bytes([args[2], args[3]]);
            }
            _ => {}
        }

        // CASET (0x2A) starts a pixel-write sequence. Acquire the bus + CS once
        // and hold both across RASET and RAMWR.
        if command == 0x2A && self.held.is_none() {
            self.held = Some(self.bus.lock().await);
            let _ = self.cs.set_low();
        }

        if let Some(bus) = self.held.as_mut() {
            // Inside a held sequence — use the existing guard, do not toggle CS.
            bus.write_cmd(command);
            for &b in args {
                bus.write_data(b);
            }
            bus.wait_busy();
        } else {
            // Standalone command (init, sleep-out, display-on, etc.).
            let mut bus = self.bus.lock().await;
            let _ = self.cs.set_low();
            bus.write_cmd(command);
            for &b in args {
                bus.write_data(b);
            }
            bus.wait_busy();
            let _ = self.cs.set_high();
        }

        Ok(())
    }

    async fn send_data_slice(&mut self, data: &[Self::Word]) -> Result<(), Self::Error> {
        let (x0, y0, x1, y1) = self.window;

        // Re-use the guard acquired in CASET if we're inside a pixel sequence;
        // otherwise (defensive) acquire fresh.
        if let Some(bus) = self.held.as_mut() {
            bus.write_pixels(x0, y0, x1, y1, data).await;
        } else {
            let mut bus = self.bus.lock().await;
            let _ = self.cs.set_low();
            bus.write_pixels(x0, y0, x1, y1, data).await;
        }

        let _ = self.cs.set_high();
        // Drop the held guard to release the bus mutex for the next display.
        self.held = None;
        Ok(())
    }
}
