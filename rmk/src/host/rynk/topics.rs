//! Rynk topic publishers.
//!
//! Topics are firmware -> host event publishing channels.

use futures::FutureExt;
#[cfg(feature = "_ble")]
use rmk_types::battery::BatteryStatus;
use rmk_types::connection::ConnectionStatus;
use rmk_types::led_indicator::LedIndicator;
use rmk_types::protocol::rynk::{RynkMessage, command};

#[cfg(feature = "_ble")]
use crate::event::BatteryStatusEvent;
use crate::event::{
    ConnectionStatusChangeEvent, EventSubscriber, LayerChangeEvent, LedIndicatorEvent, SleepStateEvent,
    SubscribableEvent, WpmUpdateEvent,
};

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
    /// Build a topic message into `buf`: header (cmd, seq=0, payload_len)
    /// plus the postcard-encoded value, with cmd and payload type pinned per
    /// topic by the shared table. Returns the fully-formed message; the
    /// caller sends `&buf[..msg.frame_len()]`.
    pub(crate) fn encode<'a>(
        &self,
        buf: &'a mut [u8],
    ) -> Result<RynkMessage<'a>, rmk_types::protocol::rynk::RynkError> {
        match self {
            TopicEvent::LayerChange(v) => RynkMessage::build_topic::<command::LayerChange>(buf, v),
            TopicEvent::WpmUpdate(v) => RynkMessage::build_topic::<command::WpmUpdate>(buf, v),
            TopicEvent::ConnectionChange(v) => RynkMessage::build_topic::<command::ConnectionChange>(buf, v),
            TopicEvent::SleepState(v) => RynkMessage::build_topic::<command::SleepState>(buf, v),
            TopicEvent::LedIndicator(v) => RynkMessage::build_topic::<command::LedIndicatorChange>(buf, v),
            #[cfg(feature = "_ble")]
            TopicEvent::BatteryStatus(v) => RynkMessage::build_topic::<command::BatteryStatusChange>(buf, v),
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
        }
    }

    /// Await the next topic event to forward to the host.
    /// Each value's current snapshot is owned by its producer,
    /// this only forwards change notifications onto the wire.
    pub(crate) async fn next_event(&mut self) -> TopicEvent {
        crate::select_biased_with_feature! {
            e = self.layer.next_event().fuse() => TopicEvent::LayerChange(*e),
            e = self.wpm.next_event().fuse() => TopicEvent::WpmUpdate(*e),
            e = self.conn.next_event().fuse() => TopicEvent::ConnectionChange(*e),
            e = self.sleep.next_event().fuse() => TopicEvent::SleepState(*e),
            e = self.led.next_event().fuse() => TopicEvent::LedIndicator(*e),
            with_feature("_ble"): e = self.battery.next_event().fuse() => TopicEvent::BatteryStatus(*e),
        }
    }
}
