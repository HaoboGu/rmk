use embassy_nrf::gpio::Output;
use rmk::event::{BatteryStatusEvent, ConnectionStatusChangeEvent, LedIndicatorEvent};
use rmk::macros::processor;
use rmk::types::battery::BatteryStatus;
use rmk::types::ble::BleState;
use rmk::types::led_indicator::LedIndicator;

const LOW_BATTERY_THRESHOLD: u8 = 10;
const RED: u8 = 0b001;
const GREEN: u8 = 0b010;
const BLUE: u8 = 0b100;
const PROFILE_COLORS: [u8; 3] = [RED | GREEN, GREEN | BLUE, RED | BLUE];

#[processor(subscribe = [BatteryStatusEvent, ConnectionStatusChangeEvent, LedIndicatorEvent], poll_interval = 20)]
pub(crate) struct StatusRgb {
    red: Output<'static>,
    green: Output<'static>,
    blue: Output<'static>,
    last_bits: u8,
    keylock: LedIndicator,
    connection: u8,
    active_device: u8,
    battery: u8,
    flash_times: u8,
    timer_steps: u16,
}

impl StatusRgb {
    pub(crate) fn new(red: Output<'static>, green: Output<'static>, blue: Output<'static>) -> Self {
        Self {
            red,
            green,
            blue,
            last_bits: u8::MAX,
            keylock: LedIndicator::new(),
            connection: 1,
            active_device: 0,
            battery: 111,
            flash_times: 15 * 4,
            timer_steps: 0,
        }
    }

    fn set_indicator_color(&mut self, bits: u8) {
        if bits == self.last_bits {
            return;
        }

        if bits & RED != 0 {
            self.red.set_low();
        } else {
            self.red.set_high();
        }

        if bits & GREEN != 0 {
            self.green.set_low();
        } else {
            self.green.set_high();
        }

        if bits & BLUE != 0 {
            self.blue.set_low();
        } else {
            self.blue.set_high();
        }

        self.last_bits = bits;
    }

    async fn on_battery_status_event(&mut self, event: BatteryStatusEvent) {
        if let BatteryStatus::Available { level: Some(level), .. } = event.into() {
            self.battery = level;
        }
    }

    async fn on_connection_status_change_event(&mut self, event: ConnectionStatusChangeEvent) {
        let ble = event.0.ble;
        if ble.profile >= PROFILE_COLORS.len() as u8 {
            return;
        }

        self.active_device = ble.profile;
        match ble.state {
            BleState::Connected => {
                self.connection = 2;
                self.flash_times = 3 * 4;
            }
            BleState::Advertising => {
                self.connection = 1;
                self.flash_times = 15 * 4;
            }
            BleState::Inactive => {}
        }
    }

    async fn on_led_indicator_event(&mut self, event: LedIndicatorEvent) {
        self.keylock = event.into();
    }

    async fn poll(&mut self) {
        self.timer_steps = self.timer_steps.wrapping_add(1);

        if self.connection > 0 {
            if self.active_device >= PROFILE_COLORS.len() as u8 {
                self.set_indicator_color(0);
                return;
            }

            if self.timer_steps & 0x0f == 0x0f {
                self.flash_times = self.flash_times.saturating_sub(1);
                let color_bits = PROFILE_COLORS[self.active_device as usize];

                match (self.timer_steps >> 4) & 0x03 {
                    0 => self.set_indicator_color(0),
                    1 => self.set_indicator_color(color_bits),
                    2 => {
                        if self.connection != 2 {
                            self.set_indicator_color(0);
                        }
                    }
                    3 => {
                        if self.connection != 2 {
                            self.set_indicator_color(BLUE);
                        }
                    }
                    _ => {}
                }

                if self.flash_times == 0 {
                    self.connection = 0;
                }
            }
        } else if self.battery < LOW_BATTERY_THRESHOLD {
            if self.timer_steps & 0x1f == 0x0f {
                self.set_indicator_color(RED);
            } else if self.timer_steps & 0x1f == 0x1f {
                self.set_indicator_color(0);
            }
        } else if self.keylock.caps_lock() {
            self.set_indicator_color(RED | BLUE);
        } else {
            self.set_indicator_color(0);
        }
    }
}
