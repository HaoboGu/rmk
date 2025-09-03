use embedded_hal::digital::{ErrorType, InputPin, OutputPin};

#[cfg(feature = "rp2040_bl")]
pub mod flex_pin_rp;

/// Pin that can be switched between input and output.
pub trait FlexPin: ErrorType + InputPin + OutputPin {
    fn set_as_input(&mut self) -> ();

    fn set_as_output(&mut self) -> ();
}
