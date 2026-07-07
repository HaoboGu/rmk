//! DFU LED processor for RMK
use embedded_hal::digital::StatefulOutputPin;
use rmk_macro::processor;
use rmk_types::dfu::DfuStatus;

use crate::driver::gpio::OutputController;
use crate::event::DfuStatusEvent;

#[processor(subscribe = [DfuStatusEvent], poll_interval = 200)]
pub struct DfuLedProcessor<P: StatefulOutputPin> {
    pin: OutputController<P>,
    blink: bool,
}

impl<P: StatefulOutputPin> DfuLedProcessor<P> {
    pub fn new(pin: P, low_active: bool) -> Self {
        Self {
            pin: OutputController::new(pin, low_active),
            blink: false,
        }
    }

    async fn on_dfu_status_event(&mut self, event: DfuStatusEvent) {
        match *event {
            DfuStatus::Idle | DfuStatus::Finished => {
                self.blink = false;
                self.pin.deactivate();
            }
            DfuStatus::Started => {
                self.blink = false;
                self.pin.activate();
            }
            DfuStatus::Downloading => self.pin.toggle(),
            DfuStatus::Error => {
                self.blink = false;
                self.pin.activate();
            }
            DfuStatus::LockWaiting => {
                self.blink = false;
                self.pin.activate();
            }
            DfuStatus::LockUnlocked => {
                self.blink = true;
            }
        }
    }

    async fn poll(&mut self) {
        if self.blink {
            self.pin.toggle();
        }
    }
}
