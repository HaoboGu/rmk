//! [`DisplayDriver`] adapter for displays driven by the [`lcd-async`](https://crates.io/crates/lcd-async) crate.
//!
//! # Example
//!
//! ```rust,ignore
//! use lcd_async::{Builder, models::GC9107};
//! use rmk::display::{DisplayProcessor, drivers::lcd_async::LcdAsyncDisplay};
//! use static_cell::StaticCell;
//!
//! const W: usize = 128;
//! const H: usize = 128;
//!
//! static FB: StaticCell<[u8; W * H * 2]> = StaticCell::new();
//! let fb = FB.init([0; W * H * 2]);
//!
//! let display = Builder::new(GC9107, my_interface)
//!     .display_size(W as u16, H as u16)
//!     .init(&mut embassy_time::Delay).await.unwrap();
//!
//! let mut processor =
//!     DisplayProcessor::new(LcdAsyncDisplay::<_, _, _, _, W, H>::new(display, fb));
//! ```

use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use embedded_hal::digital::OutputPin;
use lcd_async::Display;
use lcd_async::interface::Interface;
use lcd_async::models::Model;
use lcd_async::raw_framebuf::RawFrameBuf;

use super::super::DisplayDriver;

/// Bridges an [`lcd_async::Display`] plus a software framebuffer into RMK's
/// [`DisplayDriver`] trait.
///
/// # Generics
///
/// - `DI` — bus interface (`Word = u8`).
/// - `MOD` — chip model from [`lcd_async::models`] using the `Rgb565` color format.
/// - `RST` — reset pin (use [`lcd_async::NoResetPin`] when reset is handled out-of-band).
/// - `BUF` — framebuffer storage of length `W * H * 2`. Typically `&'static mut [u8; W * H * 2]`.
/// - `W` / `H` — display resolution in pixels.
///
/// # Lifecycle
///
/// The wrapped [`Display`] must already be initialized by [`lcd_async::Builder::init`]
/// before being passed to [`new`](Self::new). [`DisplayDriver::init`] is therefore a no-op.
pub struct LcdAsyncDisplay<DI, MOD, RST, BUF, const W: usize, const H: usize>
where
    DI: Interface<Word = u8>,
    MOD: Model<ColorFormat = Rgb565>,
    RST: OutputPin,
    BUF: AsMut<[u8]> + AsRef<[u8]>,
{
    display: Display<DI, MOD, RST>,
    buffer: BUF,
}

impl<DI, MOD, RST, BUF, const W: usize, const H: usize> LcdAsyncDisplay<DI, MOD, RST, BUF, W, H>
where
    DI: Interface<Word = u8>,
    MOD: Model<ColorFormat = Rgb565>,
    RST: OutputPin,
    BUF: AsMut<[u8]> + AsRef<[u8]>,
{
    /// Wrap an already-initialized [`Display`] and its framebuffer storage.
    ///
    /// `buffer` length must be exactly `W * H * 2` bytes (Rgb565 = 2 bytes/pixel).
    pub fn new(display: Display<DI, MOD, RST>, buffer: BUF) -> Self {
        debug_assert_eq!(
            buffer.as_ref().len(),
            W * H * 2,
            "framebuffer length must equal W * H * 2 (Rgb565)",
        );
        Self { display, buffer }
    }

    /// Borrow the underlying [`Display`].
    pub fn display(&mut self) -> &mut Display<DI, MOD, RST> {
        &mut self.display
    }
}

impl<DI, MOD, RST, BUF, const W: usize, const H: usize> DisplayDriver for LcdAsyncDisplay<DI, MOD, RST, BUF, W, H>
where
    DI: Interface<Word = u8>,
    MOD: Model<ColorFormat = Rgb565>,
    RST: OutputPin,
    BUF: AsMut<[u8]> + AsRef<[u8]>,
{
    async fn init(&mut self) {}

    async fn flush(&mut self) {
        let _ = self
            .display
            .show_raw_data(0, 0, W as u16, H as u16, self.buffer.as_ref())
            .await;
    }
}

impl<DI, MOD, RST, BUF, const W: usize, const H: usize> OriginDimensions for LcdAsyncDisplay<DI, MOD, RST, BUF, W, H>
where
    DI: Interface<Word = u8>,
    MOD: Model<ColorFormat = Rgb565>,
    RST: OutputPin,
    BUF: AsMut<[u8]> + AsRef<[u8]>,
{
    fn size(&self) -> Size {
        Size::new(W as u32, H as u32)
    }
}

impl<DI, MOD, RST, BUF, const W: usize, const H: usize> DrawTarget for LcdAsyncDisplay<DI, MOD, RST, BUF, W, H>
where
    DI: Interface<Word = u8>,
    MOD: Model<ColorFormat = Rgb565>,
    RST: OutputPin,
    BUF: AsMut<[u8]> + AsRef<[u8]>,
{
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Rgb565>>,
    {
        let mut fb = RawFrameBuf::<Rgb565, _>::new(self.buffer.as_mut(), W, H);
        fb.draw_iter(pixels).ok();
        Ok(())
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Rgb565>,
    {
        let mut fb = RawFrameBuf::<Rgb565, _>::new(self.buffer.as_mut(), W, H);
        fb.fill_contiguous(area, colors).ok();
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Rgb565) -> Result<(), Self::Error> {
        let mut fb = RawFrameBuf::<Rgb565, _>::new(self.buffer.as_mut(), W, H);
        fb.fill_solid(area, color).ok();
        Ok(())
    }

    fn clear(&mut self, color: Rgb565) -> Result<(), Self::Error> {
        let mut fb = RawFrameBuf::<Rgb565, _>::new(self.buffer.as_mut(), W, H);
        fb.clear(color).ok();
        Ok(())
    }
}
