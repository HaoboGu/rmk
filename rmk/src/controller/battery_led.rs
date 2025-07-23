use crate::channel::{ControllerSub, CONTROLLER_CHANNEL};
use crate::controller::{Controller, PollingController};
use crate::driver::gpio::OutputController;
use crate::event::ControllerEvent;
use embedded_hal::digital::StatefulOutputPin;

/// Battery state
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BatteryState {
    Low,
    Normal,
    Charging,
}

pub struct BatteryLedController<P: StatefulOutputPin> {
    pin: OutputController<P>,
    sub: ControllerSub,
    state: BatteryState,
}

impl<P: StatefulOutputPin> BatteryLedController<P> {
    pub fn new(pin: P, low_active: bool) -> Self {
        Self {
            pin: OutputController::new(pin, low_active),
            sub: unwrap!(CONTROLLER_CHANNEL.subscriber()),
            state: BatteryState::Normal,
        }
    }
}

impl<P: StatefulOutputPin> Controller for BatteryLedController<P> {
    type Event = ControllerEvent;

    async fn process_event(&mut self, event: Self::Event) {
        match event {
            ControllerEvent::Battery(level) => {
                if self.state != BatteryState::Charging {
                    if level < 10 {
                        self.state = BatteryState::Low;
                    } else {
                        self.state = BatteryState::Normal;
                    }
                }
            }
            ControllerEvent::ChargingState(charging) => {
                if charging {
                    self.state = BatteryState::Charging;
                } else {
                    self.state = BatteryState::Normal;
                }
            }
            _ => (),
        }
    }

    async fn next_message(&mut self) -> Self::Event {
        self.sub.next_message_pure().await
    }
}

impl<P: StatefulOutputPin> PollingController for BatteryLedController<P> {
    const INTERVAL: embassy_time::Duration = embassy_time::Duration::from_secs(1);

    async fn update(&mut self) {
        match self.state {
            BatteryState::Low => self.pin.toggle(),
            BatteryState::Normal => self.pin.deactivate(),
            BatteryState::Charging => self.pin.activate(),
        }
    }
}
