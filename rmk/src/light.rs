use crate::{
    config::LightPinConfig,
    hid::{HidError, HidReaderTrait},
};
use bitfield_struct::bitfield;
use embassy_usb::{class::hid::HidReader, driver::Driver};
use embedded_hal::digital::{OutputPin, PinState};
use serde::{Deserialize, Serialize};

#[bitfield(u8)]
#[derive( Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct LedIndicator {
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
struct SingleLed<'d, P: OutputPin> {
    /// On/Off state
    state: bool,

    /// Pin state when turning LED on
    on_state: PinState,

    /// GPIO for controlling the LED
    pin: &'d mut P,

    /// Brightness level, range: 0 ~ 255
    brightness: u8,

    // The duration in seconds of a LED breathing period
    period: u8,
}

impl<'d, P: OutputPin> SingleLed<'d, P> {
    fn new(p: LightPinConfig<'d, P>) -> Self {
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

pub(crate) struct UsbLedReader<'a, 'd, D: Driver<'d>> {
    hid_reader: &'a mut HidReader<'d, D, 1>,
}

impl<'a, 'd, D: Driver<'d>> UsbLedReader<'a, 'd, D> {
    pub(crate) fn new(hid_reader: &'a mut HidReader<'d, D, 1>) -> Self {
        Self { hid_reader }
    }
}

impl<'a, 'd, D: Driver<'d>> HidReaderTrait for UsbLedReader<'a, 'd, D> {
    type ReportType = LedIndicator;

    async fn read_report(&mut self) -> Result<Self::ReportType, HidError> {
        let mut buf = [0u8; 1];
        self.hid_reader
            .read(&mut buf)
            .await
            .map_err(|e| HidError::UsbReadError(e))?;

        Ok(LedIndicator::from_bits(buf[0]))
    }
}

/// FIXME: LightService now requires &mut RmkConfig.light_config
pub(crate) struct LightService<'d, P: OutputPin, R: HidReaderTrait<ReportType = LedIndicator>> {
    pub(crate) enabled: bool,
    capslock: Option<LightPinConfig<'d, P>>,
    scrolllock: Option<LightPinConfig<'d, P>>,
    numslock: Option<LightPinConfig<'d, P>>,
    hid_reader: R,
}

// Implement on/off function for LightService
// macro_rules! impl_led_on_off {
//     ($n:ident, $fn_name:ident) => {
//         pub(crate) fn $fn_name(&mut self, state: bool) -> Result<(), P::Error> {
//             if let Some(led) = &mut self.$n {
//                 if state {
//                     led.on()?
//                 } else {
//                     led.off()?
//                 }
//             }
//             Ok(())
//         }
//     };
// }

impl<'d, P: OutputPin, R: HidReaderTrait<ReportType = LedIndicator>> LightService<'d, P, R> {
    pub(crate) fn new(
        // capslock_pin: Option<LightPinConfig<'d, P>>,
        // scrolllock_pin: Option<LightPinConfig<'d, P>>,
        // numslock_pin: Option<LightPinConfig<'d, P>>,
        hid_reader: R,
    ) -> Self {
        // let mut enabled = true;
        // if capslock_pin.is_none() && scrolllock_pin.is_none() && numslock_pin.is_none() {
        //     enabled = false;
        // }
        Self {
            enabled: true,
            // capslock: capslock_pin.map(|p| SingleLed::new(p)),
            // scrolllock: scrolllock_pin.map(|p| SingleLed::new(p)),
            // numslock: numslock_pin.map(|p| SingleLed::new(p)),
            capslock: None,
            scrolllock: None,
            numslock: None,
            hid_reader,
        }
    }

    pub(crate) async fn run(&mut self) {
        loop {
            if !self.enabled {
                embassy_time::Timer::after_secs(u64::MAX).await;
            } else {
                match self.hid_reader.read_report().await {
                    Ok(indicator) => {
                        // Read led indicator data and send to LED channel
                        debug!("Read keyboard state: {:?}", indicator);
                        // if let Err(e) = self.set_leds(indicator) {
                        //     error!("Set led error {:?}", e.kind());
                        //     // If there's an error, wait for a while
                        //     embassy_time::Timer::after_millis(500).await;
                        // }
                    }
                    Err(e) => error!("Hid read error: {}", e),
                };
                embassy_time::Timer::after_millis(500).await;
            }
        }
    }

    // pub(crate) fn from_config(light_config: &LightConfig<'d, P>, hid_reader: R) -> Self {
    //     let mut enabled = true;
    //     if light_config.capslock.is_none()
    //         && light_config.numslock.is_none()
    //         && light_config.scrolllock.is_none()
    //     {
    //         enabled = false;
    //     }

    //     Self {
    //         enabled,
    //         capslock: &mut light_config.capslock,
    //         scrolllock: &mut light_config.scrolllock,
    //         numslock: &mut light_config.numslock,
    //         hid_reader,
    //     }
    // }

    // pub(crate) fn set_leds(&mut self, led_indicator: LedIndicator) -> Result<(), P::Error> {
    //     self.set_capslock(led_indicator.capslock())?;
    //     self.set_numslock(led_indicator.numslock())?;
    //     self.set_scrolllock(led_indicator.scrolllock())?;

    //     Ok(())
    // }

    // impl_led_on_off!(capslock, set_capslock);
    // impl_led_on_off!(scrolllock, set_scrolllock);
    // impl_led_on_off!(numslock, set_numslock);
}
