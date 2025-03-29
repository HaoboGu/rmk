use crate::config::BleBatteryConfig;
use embassy_time::Timer;
use nrf_softdevice::ble::Connection;

#[nrf_softdevice::gatt_service(uuid = "180f")]
#[derive(Debug, Clone, Copy)]
pub(crate) struct BatteryService {
    #[characteristic(uuid = "2a19", read, notify)]
    battery_level: u8,
}

impl<'a> BatteryService {
    fn check_charging_state(battery_config: &mut BleBatteryConfig<'a>) {
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

    pub(crate) async fn run(
        &mut self,
        battery_config: &mut BleBatteryConfig<'a>,
        conn: &Connection,
    ) {
        // Wait 1 seconds, ensure that gatt server has been started
        Timer::after_secs(1).await;
        BatteryService::check_charging_state(battery_config);

        loop {
            let val = crate::channel::BATTERY_CHANNEL.receive().await;
            match self.battery_level_notify(conn, &val) {
                Ok(_) => info!("Battery value: {}", val),
                Err(e) => match self.battery_level_set(&val) {
                    Ok(_) => info!("Battery value set: {}", val),
                    Err(e2) => error!("Battery value notify error: {}, set error: {}", e, e2),
                },
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
            BatteryService::check_charging_state(battery_config);

            // Sample every 120s
            Timer::after_secs(120).await
        }
    }
}
