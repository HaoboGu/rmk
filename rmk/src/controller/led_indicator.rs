/// The controller for handling LED indicators defined in HID spec, including NumLock, CapsLock, ScrollLock, Compose, and Kana.
use embedded_hal::digital::StatefulOutputPin;
use rmk_macro::controller;
use rmk_types::led_indicator::LedIndicatorType;

use crate::event::LedIndicatorEvent;
use crate::driver::gpio::OutputController;

#[controller(subscribe = [LedIndicatorEvent])]
pub struct KeyboardIndicatorController<P: StatefulOutputPin> {
    pin: OutputController<P>,
    indicator: LedIndicatorType,
}

impl<P: StatefulOutputPin> KeyboardIndicatorController<P> {
    pub fn new(pin: P, low_active: bool, lock_name: LedIndicatorType) -> Self {
        Self {
            pin: OutputController::new(pin, low_active),
            indicator: lock_name,
        }
    }

    async fn on_led_indicator_event(&mut self, event: LedIndicatorEvent) {
        let activated = match self.indicator {
            LedIndicatorType::NumLock => event.indicator.num_lock(),
            LedIndicatorType::CapsLock => event.indicator.caps_lock(),
            LedIndicatorType::ScrollLock => event.indicator.scroll_lock(),
            LedIndicatorType::Compose => event.indicator.compose(),
            LedIndicatorType::Kana => event.indicator.kana(),
        };
        info!("Activating {:?} {}", self.indicator, activated);
        if activated {
            self.pin.activate();
        } else {
            self.pin.deactivate();
        }
    }
}
