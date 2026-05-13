//! Per-transport topic publishers.
//!
//! Each `RynkUsbTransport`/`RynkBleTransport` instance owns one
//! [`TopicSubscribers`] bundle so the same event is observed independently
//! by every active wire — the underlying channels are PubSubChannels with
//! `subs` counted to match.
//!
//! [`TopicSubscribers::next_event`] is a single-shot `select!` over every
//! event we publish on Rynk. The transport's main `select!` arm awaits it
//! alongside the inbound RX path, so a topic write and a reply write are
//! always serialized on the same future — no notify interleaving.

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
    ConnectionStatusChangeEvent, EventSubscriber, LayerChangeEvent, LedIndicatorEvent, SleepStateEvent, SubscribableEvent,
    WpmUpdateEvent,
};

/// One yielded event from any of the topic subscribers. The transport
/// encodes `cmd` + the borrowed payload into a frame via
/// [`RynkService::write_topic`](super::RynkService::write_topic).
///
/// Holds the payload by value, not by reference — postcard's `Serialize`
/// requires only `&T`, and these primitive types are cheap to copy.
pub(in crate::host::rynk) enum TopicEvent {
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
    pub(in crate::host::rynk) fn cmd(&self) -> Cmd {
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

    /// Write the topic frame (header + payload) into `frame` in place.
    /// After this returns, the full frame occupies
    /// `&frame[..RYNK_HEADER_SIZE + frame.payload_len()]`.
    pub(in crate::host::rynk) fn encode(
        &self,
        service: &super::RynkService<'_>,
        frame: &mut rmk_types::protocol::rynk::Frame,
    ) {
        let cmd = self.cmd();
        match self {
            TopicEvent::LayerChange(v) => service.write_topic(cmd, v, frame),
            TopicEvent::WpmUpdate(v) => service.write_topic(cmd, v, frame),
            TopicEvent::ConnectionChange(v) => service.write_topic(cmd, v, frame),
            TopicEvent::SleepState(v) => service.write_topic(cmd, v, frame),
            TopicEvent::LedIndicator(v) => service.write_topic(cmd, v, frame),
            #[cfg(feature = "_ble")]
            TopicEvent::BatteryStatus(v) => service.write_topic(cmd, v, frame),
            #[cfg(feature = "_ble")]
            TopicEvent::BleStatusChange(v) => service.write_topic(cmd, v, frame),
        }
    }
}

/// Bundle of subscribers — one per Rynk topic.
///
/// Construct once per transport future, then poll [`next_event`] in
/// the transport's main `select!`.
pub(in crate::host::rynk) struct TopicSubscribers {
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
    /// Create one subscriber per topic. Each call consumes a `subs` slot
    /// in the underlying PubSubChannel; the `subscriber_default.toml`
    /// `rynk` entries must reserve one slot per active transport.
    pub(in crate::host::rynk) fn new() -> Self {
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

    /// Wait for any topic to fire and return the matching [`TopicEvent`].
    ///
    /// Biased select — earlier arms win when multiple subscribers are
    /// ready on the same wake. The order below favors keymap/state
    /// changes (`layer`, `wpm`, `conn`) over passive status pushes.
    pub(in crate::host::rynk) async fn next_event(&mut self) -> TopicEvent {
        crate::select_biased_with_feature! {
            e = self.layer.next_event().fuse() => TopicEvent::LayerChange((*e).into()),
            e = self.wpm.next_event().fuse() => TopicEvent::WpmUpdate((*e).into()),
            e = self.conn.next_event().fuse() => TopicEvent::ConnectionChange((*e).into()),
            e = self.sleep.next_event().fuse() => TopicEvent::SleepState((*e).into()),
            e = self.led.next_event().fuse() => TopicEvent::LedIndicator((*e).into()),
            with_feature("_ble"): e = self.battery.next_event().fuse() => TopicEvent::BatteryStatus((*e).into()),
            with_feature("_ble"): e = self.ble_status.next_event().fuse() => TopicEvent::BleStatusChange((*e).into()),
        }
    }
}
