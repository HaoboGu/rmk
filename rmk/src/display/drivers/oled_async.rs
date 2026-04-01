//! [`DisplayDriver`] implementation for displays supported by the `oled_async` crate
//! (SH1106, SH1107, SH1108, SSD1309).

use display_interface::AsyncWriteOnlyDataCommand;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use oled_async::display::DisplayVariant;
use oled_async::mode::graphics::GraphicsMode;

use super::super::DisplayDriver;

impl<DV, DI, const BS: usize> DisplayDriver for GraphicsMode<DV, DI, BS>
where
    DI: AsyncWriteOnlyDataCommand,
    DV: DisplayVariant,
    Self: DrawTarget<Color = BinaryColor>,
{
    async fn init(&mut self) {
        GraphicsMode::init(self).await.ok();
    }

    async fn flush(&mut self) {
        GraphicsMode::flush(self).await.ok();
    }
}
