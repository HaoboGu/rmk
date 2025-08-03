use core::sync::atomic::{AtomicU8, Ordering};

use embassy_time::{Instant, Timer};
use trouble_host::prelude::*;

use super::ble_server::Server;
use crate::ble::trouble::SLEEPING_STATE;
use crate::keyboard::LAST_KEY_TIMESTAMP;

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

pub(crate) struct BleBatteryServer<'stack, 'server, 'conn, P: PacketPool> {
    pub(crate) battery_level: Characteristic<u8>,
    pub(crate) conn: &'conn GattConnection<'stack, 'server, P>,
}

impl<'stack, 'server, 'conn, P: PacketPool> BleBatteryServer<'stack, 'server, 'conn, P> {
    pub(crate) fn new(server: &Server, conn: &'conn GattConnection<'stack, 'server, P>) -> Self {
        Self {
            battery_level: server.battery_service.level,
            conn,
        }
    }
}

impl<P: PacketPool> BleBatteryServer<'_, '_, '_, P> {
    pub(crate) async fn run(&mut self) {
        // Wait 2 seconds, ensure that gatt server has been started
        Timer::after_secs(2).await;

        let report_battery_level = async {
            loop {
                let val = BATTERY_LEVEL.load(Ordering::Relaxed);
                if val <= 100 && !SLEEPING_STATE.load(Ordering::Acquire) {
                    let current_time = Instant::now().as_secs() as u32;
                    if current_time.saturating_sub(LAST_KEY_TIMESTAMP.load(Ordering::Acquire)) < 60 {
                        // Only report battery level if the last key action is less than 60 seconds ago
                        if let Err(e) = self.battery_level.notify(self.conn, &val).await {
                            error!("Failed to notify battery level: {:?}", e);
                        }
                    }
                }
                // Report battery level every 2 minutes
                Timer::after_secs(120).await
            }
        };

        report_battery_level.await;
    }
}
