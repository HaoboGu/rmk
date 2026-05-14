//! Rynk topic publishers.
//!
//! Topics are firmware -> host event publishing channels.

use futures::FutureExt;
#[cfg(feature = "_ble")]
use rmk_types::battery::BatteryStatus;
#[cfg(feature = "_ble")]
use rmk_types::ble::BleStatus;
use rmk_types::connection::ConnectionStatus;
use rmk_types::led_indicator::LedIndicator;
use rmk_types::protocol::rynk::Cmd;

#[cfg(feature = "_ble")]
use crate::event::{BatteryStatusEvent, BleStatusChangeEvent};
use crate::event::{
    ConnectionStatusChangeEvent, EventSubscriber, LayerChangeEvent, LedIndicatorEvent, SleepStateEvent,
    SubscribableEvent, WpmUpdateEvent,
};
use crate::host::context::KeyboardContext;

pub(crate) enum TopicEvent {
    LayerChange(u8),
    WpmUpdate(u16),
    ConnectionChange(ConnectionStatus),
    SleepState(bool),
    LedIndicator(LedIndicator),
    #[cfg(feature = "_ble")]
    BatteryStatus(BatteryStatus),
    #[cfg(feature = "_ble")]
    BleStatusChange(BleStatus),
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
            #[cfg(feature = "_ble")]
            TopicEvent::BleStatusChange(_) => Cmd::BleStatusChangeTopic,
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
            #[cfg(feature = "_ble")]
            TopicEvent::BleStatusChange(v) => service.write_topic(cmd, v, msg),
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
    #[cfg(feature = "_ble")]
    ble_status: <BleStatusChangeEvent as SubscribableEvent>::Subscriber,
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
            #[cfg(feature = "_ble")]
            ble_status: BleStatusChangeEvent::subscriber(),
        }
    }

    /// Latches `WpmUpdate` / `SleepState` payloads into `ctx` so the matching
    /// `GetWpm` / `GetSleepState` handlers can answer without re-subscribing.
    pub(crate) async fn next_event(&mut self, ctx: &KeyboardContext<'_>) -> TopicEvent {
        crate::select_biased_with_feature! {
            e = self.layer.next_event().fuse() => TopicEvent::LayerChange((*e).into()),
            e = self.wpm.next_event().fuse() => {
                let v: u16 = (*e).into();
                ctx.cache_wpm(v);
                TopicEvent::WpmUpdate(v)
            },
            e = self.conn.next_event().fuse() => TopicEvent::ConnectionChange((*e).into()),
            e = self.sleep.next_event().fuse() => {
                let v: bool = (*e).into();
                ctx.cache_sleep(v);
                TopicEvent::SleepState(v)
            },
            e = self.led.next_event().fuse() => TopicEvent::LedIndicator((*e).into()),
            with_feature("_ble"): e = self.battery.next_event().fuse() => TopicEvent::BatteryStatus((*e).into()),
            with_feature("_ble"): e = self.ble_status.next_event().fuse() => TopicEvent::BleStatusChange((*e).into()),
        }
    }
}
