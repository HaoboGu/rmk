//! Rynk topic publishers.
//!
//! Topics are firmware -> host event publishing channels.

use futures::FutureExt;
#[cfg(feature = "_ble")]
use rmk_types::battery::BatteryStatus;
use rmk_types::connection::ConnectionStatus;
use rmk_types::led_indicator::LedIndicator;
use rmk_types::protocol::rynk::{Cmd, RynkMessage};

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

    /// Build a topic message into `buf`: header (cmd, seq=0, payload_len)
    /// plus the postcard-encoded value. Returns the fully-formed message;
    /// the caller sends `&buf[..msg.frame_len()]`.
    pub(crate) fn encode<'a>(
        &self,
        buf: &'a mut [u8],
    ) -> Result<RynkMessage<'a>, rmk_types::protocol::rynk::RynkError> {
        let cmd = self.cmd();
        debug_assert!(cmd.is_topic(), "TopicEvent produced non-topic cmd");
        match self {
            TopicEvent::LayerChange(v) => RynkMessage::build(buf, cmd, 0, v),
            TopicEvent::WpmUpdate(v) => RynkMessage::build(buf, cmd, 0, v),
            TopicEvent::ConnectionChange(v) => RynkMessage::build(buf, cmd, 0, v),
            TopicEvent::SleepState(v) => RynkMessage::build(buf, cmd, 0, v),
            TopicEvent::LedIndicator(v) => RynkMessage::build(buf, cmd, 0, v),
            #[cfg(feature = "_ble")]
            TopicEvent::BatteryStatus(v) => RynkMessage::build(buf, cmd, 0, v),
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
            e = self.layer.next_event().fuse() => TopicEvent::LayerChange((*e).into()),
            e = self.wpm.next_event().fuse() => TopicEvent::WpmUpdate((*e).into()),
            e = self.conn.next_event().fuse() => TopicEvent::ConnectionChange((*e).into()),
            e = self.sleep.next_event().fuse() => TopicEvent::SleepState((*e).into()),
            e = self.led.next_event().fuse() => TopicEvent::LedIndicator((*e).into()),
            with_feature("_ble"): e = self.battery.next_event().fuse() => TopicEvent::BatteryStatus((*e).into()),
        }
    }
}
