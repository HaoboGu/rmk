#![no_main]
#![no_std]

use defmt::info;
use rmk::macros::rmk_central;

// Custom controller to monitor peripheral battery levels
use rmk::controller::Controller;
use rmk::channel::ControllerSub;

struct PeripheralBatteryMonitor {
    controller_sub: ControllerSub,
}

impl Controller for PeripheralBatteryMonitor {
    type Event = rmk::event::ControllerEvent;

    async fn process_event(&mut self, event: Self::Event) {
        use rmk::event::ControllerEvent;
        if let ControllerEvent::SplitPeripheralBattery(peripheral_id, level) = event {
            info!("Peripheral {} battery level: {}%", peripheral_id, level);
            // You can add custom logic here:
            // - Update a display
            // - Trigger LED warnings for low battery
            // - Store battery history
        }
    }

    async fn next_message(&mut self) -> Self::Event {
        self.controller_sub.next_message_pure().await
    }
}

impl PeripheralBatteryMonitor {
    fn new(controller_sub: ControllerSub) -> Self {
        Self { controller_sub }
    }
}

#[rmk_central(controller(peripheral_battery = PeripheralBatteryMonitor))]
mod keyboard_central {}
