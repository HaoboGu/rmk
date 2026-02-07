use embedded_hal::digital::StatefulOutputPin;
use rmk_macro::controller;

use crate::controller::PollingController;
use crate::driver::gpio::OutputController;
use crate::event::BatteryStateEvent;

#[controller(subscribe = [BatteryStateEvent])]
pub struct BatteryLedController<P: StatefulOutputPin> {
    pin: OutputController<P>,
    state: BatteryStateEvent,
}

impl<P: StatefulOutputPin> BatteryLedController<P> {
    pub fn new(pin: P, low_active: bool) -> Self {
        Self {
            pin: OutputController::new(pin, low_active),
            state: BatteryStateEvent::NotAvailable,
        }
    }

    async fn on_battery_state_event(&mut self, event: BatteryStateEvent) {
        self.state = event;
    }
}

impl<P: StatefulOutputPin> PollingController for BatteryLedController<P> {
    fn interval(&self) -> embassy_time::Duration {
        embassy_time::Duration::from_secs(1)
    }

    async fn update(&mut self) {
        match self.state {
            BatteryStateEvent::Normal(level) => {
                if level < 10 {
                    // Battery low, blinking the LED
                    self.pin.toggle();
                } else {
                    self.pin.deactivate();
                }
            }
            BatteryStateEvent::Charging => self.pin.activate(),
            BatteryStateEvent::Charged => self.pin.activate(), // LED stays on when fully charged
            BatteryStateEvent::NotAvailable => self.pin.deactivate(),
        }
    }
}
