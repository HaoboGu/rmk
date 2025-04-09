use crate::config::BleBatteryConfig;
use core::sync::atomic::{AtomicU8, Ordering};
use embassy_time::Timer;
use nrf_softdevice::ble::Connection;

#[nrf_softdevice::gatt_service(uuid = "180f")]
#[derive(Debug, Clone, Copy)]
pub(crate) struct BatteryService {
    #[characteristic(uuid = "2a19", read, notify)]
    battery_level: u8,
}

// Global static variable, store the current battery level
static CURRENT_BATTERY_LEVEL: AtomicU8 = AtomicU8::new(255);

impl<'a> BatteryService {
    pub(crate) async fn run(
        &mut self,
        battery_config: &mut BleBatteryConfig<'a>,
        conn: &Connection,
    ) {
        // Wait 1 seconds, ensure that gatt server has been started
        Timer::after_secs(1).await;

        let battery_led_control = async {
            loop {
                // Read the current battery level
                let current_battery_level = CURRENT_BATTERY_LEVEL.load(Ordering::Relaxed);

                // Check if the device is charging
                let is_charging = if let Some(ref is_charging_pin) = battery_config.charge_state_pin
                {
                    is_charging_pin.is_low() == battery_config.charge_state_low_active
                } else {
                    false
                };

                // Control the LED based on the charging state and battery level
                if let Some(ref mut charge_led) = battery_config.charge_led_pin {
                    if is_charging {
                        // If the device is charging, the LED is always on
                        if battery_config.charge_led_low_active {
                            charge_led.set_low();
                        } else {
                            charge_led.set_high();
                        }
                    } else if current_battery_level < 50 {
                        // If the device is not charging and the battery level is less than 10%, the LED will blink
                        charge_led.toggle();
                        Timer::after_millis(200).await;
                    } else {
                        // If the device is not charging and the battery level is greater than 10%, the LED is always off
                        if battery_config.charge_led_low_active {
                            charge_led.set_high();
                        } else {
                            charge_led.set_low();
                        }
                    }
                }

                // If there is no blinking operation, wait for a while before checking again
                if !(is_charging == false && current_battery_level < 10) {
                    Timer::after_secs(30).await;
                }
            }
        };

        let report_battery_level = async {
            loop {
                let val = crate::channel::BATTERY_LEVEL_SIGNAL.wait().await;

                // Update the battery level
                CURRENT_BATTERY_LEVEL.store(val, Ordering::Relaxed);

                match self.battery_level_notify(conn, &val) {
                    Ok(_) => info!("Battery value: {}", val),
                    Err(e) => match self.battery_level_set(&val) {
                        Ok(_) => info!("Battery value set: {}", val),
                        Err(e2) => error!("Battery value notify error: {}, set error: {}", e, e2),
                    },
                }
            }
        };

        embassy_futures::join::join(battery_led_control, report_battery_level).await;
    }
}
