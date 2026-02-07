use core::sync::atomic::Ordering;

use embassy_futures::join::join;
use embassy_futures::select::{Either, select};
use embassy_sync::pubsub::Subscriber;
use embassy_time::{Duration, Instant, Timer, with_timeout};
use trouble_host::prelude::*;

use super::ble_server::Server;
use crate::ble::SLEEPING_STATE;
use crate::event::{BatteryStateEvent, SubscribableControllerEvent};
use crate::keyboard::LAST_KEY_TIMESTAMP;

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
    pub(crate) sub: Subscriber<
        'static,
        crate::RawMutex,
        BatteryStateEvent,
        { crate::BATTERY_STATE_EVENT_CHANNEL_SIZE },
        { crate::BATTERY_STATE_EVENT_SUB_SIZE },
        { crate::BATTERY_STATE_EVENT_PUB_SIZE },
    >,
}

impl<'stack, 'server, 'conn, P: PacketPool> BleBatteryServer<'stack, 'server, 'conn, P> {
    pub(crate) fn new(server: &Server, conn: &'conn GattConnection<'stack, 'server, P>) -> Self {
        Self {
            battery_level: server.battery_service.level,
            conn,
            sub: BatteryStateEvent::controller_subscriber(),
        }
    }
}

impl<P: PacketPool> BleBatteryServer<'_, '_, '_, P> {
    pub(crate) async fn run(&mut self) {
        // Wait 2 seconds, ensure that gatt server has been started
        Timer::after_secs(2).await;

        // First report after connected
        let first_report = async {
            loop {
                if let BatteryStateEvent::Normal(level) = self.sub.next_message_pure().await {
                    if let Err(e) = self.battery_level.notify(self.conn, &level).await {
                        error!("Failed to notify battery level: {:?}", e);
                    } else {
                        return;
                    }
                }
                embassy_time::Timer::after_secs(2).await;
            }
        };

        // Try to do the first battery report in 30 seconds
        with_timeout(Duration::from_secs(30), first_report).await.ok();

        // Report the battery level.
        loop {
            let battery_state = self.wait_until_battery_state_available().await;

            // Try to receive the latest message
            if let BatteryStateEvent::Normal(level) = self.sub.try_next_message_pure().unwrap_or(battery_state)
                && let Err(e) = self.battery_level.notify(self.conn, &level).await
            {
                error!("Failed to notify battery level: {:?}", e);
            }
        }
    }

    /// Wait until the battery state is available.
    /// To avoid unexpected wakeup, before reporting battery level, all conditions should be satistied:
    ///
    /// 1. There's a battery state update
    /// 2. There's a key press in last 1 minute, or timeout(30 minutes)
    /// 3. The keyboard is not in the sleep mode
    async fn wait_until_battery_state_available(&mut self) -> BatteryStateEvent {
        loop {
            // Calculate timeout when reporting battery level
            let timeout = async {
                loop {
                    embassy_time::Timer::after_secs(1800).await;
                    // 30 minutes passed and the keyboard isn't in sleep mode: timeout
                    if !SLEEPING_STATE.load(Ordering::Acquire) {
                        break;
                    }
                }
            };

            // Wait until there are both battery state update and key pressing or timeout
            let (battery_state, last_press) =
                join(self.sub.next_message_pure(), select(timeout, LAST_KEY_TIMESTAMP.wait())).await;

            // Then check the value last press time
            let last_press = match last_press {
                Either::First(_) => Instant::now().as_secs() as u32,
                Either::Second(last_press) => last_press,
            };

            // Only report battery state if the last key action is less than 60 seconds ago
            let current_time = Instant::now().as_secs() as u32;
            if current_time.saturating_sub(last_press) < 60 {
                return battery_state;
            }
        }
    }
}
