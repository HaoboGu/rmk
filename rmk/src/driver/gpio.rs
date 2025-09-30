use embedded_hal::digital::StatefulOutputPin;

/// The gpio driver is a wrapper for the embedded-hal digital output pin trait.
/// It wraps the low-active and high-active pins, and provides a way to set the pin state
pub(crate) struct OutputController<P: StatefulOutputPin> {
    pin: P,
    low_active: bool,
}

impl<P: StatefulOutputPin> OutputController<P> {
    /// Create a new OutputController instance
    pub fn new(pin: P, low_active: bool) -> Self {
        Self { pin, low_active }
    }

    /// Activate the GPIO pin
    pub fn activate(&mut self) {
        if self.low_active {
            self.pin.set_low().ok();
        } else {
            self.pin.set_high().ok();
        }
    }

    /// Deactivate the GPIO pin
    pub fn deactivate(&mut self) {
        if self.low_active {
            self.pin.set_high().ok();
        } else {
            self.pin.set_low().ok();
        }
    }

    /// Toggle the GPIO pin state
    pub fn toggle(&mut self) {
        self.pin.toggle().ok();
    }

    /// Check if the GPIO pin is active
    pub fn is_active(&mut self) -> Result<bool, P::Error> {
        if self.low_active {
            self.pin.is_set_low()
        } else {
            self.pin.is_set_high()
        }
    }
}
