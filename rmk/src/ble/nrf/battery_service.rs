use defmt::{error, info};
use embassy_time::Timer;
use nrf_softdevice::ble::Connection;
use rmk_config::BleBatteryConfig;

use super::server::BleServer;

#[nrf_softdevice::gatt_service(uuid = "180f")]
#[derive(Debug, Clone, Copy)]
pub struct BatteryService {
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
            if let Some(ref mut saadc) = battery_config.saadc {
                let mut buf = [0i16; 1];
                saadc.sample(&mut buf).await;
                // We only sampled one ADC channel.
                let val: u8 = self.get_battery_percent(buf[0]);
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
                    Timer::after_millis(500).await;
                    continue;
                }
            } else {
                // No SAADC, skip battery check
                Timer::after_secs(u64::MAX).await;
            }

            // Check charging state
            BatteryService::check_charging_state(battery_config);

            // Sample every 120s
            Timer::after_secs(120).await
        }
    }

    // TODO: Make battery calculation user customizable
    fn get_battery_percent(&self, val: i16) -> u8 {
        info!("Detected adv value: {=i16}", val);
        // According to nRF52840's datasheet, for single_ended saadc:
        // val = v_adc * (gain / reference) * 2^(resolution)
        //
        // When using default setting, gain = 1/6, reference = 0.6v, resolution = 12bits, so:
        // val = v_adc * 1137.8
        //
        // For example, rmk-ble-keyboard uses two resistors 820K and 2M adjusting the v_adc, then,
        // v_adc = v_bat * 0.7092 => val = v_bat * 806.93
        // 
        // If the battery voltage range is 3.3v ~ 4.2v, the adc val range should be 2663 ~ 3389
        // To make calculation simple, adc val range 2650 ~ 3350 is used.
        if val > 3350 {
            100_u8
        } else if val < 2650 {
            0_u8
        } else {
            ((val - 2650) / 7) as u8
        }
    }
}

impl BleServer {
    pub(crate) fn set_battery_value(&self, conn: &Connection, val: &u8) {
        match self.bas.battery_level_notify(conn, val) {
            Ok(_) => info!("Battery value: {}", val),
            Err(e) => match self.bas.battery_level_set(val) {
                Ok(_) => info!("Battery value set: {}", val),
                Err(e2) => error!("Battery value notify error: {}, set error: {}", e, e2),
            },
        }
    }
}
