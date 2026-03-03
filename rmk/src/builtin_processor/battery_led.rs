//! Battery LED processor for RMK
use embedded_hal::digital::StatefulOutputPin;
use rmk_macro::processor;
use rmk_types::event::ChargeState;

use crate::driver::gpio::OutputController;
use crate::event::BatteryStatusEvent;

#[processor(subscribe = [BatteryStatusEvent], poll_interval = 1000)]
pub struct BatteryLedProcessor<P: StatefulOutputPin> {
    pin: OutputController<P>,
    state: BatteryStatusEvent,
}

impl<P: StatefulOutputPin> BatteryLedProcessor<P> {
    pub fn new(pin: P, low_active: bool) -> Self {
        Self {
            pin: OutputController::new(pin, low_active),
            state: BatteryStatusEvent::unavailable(),
        }
    }

    async fn on_battery_status_event(&mut self, event: BatteryStatusEvent) {
        self.state = event;
    }

    async fn poll(&mut self) {
        if !self.state.is_available() {
            self.pin.deactivate();
        } else if self.state.charge_state() == Some(ChargeState::Charging) {
            self.pin.activate();
        } else if self.state.level().unwrap_or(100) < 10 {
            // Battery low, blinking the LED
            self.pin.toggle();
        } else {
            self.pin.deactivate();
        }
    }
}
