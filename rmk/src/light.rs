use crate::hid::HidReaderWrapper;
use bitfield_struct::bitfield;
use defmt::{debug, error, Format};
use embassy_time::Timer;
use embedded_hal::digital::{OutputPin, PinState};
use rmk_config::{LightConfig, LightPinConfig};

pub(crate) async fn led_task<R: HidReaderWrapper, Out: OutputPin>(
    keyboard_hid_reader: &mut R,
    light_service: &mut LightService<Out>,
) {
    loop {
        match light_service.check_led_indicator(keyboard_hid_reader).await {
            Ok(_) => Timer::after_millis(50).await,
            Err(_) => Timer::after_secs(2).await,
        }
    }
}

#[bitfield(u8)]
#[derive(Format, Eq, PartialEq)]
pub struct LedIndicator {
    #[bits(1)]
    numslock: bool,
    #[bits(1)]
    capslock: bool,
    #[bits(1)]
    scrolllock: bool,
    #[bits(1)]
    compose: bool,
    #[bits(1)]
    kana: bool,
    #[bits(1)]
    shift: bool,
    #[bits(2)]
    _reserved: u8,
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
    fn new(p: LightPinConfig<P>) -> Self {
        let on_state = if p.low_active {
            PinState::Low
        } else {
            PinState::High
        };
        Self {
            state: false,
            on_state,
            pin: p.pin,
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
    pub(crate) enabled: bool,
    led_indicator_data: [u8; 1],
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
        capslock_pin: Option<LightPinConfig<P>>,
        scrolllock_pin: Option<LightPinConfig<P>>,
        numslock_pin: Option<LightPinConfig<P>>,
    ) -> Self {
        let mut enabled = true;
        if capslock_pin.is_none() && scrolllock_pin.is_none() && numslock_pin.is_none() {
            enabled = false;
        }
        Self {
            enabled,
            led_indicator_data: [0; 1],
            capslock: capslock_pin.map(|p| SingleLED::new(p)),
            scrolllock: scrolllock_pin.map(|p| SingleLED::new(p)),
            numslock: numslock_pin.map(|p| SingleLED::new(p)),
        }
    }

    pub(crate) fn from_config(light_config: LightConfig<P>) -> Self {
        let mut enabled = true;
        if light_config.capslock.is_none()
            && light_config.numslock.is_none()
            && light_config.scrolllock.is_none()
        {
            enabled = false;
        }
        Self {
            enabled,
            led_indicator_data: [0; 1],
            capslock: light_config
                .capslock
                .map(|p| SingleLED::new(p)),
            scrolllock: light_config
                .scrolllock
                .map(|p| SingleLED::new(p)),
            numslock: light_config
                .numslock
                .map(|p| SingleLED::new(p)),
        }
    }
}

impl<P: OutputPin> LightService<P> {
    impl_led_on_off!(capslock, set_capslock);
    impl_led_on_off!(scrolllock, set_scrolllock);
    impl_led_on_off!(numslock, set_numslock);

    pub(crate) fn set_leds(&mut self, led_indicator: LedIndicator) -> Result<(), P::Error> {
        self.set_capslock(led_indicator.capslock())?;
        self.set_numslock(led_indicator.numslock())?;
        self.set_scrolllock(led_indicator.scrolllock())?;

        Ok(())
    }

    /// Check led indicator and update led status.
    ///
    /// If there's an error, print a message and ignore error types
    pub(crate) async fn check_led_indicator<R: HidReaderWrapper>(
        &mut self,
        keyboard_hid_reader: &mut R,
    ) -> Result<(), ()> {
        // If light service is not enabled, wait 2 seconds and recheck
        if !self.enabled {
            return Err(());
        }
        match keyboard_hid_reader.read(&mut self.led_indicator_data).await {
            Ok(_) => {
                let indicator = LedIndicator::from_bits(self.led_indicator_data[0]);
                debug!("Read keyboard state: {:?}", indicator);
                self.set_leds(indicator).map_err(|_| ())
            }
            Err(e) => {
                error!("Read keyboard state error: {}", e);
                Err(())
            }
        }
    }
}
