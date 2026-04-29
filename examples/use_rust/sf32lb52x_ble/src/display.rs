//! 3× GC9107 display driver for the SuperKey hardware.
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

use embassy_time::{Duration, Timer};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::pixelcolor::raw::RawU16;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
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

    /// Send `cmd` followed by N data parameters.
    pub fn write_cmd_with(&mut self, cmd: u8, params: &[u8]) {
        self.write_cmd(cmd);
        for &b in params {
            self.write_data(b);
        }
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

/// Three GC9107 displays sharing the LCDC1 SPI bus, addressed by individual CS pins.
pub struct TripleDisplay<'a> {
    pub bus: LcdcBus,
    rst: Output<'a>,
    cs: [Output<'a>; 3],
    _power: Output<'a>,
}

impl<'a> TripleDisplay<'a> {
    /// Power up the LCD rail, hardware-reset every display, then run the GC9107
    /// init sequence simultaneously on all 3 panels.
    pub async fn new(
        bus: LcdcBus,
        rst: Output<'a>,
        cs0: Output<'a>,
        cs1: Output<'a>,
        cs2: Output<'a>,
        power: Output<'a>,
    ) -> Self {
        let mut me = Self {
            bus,
            rst,
            cs: [cs0, cs1, cs2],
            _power: power,
        };
        me.power_cycle().await;
        me.init_all().await;
        me
    }

    async fn power_cycle(&mut self) {
        // Power was already enabled by the caller; just give the rail time to
        // settle before pulsing reset.
        Timer::after(Duration::from_millis(20)).await;
        self.rst.set_low();
        Timer::after(Duration::from_millis(20)).await;
        self.rst.set_high();
        Timer::after(Duration::from_millis(120)).await;
    }

    fn select_one(&mut self, idx: usize) {
        for (i, cs) in self.cs.iter_mut().enumerate() {
            if i == idx {
                cs.set_low();
            } else {
                cs.set_high();
            }
        }
    }

    fn select_all(&mut self) {
        for cs in self.cs.iter_mut() {
            cs.set_low();
        }
    }

    fn deselect_all(&mut self) {
        for cs in self.cs.iter_mut() {
            cs.set_high();
        }
    }

    /// Send `cmd` (with optional params) to ALL 3 panels in a single
    /// transaction. Mirrors `LCD_WriteReg_More` from the SuperKey C SDK:
    /// every chip select goes low, the LCDC clocks the bytes, every CS goes
    /// high once the LCDC reports idle.
    fn write_cmd_all(&mut self, cmd: u8, params: &[u8]) {
        self.select_all();
        self.bus.write_cmd(cmd);
        for &b in params {
            self.bus.write_data(b);
        }
        // Make sure the transaction has fully drained before lifting CS,
        // otherwise the trailing byte can be cut off mid-clock.
        self.bus.wait_busy();
        self.deselect_all();
    }

    /// GC9107 init sequence (lifted from `SiFliSparks/SuperKey` / SiFli SDK).
    async fn init_all(&mut self) {
        self.write_cmd_all(0x11, &[]); // sleep out
        Timer::after(Duration::from_millis(120)).await;

        self.write_cmd_all(0xFE, &[]); // internal reg enable
        self.write_cmd_all(0xEF, &[]); // internal reg enable

        self.write_cmd_all(0xB0, &[0xC0]);
        self.write_cmd_all(0xB1, &[0x80]);
        self.write_cmd_all(0xB2, &[0x27]);
        self.write_cmd_all(0xB3, &[0x13]);
        self.write_cmd_all(0xB6, &[0x19]);
        self.write_cmd_all(0xB7, &[0x05]);
        self.write_cmd_all(0xAC, &[0xC8]);
        self.write_cmd_all(0xAB, &[0x0F]);
        self.write_cmd_all(0x3A, &[0x05]); // 16-bit RGB565
        self.write_cmd_all(0xB4, &[0x04]);
        self.write_cmd_all(0xA8, &[0x08]);
        self.write_cmd_all(0xB8, &[0x08]);
        self.write_cmd_all(0xEA, &[0x02]);
        self.write_cmd_all(0xE8, &[0x2A]);
        self.write_cmd_all(0xE9, &[0x47]);
        self.write_cmd_all(0xE7, &[0x5F]);
        self.write_cmd_all(0xC6, &[0x21]);
        self.write_cmd_all(0xC7, &[0x15]);

        self.write_cmd_all(
            0xF0,
            &[
                0x1D, 0x38, 0x09, 0x4D, 0x92, 0x2F, 0x35, 0x52, 0x1E, 0x0C, 0x04, 0x12, 0x14, 0x1F,
            ],
        );
        self.write_cmd_all(
            0xF1,
            &[
                0x16, 0x40, 0x1C, 0x54, 0xA9, 0x2D, 0x2E, 0x56, 0x10, 0x0D, 0x0C, 0x1A, 0x14, 0x1E,
            ],
        );

        self.write_cmd_all(0xF4, &[0x00, 0x00, 0xFF]);
        self.write_cmd_all(0xBA, &[0xFF, 0xFF]);
        self.write_cmd_all(0x36, &[0x00]); // MADCTL — 0° rotation
        self.write_cmd_all(0x11, &[]); // sleep out (again, matches SDK)
        Timer::after(Duration::from_millis(20)).await;
        self.write_cmd_all(0x29, &[]); // display on
    }

    /// Set the column/row address windows, then start a pixel transfer.
    async fn write_window(&mut self, idx: usize, x0: u16, y0: u16, x1: u16, y1: u16, buffer: &[u8]) {
        self.select_one(idx);
        // CASET
        self.bus.write_cmd_with(
            0x2A,
            &[(x0 >> 8) as u8, (x0 & 0xFF) as u8, (x1 >> 8) as u8, (x1 & 0xFF) as u8],
        );
        // RASET
        self.bus.write_cmd_with(
            0x2B,
            &[(y0 >> 8) as u8, (y0 & 0xFF) as u8, (y1 >> 8) as u8, (y1 & 0xFF) as u8],
        );
        self.bus.write_cmd(0x2C); // RAMWR
        self.bus.wait_busy();
        self.bus.write_pixels(x0, y0, x1, y1, buffer).await;
        self.deselect_all();
    }

    /// Push a full-screen framebuffer to the selected display.
    pub async fn write_frame(&mut self, idx: usize, fb: &[u8]) {
        self.write_window(idx, 0, 0, LCD_W - 1, LCD_H - 1, fb).await;
    }
}

/// Software framebuffer that draws into a 4-byte-aligned RGB565 buffer.
///
/// Implements `embedded_graphics::DrawTarget<Color = Rgb565>` so the standard
/// text/primitive APIs can target it. Pixels are stored big-endian to match
/// the GC9107's wire format.
#[repr(align(4))]
pub struct Framebuffer {
    pub data: [u8; FB_BYTES],
}

impl Framebuffer {
    pub const fn new() -> Self {
        Self { data: [0; FB_BYTES] }
    }

    pub fn fill(&mut self, color: Rgb565) {
        let raw: u16 = RawU16::from(color).into_inner();
        let bytes = raw.to_be_bytes();
        for chunk in self.data.chunks_exact_mut(2) {
            chunk[0] = bytes[0];
            chunk[1] = bytes[1];
        }
    }

    fn put_pixel(&mut self, x: i32, y: i32, color: Rgb565) {
        if x < 0 || y < 0 || x >= LCD_W as i32 || y >= LCD_H as i32 {
            return;
        }
        let idx = (y as usize * LCD_W as usize + x as usize) * 2;
        let raw: u16 = RawU16::from(color).into_inner();
        let bytes = raw.to_be_bytes();
        self.data[idx] = bytes[0];
        self.data[idx + 1] = bytes[1];
    }
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl OriginDimensions for Framebuffer {
    fn size(&self) -> Size {
        Size::new(LCD_W as u32, LCD_H as u32)
    }
}

impl DrawTarget for Framebuffer {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(p, c) in pixels {
            self.put_pixel(p.x, p.y, c);
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let area = area.intersection(&self.bounding_box());
        if area.is_zero_sized() {
            return Ok(());
        }
        let raw: u16 = RawU16::from(color).into_inner();
        let bytes = raw.to_be_bytes();
        let x0 = area.top_left.x as usize;
        let y0 = area.top_left.y as usize;
        let w = area.size.width as usize;
        let h = area.size.height as usize;
        for y in y0..(y0 + h) {
            for x in x0..(x0 + w) {
                let idx = (y * LCD_W as usize + x) * 2;
                self.data[idx] = bytes[0];
                self.data[idx + 1] = bytes[1];
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.fill(color);
        Ok(())
    }
}
