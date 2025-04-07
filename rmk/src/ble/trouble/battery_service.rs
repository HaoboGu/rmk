use crate::config::BleBatteryConfig;
use embassy_time::Timer;
use trouble_host::prelude::*;

use super::ble_server::Server;

/// Battery service
#[gatt_service(uuid = service::BATTERY)]
pub(crate) struct BatteryService {
    /// Battery Level
    #[descriptor(uuid = descriptors::VALID_RANGE, read, value = [0, 100])]
    #[characteristic(uuid = characteristic::BATTERY_LEVEL, read, notify)]
    pub(crate) level: u8,
}

pub(crate) struct BleBatteryServer<'stack, 'server, 'conn> {
    pub(crate) battery_level: Characteristic<u8>,
    pub(crate) conn: &'conn GattConnection<'stack, 'server>,
}

impl<'stack, 'server, 'conn> BleBatteryServer<'stack, 'server, 'conn> {
    pub(crate) fn new(server: &Server, conn: &'conn GattConnection<'stack, 'server>) -> Self {
        Self {
            battery_level: server.battery_service.level,
            conn,
        }
    }
}

impl<'a> BleBatteryServer<'_, '_, '_> {
    fn check_charging_state(&self, battery_config: &mut BleBatteryConfig<'a>) {
        if let Some(ref is_charging_pin) = battery_config.charge_state_pin {
            if is_charging_pin.is_low() == battery_config.charge_state_low_active {
                info!("Charging!");
                if let Some(ref mut charge_led) = battery_config.charge_led_pin {
                    if battery_config.charge_led_low_active {
                        charge_led.set_low()
                    } else {
                        charge_led.set_high()
                    }
                }
            } else {
                info!("Not charging!");
                if let Some(ref mut charge_led) = battery_config.charge_led_pin {
                    if battery_config.charge_led_low_active {
                        charge_led.set_high()
                    } else {
                        charge_led.set_low()
                    }
                }
            }
        }
    }

    pub(crate) async fn run(&mut self, battery_config: &mut BleBatteryConfig<'a>) {
        // Wait 1 seconds, ensure that gatt server has been started
        Timer::after_secs(1).await;
        self.check_charging_state(battery_config);

        loop {
            let val = crate::channel::BATTERY_CHANNEL.receive().await;
            match self.battery_level.notify(self.conn, &val).await {
                Ok(_) => {}
                Err(_) => {
                    error!("Failed to notify battery level");
                    break;
                }
            }
            if val < 10 {
                // The battery is low, blink the led!
                if let Some(ref mut charge_led) = battery_config.charge_led_pin {
                    charge_led.toggle();
                }
                Timer::after_secs(200).await;
            } else {
                // Turn off the led
                if let Some(ref mut charge_led) = battery_config.charge_led_pin {
                    if battery_config.charge_led_low_active {
                        charge_led.set_high();
                    } else {
                        charge_led.set_low();
                    }
                }
            }

            // Check charging state
            self.check_charging_state(battery_config);

            // Sample every 120s
            Timer::after_secs(120).await
        }
    }
}
