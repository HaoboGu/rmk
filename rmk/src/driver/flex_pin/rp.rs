use embassy_rp::gpio::{Flex, Pull, Level};

use crate::driver::flex_pin::FlexPin;

impl<'d> FlexPin for Flex<'d> {
    fn set_as_input(&mut self) {
        self.set_as_input();
        self.set_pull(Pull::Down);
    }

    fn set_as_output(&mut self) {
        self.set_level(Level::Low);
        self.set_as_output();
    }
}