use embassy_nrf::gpio::{Flex, Level, OutputDrive, Pull};

use crate::driver::flex_pin::FlexPin;

impl<'d> FlexPin for Flex<'d> {
    fn set_as_input(&mut self) {
        self.set_as_input(Pull::Down);
    }

    fn set_as_output(&mut self) {
        self.set_level(Level::Low);
        self.set_as_output(OutputDrive::Standard);
    }
}