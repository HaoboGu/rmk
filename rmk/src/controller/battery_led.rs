use embedded_hal::digital::StatefulOutputPin;
use rmk_macro::controller;

use crate::event::{BatteryLevelEvent, ChargingStateEvent};
use crate::controller::PollingController;
use crate::driver::gpio::OutputController;

/// Battery state
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BatteryState {
    Low,
    Normal,
    Charging,
}

#[controller(subscribe = [BatteryLevelEvent, ChargingStateEvent])]
pub struct BatteryLedController<P: StatefulOutputPin> {
    pin: OutputController<P>,
    state: BatteryState,
}

impl<P: StatefulOutputPin> BatteryLedController<P> {
    pub fn new(pin: P, low_active: bool) -> Self {
        Self {
            pin: OutputController::new(pin, low_active),
            state: BatteryState::Normal,
        }
    }

    async fn on_battery_level_event(&mut self, event: BatteryLevelEvent) {
        if self.state != BatteryState::Charging {
            if event.level < 10 {
                self.state = BatteryState::Low;
            } else {
                self.state = BatteryState::Normal;
            }
        }
    }

    async fn on_charging_state_event(&mut self, event: ChargingStateEvent) {
        if event.charging {
            self.state = BatteryState::Charging;
        } else {
            self.state = BatteryState::Normal;
        }
    }
}

impl<P: StatefulOutputPin> PollingController for BatteryLedController<P> {
    fn interval(&self) -> embassy_time::Duration {
        embassy_time::Duration::from_secs(1)
    }

    async fn update(&mut self) {
        match self.state {
            BatteryState::Low => self.pin.toggle(),
            BatteryState::Normal => self.pin.deactivate(),
            BatteryState::Charging => self.pin.activate(),
        }
    }
}
