/// The controller for handling LEDindicators defines in HID spec, including NumLock, CapsLock, ScrollLock, Compose, and Kana.
use embedded_hal::digital::StatefulOutputPin;
use rmk_types::led_indicator::LedIndicatorType;

use crate::channel::{CONTROLLER_CHANNEL, ControllerSub};
use crate::controller::Controller;
use crate::driver::gpio::OutputController;
use crate::event::ControllerEvent;

pub struct KeyboardIndicatorController<P: StatefulOutputPin> {
    pin: OutputController<P>,
    sub: ControllerSub,
    indicator: LedIndicatorType,
}

impl<P: StatefulOutputPin> KeyboardIndicatorController<P> {
    pub fn new(pin: P, low_active: bool, lock_name: LedIndicatorType) -> Self {
        Self {
            pin: OutputController::new(pin, low_active),
            sub: unwrap!(CONTROLLER_CHANNEL.subscriber()),
            indicator: lock_name,
        }
    }
}

impl<P: StatefulOutputPin> Controller for KeyboardIndicatorController<P> {
    type Event = ControllerEvent;

    async fn process_event(&mut self, event: Self::Event) {
        if let ControllerEvent::KeyboardIndicator(state) = event {
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

    async fn next_message(&mut self) -> Self::Event {
        self.sub.next_message_pure().await
    }
}
