use embassy_rp::gpio::Flex;

use crate::driver::flex_pin::FlexPin;

impl<'d> FlexPin for Flex<'d> {
    fn set_as_input(&mut self) {
        self.set_as_input();
    }

    fn set_as_output(&mut self) {
        self.set_as_output();
    }
}