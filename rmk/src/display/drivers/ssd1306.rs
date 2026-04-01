//! [`DisplayDriver`] implementation for SSD1306 OLED displays.

use display_interface::AsyncWriteOnlyDataCommand;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use ssd1306::mode::{BufferedGraphicsModeAsync, DisplayConfigAsync};
use ssd1306::size::DisplaySizeAsync;

use super::super::DisplayDriver;

impl<DI, SIZE> DisplayDriver for ssd1306::Ssd1306Async<DI, SIZE, BufferedGraphicsModeAsync<SIZE>>
where
    DI: AsyncWriteOnlyDataCommand,
    SIZE: DisplaySizeAsync,
    Self: DrawTarget<Color = BinaryColor> + DisplayConfigAsync,
{
    async fn init(&mut self) {
        DisplayConfigAsync::init(self).await.ok();
    }

    async fn flush(&mut self) {
        ssd1306::Ssd1306Async::flush(self).await.ok();
    }
}
