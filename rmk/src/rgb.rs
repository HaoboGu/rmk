use smart_leds::{
    hsv::{hsv2rgb, Hsv},
    SmartLedsWrite,
};

pub struct RgbDriver<const NUM_LED: usize> {
    buf: [Hsv; NUM_LED],
}

impl<const NUM_LED: usize> RgbDriver<NUM_LED> {
    fn write_byte(&mut self, _byte: u8) {
        todo!("write byte to ws2812")
    }
}

impl<const NUM_LED: usize> SmartLedsWrite for RgbDriver<NUM_LED> {
    type Error = ();

    type Color = Hsv;

    /// Write all the items of an iterator to a ws2812 strip
    fn write<T, I>(&mut self, iterator: T) -> Result<(), Self::Error>
    where
        T: IntoIterator<Item = I>,
        I: Into<Self::Color>,
    {
        for item in iterator {
            let item = hsv2rgb(item.into());
            self.write_byte(item.r);
            self.write_byte(item.g);
            self.write_byte(item.b);
        }

        // self.flush();

        Ok(())
    }
}
