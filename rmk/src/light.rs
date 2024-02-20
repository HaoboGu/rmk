use defmt::Format;
///! Lighting in keyboard
///!
///! Two types of light: single color LED, RGB LED(ws2812).
///!
///! Three usages of LEDs: single/matrix/underglow(RGB only).
use embedded_hal::digital::{OutputPin, PinState};
use packed_struct::prelude::*;

#[derive(PackedStruct, Clone, Copy, Debug, Default, Format, Eq, PartialEq)]
#[packed_struct(bit_numbering = "lsb0", size_bytes = "1")]
pub struct LedIndicator {
    #[packed_field(bits = "0")]
    numslock: bool,
    #[packed_field(bits = "1")]
    capslock: bool,
    #[packed_field(bits = "2")]
    scrolllock: bool,
    #[packed_field(bits = "3")]
    compose: bool,
    #[packed_field(bits = "4")]
    kana: bool,
    #[packed_field(bits = "5")]
    shift: bool,
}

/// A single LED
///
/// In general, a single LED can be used for capslock/numslock, or in a LED matrix.
struct SingleLED<P: OutputPin> {
    /// On/Off state
    state: bool,

    /// Pin state when turning LED on
    on_state: PinState,

    /// GPIO for controlling the LED
    pin: P,

    /// Brightness level, range: 0 ~ 255
    brightness: u8,

    // The duration in seconds of a LED breathing period
    period: u8,
}

impl<P: OutputPin> SingleLED<P> {
    fn new(pin: P, on_state: PinState) -> Self {
        Self {
            state: false,
            on_state,
            pin,
            brightness: 255,
            period: 0,
        }
    }

    /// Turn LED on
    fn on(&mut self) -> Result<(), P::Error> {
        self.pin.set_state(self.on_state)
    }

    /// Turn LED off
    fn off(&mut self) -> Result<(), P::Error> {
        self.pin.set_state(!self.on_state)
    }

    /// Set LED's brightness
    fn set_brightness(&mut self, brightness: u8) -> Result<(), P::Error> {
        self.brightness = brightness;

        // TODO: Write brightness to LED

        Ok(())
    }
}

pub(crate) struct LightService<P: OutputPin> {
    capslock: Option<SingleLED<P>>,
    scrolllock: Option<SingleLED<P>>,
    numslock: Option<SingleLED<P>>,
}

// Implement on/off function for LightService
macro_rules! impl_led_on_off {
    ($n:ident, $fn_name:ident) => {
        pub(crate) fn $fn_name(&mut self, state: bool) -> Result<(), P::Error> {
            if let Some(led) = &mut self.$n {
                if state {
                    led.on()?
                } else {
                    led.off()?
                }
            }
            Ok(())
        }
    };
}

impl<P: OutputPin> LightService<P> {
    pub(crate) fn new(
        capslock_pin: Option<P>,
        scrolllock_pin: Option<P>,
        numslock_pin: Option<P>,
        on_state: PinState,
    ) -> Self {
        Self {
            capslock: capslock_pin.map(|p| SingleLED::new(p, on_state)),
            scrolllock: scrolllock_pin.map(|p| SingleLED::new(p, on_state)),
            numslock: numslock_pin.map(|p| SingleLED::new(p, on_state)),
        }
    }
}

impl<P: OutputPin> LightService<P> {
    impl_led_on_off!(capslock, set_capslock);
    impl_led_on_off!(scrolllock, set_scrolllock);
    impl_led_on_off!(numslock, set_numslock);

    pub(crate) fn set_leds(&mut self, led_indicator: LedIndicator) -> Result<(), P::Error> {
        self.set_capslock(led_indicator.capslock)?;
        self.set_numslock(led_indicator.numslock)?;
        self.set_scrolllock(led_indicator.scrolllock)?;

        Ok(())
    }
}
