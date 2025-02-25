use embassy_time::Timer;
use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};
use embedded_hal::{digital::OutputPin, spi::SpiBus};
pub use memory_lcd_spi;
use memory_lcd_spi::{
    framebuffer::{FramebufferBW, Sharp},
    DisplaySpec, MemoryLCD,
};

pub struct NiceView;
impl DisplaySpec for NiceView {
    const WIDTH: u16 = 160;
    const HEIGHT: u16 = 68;

    type Framebuffer = FramebufferBW<{ Self::WIDTH }, { Self::HEIGHT }, Sharp>;
}

pub async fn run_display<SPEC, SPI, CS>(mut display: MemoryLCD<SPEC, SPI, CS>)
where
    SPEC: DisplaySpec,
    <<SPEC as DisplaySpec>::Framebuffer as embedded_graphics::draw_target::DrawTarget>::Error:
        core::fmt::Debug,
    <SPEC as DisplaySpec>::Framebuffer: DrawTarget<Color = BinaryColor>,
    SPI: SpiBus,
    CS: OutputPin,
{
    let mut battery = 0;
    let screen_size = display.size();
    loop {
        display.clear(BinaryColor::Off).unwrap();
        battery += 5;
        if battery > 100 {
            battery = 0;
        }
        Rectangle::new(
            Point::new(0, 0),
            Size::new(screen_size.width * battery / 100, screen_size.height),
        )
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(&mut *display)
        .unwrap();
        display.update().unwrap();
        Timer::after_millis(200).await;
    }
}
