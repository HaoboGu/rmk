use defmt::{error, info};
use embassy_time::Timer;
use nrf_softdevice::ble::Connection;

use crate::config::BleBatteryConfig;

use super::server::BleServer;

#[nrf_softdevice::gatt_service(uuid = "180f")]
#[derive(Debug, Clone, Copy)]
pub struct BatteryService {
    #[characteristic(uuid = "2a19", read, notify)]
    battery_level: u8,
}

// https://github.com/makerdiary/nrf52840-m2-devkit/blob/master/config/nrf52840_m2.h#L127
const BATTERY_LEVEL_LOOPUP_TABLE: [u8; 111] = [
    0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3,
    3, 3, 3, 4, 4, 4, 4, 4, 4, 5, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 11, 12, 13, 13, 14, 15, 16, 18,
    19, 22, 25, 28, 32, 36, 40, 44, 47, 51, 53, 56, 58, 60, 62, 64, 66, 67, 69, 71, 72, 74, 76, 77,
    79, 81, 82, 84, 85, 85, 86, 86, 86, 87, 88, 88, 89, 90, 91, 91, 92, 93, 94, 95, 96, 97, 98, 99,
    100, 100,
];

impl<'a> BatteryService {
    pub(crate) async fn run(&mut self, battery_config: &mut BleBatteryConfig<'a>, conn: &Connection) {
        // Wait 1 seconds, ensure that gatt server has been started
        Timer::after_secs(1).await;
        // Low means charging
        if let Some(ref is_charging_pin) = battery_config.charge_state_pin {
            if is_charging_pin.is_low() {
                info!("Charging!");
            }
        }
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
            }

            // Low means charging
            // TODO: customize charging level
            if let Some(ref is_charging_pin) = battery_config.charge_state_pin {
                if is_charging_pin.is_low() {
                    info!("Charging!");
                }
            }
            // Sample every 120s
            Timer::after_secs(120).await
        }
    }

    fn get_battery_percent(&self, val: i16) -> u8 {
        info!("Detected adv value: {=i16}", val);
        // Reference: https://github.com/makerdiary/nrf52840-m2-devkit/blob/master/examples/nrf5-sdk/battery_status/main.c#L102
        let mut idx = (val * 7 - 3100) / 10;
        if idx < 0_i16 {
            idx = 0_i16;
        } else if idx as usize >= BATTERY_LEVEL_LOOPUP_TABLE.len() {
            idx = (BATTERY_LEVEL_LOOPUP_TABLE.len() - 1) as i16;
        }

        BATTERY_LEVEL_LOOPUP_TABLE[idx as usize]
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
