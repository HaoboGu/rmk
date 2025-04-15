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

        let report_battery_level = async {
            loop {
                let val = BATTERY_LEVEL.load(Ordering::Relaxed);
                if val <= 100 {
                    match self.battery_level.notify(self.conn, &val).await {
                        Ok(_) => {}
                        Err(_) => {
                            error!("Failed to notify battery level");
                            break;
                        }
                    }
                } else if val < 255 {
                    debug!("Charging, val: {}", val);
                } else {
                    debug!("Battery level not available");
                }
                // Report battery level every 30s
                Timer::after_secs(30).await
            }
        };

        report_battery_level.await;
    }
}
