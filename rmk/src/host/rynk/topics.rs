//! Rynk topic publishers.
//!
//! Topics are firmware -> host event publishing channels.

use futures::FutureExt;
#[cfg(feature = "_ble")]
use rmk_types::battery::BatteryStatus;
use rmk_types::connection::ConnectionStatus;
use rmk_types::led_indicator::LedIndicator;
use rmk_types::protocol::rynk::Cmd;

#[cfg(feature = "_ble")]
use crate::event::BatteryStatusEvent;
use crate::event::{
    ConnectionStatusChangeEvent, EventSubscriber, LayerChangeEvent, LedIndicatorEvent, SleepStateEvent,
    SubscribableEvent, WpmUpdateEvent,
};
#[cfg(feature = "split")]
use crate::event::{PeripheralBatteryEvent, PeripheralConnectedEvent};
use crate::host::context::KeyboardContext;

pub(crate) enum TopicEvent {
    LayerChange(u8),
    WpmUpdate(u16),
    ConnectionChange(ConnectionStatus),
    SleepState(bool),
    LedIndicator(LedIndicator),
    #[cfg(feature = "_ble")]
    BatteryStatus(BatteryStatus),
}

impl TopicEvent {
    /// Map the event to its wire-format `Cmd` tag.
    pub(crate) fn cmd(&self) -> Cmd {
        match self {
            TopicEvent::LayerChange(_) => Cmd::LayerChange,
            TopicEvent::WpmUpdate(_) => Cmd::WpmUpdate,
            TopicEvent::ConnectionChange(_) => Cmd::ConnectionChange,
            TopicEvent::SleepState(_) => Cmd::SleepState,
            TopicEvent::LedIndicator(_) => Cmd::LedIndicator,
            #[cfg(feature = "_ble")]
            TopicEvent::BatteryStatus(_) => Cmd::BatteryStatusTopic,
        }
    }

    /// Write the topic message (header + payload) into `msg` in place.
    pub(crate) fn encode(
        &self,
        service: &super::RynkService<'_>,
        msg: &mut [u8],
    ) -> Result<(), rmk_types::protocol::rynk::RynkError> {
        let cmd = self.cmd();
        match self {
            TopicEvent::LayerChange(v) => service.write_topic(cmd, v, msg),
            TopicEvent::WpmUpdate(v) => service.write_topic(cmd, v, msg),
            TopicEvent::ConnectionChange(v) => service.write_topic(cmd, v, msg),
            TopicEvent::SleepState(v) => service.write_topic(cmd, v, msg),
            TopicEvent::LedIndicator(v) => service.write_topic(cmd, v, msg),
            #[cfg(feature = "_ble")]
            TopicEvent::BatteryStatus(v) => service.write_topic(cmd, v, msg),
        }
    }
}

pub(crate) struct TopicSubscribers {
    layer: <LayerChangeEvent as SubscribableEvent>::Subscriber,
    wpm: <WpmUpdateEvent as SubscribableEvent>::Subscriber,
    conn: <ConnectionStatusChangeEvent as SubscribableEvent>::Subscriber,
    sleep: <SleepStateEvent as SubscribableEvent>::Subscriber,
    led: <LedIndicatorEvent as SubscribableEvent>::Subscriber,
    #[cfg(feature = "_ble")]
    battery: <BatteryStatusEvent as SubscribableEvent>::Subscriber,
    /// Cache-only: feeds `KeyboardContext::peripheral_status` snapshots so
    /// `Cmd::GetPeripheralStatus` can answer synchronously. Not surfaced as
    /// a Rynk topic on the wire.
    #[cfg(feature = "split")]
    peri_connected: <PeripheralConnectedEvent as SubscribableEvent>::Subscriber,
    #[cfg(feature = "split")]
    peri_battery: <PeripheralBatteryEvent as SubscribableEvent>::Subscriber,
}

impl TopicSubscribers {
    pub(crate) fn new() -> Self {
        Self {
            layer: LayerChangeEvent::subscriber(),
            wpm: WpmUpdateEvent::subscriber(),
            conn: ConnectionStatusChangeEvent::subscriber(),
            sleep: SleepStateEvent::subscriber(),
            led: LedIndicatorEvent::subscriber(),
            #[cfg(feature = "_ble")]
            battery: BatteryStatusEvent::subscriber(),
            #[cfg(feature = "split")]
            peri_connected: PeripheralConnectedEvent::subscriber(),
            #[cfg(feature = "split")]
            peri_battery: PeripheralBatteryEvent::subscriber(),
        }
    }

    /// Latches `WpmUpdate` / `SleepState` / `PeripheralConnected` /
    /// `PeripheralBattery` payloads into `ctx` so the matching `Get*` handlers
    /// can answer without re-subscribing. Loops internally on cache-only
    /// arms so it only returns when a real wire-visible topic fires.
    pub(crate) async fn next_event(&mut self, ctx: &KeyboardContext<'_>) -> TopicEvent {
        loop {
            let next: Option<TopicEvent> = crate::select_biased_with_feature! {
                e = self.layer.next_event().fuse() => Some(TopicEvent::LayerChange((*e).into())),
                e = self.wpm.next_event().fuse() => {
                    let v: u16 = (*e).into();
                    ctx.set_wpm(v);
                    Some(TopicEvent::WpmUpdate(v))
                },
                e = self.conn.next_event().fuse() => Some(TopicEvent::ConnectionChange((*e).into())),
                e = self.sleep.next_event().fuse() => {
                    let v: bool = (*e).into();
                    ctx.set_sleep(v);
                    Some(TopicEvent::SleepState(v))
                },
                e = self.led.next_event().fuse() => Some(TopicEvent::LedIndicator((*e).into())),
                with_feature("_ble"): e = self.battery.next_event().fuse() => Some(TopicEvent::BatteryStatus((*e).into())),
                with_feature("split"): e = self.peri_connected.next_event().fuse() => {
                    ctx.set_peripheral_connected(e.id, e.connected);
                    None
                },
                with_feature("split"): e = self.peri_battery.next_event().fuse() => {
                    ctx.set_peripheral_battery(e.id, e.state.0);
                    None
                },
            };
            if let Some(t) = next {
                return t;
            }
        }
    }
}
