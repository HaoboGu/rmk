use core::sync::atomic::{AtomicU8, Ordering};

use embassy_time::Timer;
use trouble_host::prelude::*;

use super::ble_server::Server;

/// Battery level global value.
/// The range of battery level is 0-100, 255 > level > 100 means the battery is charging. 255 means the battery level is not available.
pub(crate) static BATTERY_LEVEL: AtomicU8 = AtomicU8::new(255);

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
    pub(crate) async fn run(&mut self) {
        // Wait 2 seconds, ensure that gatt server has been started
        Timer::after_secs(2).await;

        // TODO: Move battery charging state checking to a separate input device and processor.
        // let battery_led_control = async {
        //     loop {
        //         // Read the current battery level
        //         let current_battery_level = BATTERY_LEVEL.load(Ordering::Relaxed);

        //         // Check if the device is charging
        //         let is_charging = if let Some(ref is_charging_pin) = battery_config.charge_state_pin {
        //             is_charging_pin.is_low() == battery_config.charge_state_low_active
        //         } else {
        //             false
        //         };

        //         // Control the LED based on the charging state and battery level
        //         if let Some(ref mut charge_led) = battery_config.charge_led_pin {
        //             if is_charging {
        //                 // If the device is charging, the LED is always on
        //                 if battery_config.charge_led_low_active {
        //                     charge_led.set_low();
        //                 } else {
        //                     charge_led.set_high();
        //                 }
        //             } else if current_battery_level < 50 {
        //                 // If the device is not charging and the battery level is less than 10%, the LED will blink
        //                 charge_led.toggle();
        //                 Timer::after_millis(200).await;
        //             } else {
        //                 // If the device is not charging and the battery level is greater than 10%, the LED is always off
        //                 if battery_config.charge_led_low_active {
        //                     charge_led.set_high();
        //                 } else {
        //                     charge_led.set_low();
        //                 }
        //             }
        //         }

        //         // If there is no blinking operation, wait for a while before checking again
        //         if !(is_charging == false && current_battery_level < 10) {
        //             Timer::after_secs(30).await;
        //         }
        //     }
        // };

        let report_battery_level = async {
            loop {
                let val = BATTERY_LEVEL.load(Ordering::Relaxed);
                if val < 100 {
                    match self.battery_level.notify(self.conn, &val).await {
                        Ok(_) => {}
                        Err(_) => {
                            error!("Failed to notify battery level");
                            break;
                        }
                    }
                }
                // Report battery level every 30s
                Timer::after_secs(30).await
            }
        };

        report_battery_level.await;
    }
}
