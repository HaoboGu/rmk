/// The controller for handling LED indicators defined in HID spec, including NumLock, CapsLock, ScrollLock, Compose, and Kana.
use embedded_hal::digital::StatefulOutputPin;
use rmk_macro::controller;
use rmk_types::led_indicator::LedIndicatorType;

use crate::builtin_events::KeyboardStateEvent;
use crate::driver::gpio::OutputController;

#[controller(subscribe = [KeyboardStateEvent])]
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

    async fn on_keyboard_state_event(&mut self, event: KeyboardStateEvent) {
        if let KeyboardStateEvent::Indicator(state) = event {
            let activated = match self.indicator {
                LedIndicatorType::NumLock => state.num_lock(),
                LedIndicatorType::CapsLock => state.caps_lock(),
                LedIndicatorType::ScrollLock => state.scroll_lock(),
                LedIndicatorType::Compose => state.compose(),
                LedIndicatorType::Kana => state.kana(),
            };
            info!("Activating {:?} {}", self.indicator, activated);
            if activated {
                self.pin.activate();
            } else {
                self.pin.deactivate();
            }
        }
    }
}
