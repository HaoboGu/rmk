use core::convert::Infallible;

use embedded_hal::digital::{OutputPin, PinState};

/// Lighting in keyboard
///
/// Two types of light: single color LED, RGB LED(ws2812).
///
/// Three usages of LEDs: single/matrix/underglow(RGB only).
trait LED {
    /// Turn LED on
    fn on(&mut self) -> Result<(), Infallible>;

    /// Turn LED off
    fn off(&mut self) -> Result<(), Infallible>;

    /// Set LED's brightness
    fn set_brightness(&mut self, brightness: u8) -> Result<(), Infallible>;
}

/// A single LED
///
/// In general, a single LED can be used for capslock/numslock, or in a LED matrix.
/// TODO: need separate LED abstraction with hardware like GPIO/PWM control?
struct SingleLED<LedPin: OutputPin<Error = Infallible>> {
    /// On/Off state
    state: bool,

    /// Pin state when turning LED on
    on_state: PinState,

    /// GPIO for controlling the LED
    pin: LedPin,

    /// Brightness level, range: 0 ~ 255
    brightness: u8,

    // The duration in seconds of a LED breathing period
    period: u8,
}

impl<LedPin: OutputPin<Error = Infallible>> LED for SingleLED<LedPin> {
    /// Turn LED on
    fn on(&mut self) -> Result<(), Infallible> {
        self.pin.set_state(self.on_state)
    }

    /// Turn LED off
    fn off(&mut self) -> Result<(), Infallible> {
        self.pin.set_state(!self.on_state)
    }

    fn set_brightness(&mut self, brightness: u8) -> Result<(), Infallible> {
        self.brightness = brightness;

        // TODO: Write brightness to LED

        Ok(())
    }
}
