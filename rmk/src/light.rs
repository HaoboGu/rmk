use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

use bitfield_struct::bitfield;
use embassy_usb::class::hid::HidReader;
use embassy_usb::driver::Driver;
use embedded_hal::digital::{Error, OutputPin, PinState};
use serde::{Deserialize, Serialize};

use crate::config::{LightConfig, LightPinConfig};
use crate::hid::{HidError, HidReaderTrait};
use crate::keyboard::LOCK_LED_STATES;

#[bitfield(u8)]
#[derive(Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct LedIndicator {
    #[bits(1)]
    num_lock: bool,
    #[bits(1)]
    caps_lock: bool,
    #[bits(1)]
    scroll_lock: bool,
    #[bits(1)]
    compose: bool,
    #[bits(1)]
    kana: bool,
    #[bits(3)]
    _reserved: u8,
}

impl BitOr for LedIndicator {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self::from_bits(self.into_bits() | rhs.into_bits())
    }
}
impl BitAnd for LedIndicator {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self::from_bits(self.into_bits() & rhs.into_bits())
    }
}
impl Not for LedIndicator {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self::from_bits(!self.into_bits())
    }
}
impl BitAndAssign for LedIndicator {
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs;
    }
}
impl BitOrAssign for LedIndicator {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl LedIndicator {
    pub const fn new_from(num_lock: bool, caps_lock: bool, scroll_lock: bool, compose: bool, kana: bool) -> Self {
        Self::new()
            .with_num_lock(num_lock)
            .with_caps_lock(caps_lock)
            .with_scroll_lock(scroll_lock)
            .with_compose(compose)
            .with_kana(kana)
    }
}

pub(crate) struct LightService<'d, P: OutputPin, R: HidReaderTrait<ReportType = LedIndicator>> {
    pub(crate) enabled: bool,
    light_controller: &'d mut LightController<P>,
    reader: R,
}

impl<'d, P: OutputPin, R: HidReaderTrait<ReportType = LedIndicator>> LightService<'d, P, R> {
    pub(crate) fn new(light_controller: &'d mut LightController<P>, reader: R) -> Self {
        Self {
            enabled: light_controller.enabled(),
            light_controller,
            reader,
        }
    }

    pub(crate) async fn run(&mut self) {
        loop {
            match self.reader.read_report().await {
                Ok(indicator) => {
                    if self.enabled {
                        // Read led indicator data and send to LED channel
                        debug!("Read keyboard state: {:?}", indicator);
                        if let Err(e) = self.light_controller.set_leds(indicator) {
                            error!("Send led error {:?}", e.kind());
                            // If there's an error, wait for a while
                            embassy_time::Timer::after_millis(500).await;
                        }
                    }
                }
                Err(e) => {
                    error!("Read led error {:?}", e);
                    embassy_time::Timer::after_secs(1).await;
                }
            }
        }
    }
}

pub struct LightController<P: OutputPin> {
    capslock: Option<SingleLed<P>>,
    scrolllock: Option<SingleLed<P>>,
    numslock: Option<SingleLed<P>>,
}

/// A single LED
///
/// In general, a single LED can be used for capslock/numslock, or in a LED matrix.
struct SingleLed<P: OutputPin> {
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

impl<P: OutputPin> SingleLed<P> {
    fn new(p: LightPinConfig<P>) -> Self {
        let on_state = if p.low_active { PinState::Low } else { PinState::High };
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

pub(crate) struct UsbLedReader<'a, 'd, D: Driver<'d>> {
    hid_reader: &'a mut HidReader<'d, D, 1>,
}

impl<'a, 'd, D: Driver<'d>> UsbLedReader<'a, 'd, D> {
    pub(crate) fn new(hid_reader: &'a mut HidReader<'d, D, 1>) -> Self {
        Self { hid_reader }
    }
}

impl<'d, D: Driver<'d>> HidReaderTrait for UsbLedReader<'_, 'd, D> {
    type ReportType = LedIndicator;

    async fn read_report(&mut self) -> Result<Self::ReportType, HidError> {
        let mut buf = [0u8; 1];
        self.hid_reader.read(&mut buf).await.map_err(HidError::UsbReadError)?;

        LOCK_LED_STATES.store(buf[0], core::sync::atomic::Ordering::Relaxed);
        Ok(LedIndicator::from_bits(buf[0]))
    }
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

impl<P: OutputPin> LightController<P> {
    pub fn new(light_config: LightConfig<P>) -> Self {
        Self {
            capslock: light_config.capslock.map(|p| SingleLed::new(p)),
            scrolllock: light_config.scrolllock.map(|p| SingleLed::new(p)),
            numslock: light_config.numslock.map(|p| SingleLed::new(p)),
        }
    }

    impl_led_on_off!(capslock, set_capslock);
    impl_led_on_off!(scrolllock, set_scrolllock);
    impl_led_on_off!(numslock, set_numslock);

    pub(crate) fn set_leds(&mut self, led_indicator: LedIndicator) -> Result<(), P::Error> {
        self.set_capslock(led_indicator.caps_lock())?;
        self.set_numslock(led_indicator.num_lock())?;
        self.set_scrolllock(led_indicator.scroll_lock())?;

        Ok(())
    }

    pub(crate) fn enabled(&self) -> bool {
        self.capslock.is_some() || self.numslock.is_some() || self.scrolllock.is_some()
    }
}
