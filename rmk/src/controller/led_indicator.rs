/// The controller for handling LEDindicators defines in HID spec, including NumLock, CapsLock, ScrollLock, Compose, and Kana.
use embedded_hal::digital::StatefulOutputPin;

use crate::{
    channel::{ControllerSub, CONTROLLER_CHANNEL},
    controller::Controller,
    driver::gpio::OutputController,
    event::ControllerEvent,
};

/// Indicators defined in the HID spec 11.1
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum KeyboardIndicator {
    NumLock,
    CapsLock,
    ScrollLock,
    Compose,
    Kana,
}

pub struct KeyboardIndicatorController<P: StatefulOutputPin> {
    pin: OutputController<P>,
    sub: ControllerSub,
    indicator: KeyboardIndicator,
}

impl<P: StatefulOutputPin> KeyboardIndicatorController<P> {
    pub fn new(pin: P, low_active: bool, lock_name: KeyboardIndicator) -> Self {
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
        match event {
            ControllerEvent::KeyboardIndicator(state) => {
                let activated = match self.indicator {
                    KeyboardIndicator::NumLock => state.num_lock(),
                    KeyboardIndicator::CapsLock => state.caps_lock(),
                    KeyboardIndicator::ScrollLock => state.scroll_lock(),
                    KeyboardIndicator::Compose => state.compose(),
                    KeyboardIndicator::Kana => state.kana(),
                };
                info!("Activating {} {}", self.indicator, activated);
                if activated {
                    self.pin.activate();
                } else {
                    self.pin.deactivate();
                }
            }
            _ => (),
        }
    }

    async fn next_message(&mut self) -> Self::Event {
        self.sub.next_message_pure().await
    }
}
