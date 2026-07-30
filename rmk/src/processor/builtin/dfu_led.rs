use embedded_hal::digital::StatefulOutputPin;
use rmk_macro::processor;
use rmk_types::dfu::DfuStatus;

use crate::driver::gpio::OutputController;
use crate::event::DfuStatusEvent;

// Morse D F U  (200ms per unit)
// D = Dash(3) Dot(1) Dot(1) Dot(1)  + inter-char(3)
//     on 3, off 1, on 1, off 1, on 1, off 1, on 1, off 3
// F = Dot(1) Dot(1) Dash(3) Dot(1)  + inter-char(3)
//     on 1, off 1, on 1, off 1, on 1, off 1, on 3, off 1, on 1, off 3
// U = Dot(1) Dot(1) Dash(3)          + inter-word(7)
//     on 1, off 1, on 1, off 1, on 3, off 7
const DFU_MORSE: &[u8] = &[3, 1, 1, 1, 1, 1, 1, 3, 1, 1, 1, 1, 1, 1, 3, 1, 1, 3, 1, 1, 1, 1, 3, 7];

/// LED indicator that visually reflects the DFU state.
///
/// Reacts to [`DfuStatusEvent`] published by `dfu_lock` or the BLE DFU handler:
///
/// | State         | LED behaviour               |
/// |---------------|-----------------------------|
/// | `Idle`        | Off                         |
/// | `LockWaiting` | Solid on                    |
/// | `LockUnlocked`| Morse-code "DFU" (on loop)  |
/// | `Started`     | Solid on                    |
/// | `Downloading` | Toggle every 200 ms         |
/// | `Error`       | Solid on                    |
/// | `Finished`    | Off                         |
#[processor(subscribe = [DfuStatusEvent], poll_interval = 200)]
pub struct DfuLedProcessor<P: StatefulOutputPin> {
    pin: OutputController<P>,
    blink: bool,
    morse: bool,
    morse_pos: usize,
    morse_step: u8,
}

impl<P: StatefulOutputPin> DfuLedProcessor<P> {
    /// Create a new DFU LED processor.
    ///
    /// `pin` — the GPIO pin driving the LED.
    /// `low_active` — set to `true` if the LED is wired cathode-to-pin
    /// (pin low = LED on), `false` for anode-to-pin (pin high = LED on).
    pub fn new(pin: P, low_active: bool) -> Self {
        Self {
            pin: OutputController::new(pin, low_active),
            blink: false,
            morse: false,
            morse_pos: 0,
            morse_step: 0,
        }
    }

    fn clear_blink(&mut self) {
        self.blink = false;
        self.morse = false;
    }

    async fn on_dfu_status_event(&mut self, event: DfuStatusEvent) {
        match *event {
            DfuStatus::Idle | DfuStatus::Finished => {
                self.clear_blink();
                self.pin.deactivate();
            }
            DfuStatus::Started => {
                self.clear_blink();
                self.pin.activate();
            }
            DfuStatus::Downloading => {
                self.clear_blink();
                self.pin.toggle();
            }
            DfuStatus::Error => {
                self.clear_blink();
                self.pin.activate();
            }
            DfuStatus::LockWaiting => {
                self.clear_blink();
                self.pin.activate();
            }
            DfuStatus::LockUnlocked => {
                self.blink = false;
                self.morse = true;
                self.morse_pos = 0;
                self.morse_step = 0;
                self.pin.activate();
            }
        }
    }

    async fn poll(&mut self) {
        if self.morse {
            self.morse_step += 1;
            let duration = DFU_MORSE[self.morse_pos];
            if self.morse_step >= duration {
                self.morse_step = 0;
                self.morse_pos = (self.morse_pos + 1) % DFU_MORSE.len();
                self.pin.toggle();
            }
        } else if self.blink {
            self.pin.toggle();
        }
    }
}
