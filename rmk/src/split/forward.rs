//! Split event forwarding infrastructure
//!
//! This module provides the channels and wrappers that enable user-defined events
//! to be automatically forwarded across split keyboard halves.

use core::marker::PhantomData;

use embassy_sync::pubsub::PubSubChannel;
use futures::FutureExt;
use postcard::experimental::max_size::MaxSize;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    event::{AsyncEventPublisher, EventPublisher, EventSubscriber},
    RawMutex, SPLIT_DISPATCH_EVENT_CHANNEL_SIZE, SPLIT_DISPATCH_EVENT_PUB_SIZE,
    SPLIT_DISPATCH_EVENT_SUB_SIZE, SPLIT_FORWARD_EVENT_CHANNEL_SIZE, SPLIT_FORWARD_EVENT_PUB_SIZE,
    SPLIT_FORWARD_EVENT_SUB_SIZE,
};

use super::{DispatchedSplitPacket, SplitUserPacket};

/// Publisher → split transport layer
/// PubSub: multiple SplitForwardingPublisher write, SplitPeripheral + PeripheralManagers subscribe
pub static SPLIT_FORWARD_CHANNEL: PubSubChannel<
    RawMutex,
    SplitUserPacket,
    SPLIT_FORWARD_EVENT_CHANNEL_SIZE,
    SPLIT_FORWARD_EVENT_SUB_SIZE,
    SPLIT_FORWARD_EVENT_PUB_SIZE,
> = PubSubChannel::new();

/// Split transport layer → subscriber
/// PubSub: SplitPeripheral/PeripheralManager write, multiple SplitAwareSubscriber subscribe
/// Carries `DispatchedSplitPacket` which includes the source peripheral_id stamped by the transport layer.
pub static SPLIT_DISPATCH_CHANNEL: PubSubChannel<
    RawMutex,
    DispatchedSplitPacket,
    SPLIT_DISPATCH_EVENT_CHANNEL_SIZE,
    SPLIT_DISPATCH_EVENT_SUB_SIZE,
    SPLIT_DISPATCH_EVENT_PUB_SIZE,
> = PubSubChannel::new();

/// Trait for events that can be forwarded across split keyboard halves
pub trait SplitForwardable: Serialize + DeserializeOwned + MaxSize + Clone + Send {
    /// Unique identifier for this event type (u16 for RAM efficiency)
    const SPLIT_EVENT_KIND: u16;
}

/// Encode an event into a SplitUserPacket for wire transmission
pub fn encode_split_event<E: SplitForwardable>(event: &E) -> Option<SplitUserPacket> {
    let mut packet = SplitUserPacket {
        kind: E::SPLIT_EVENT_KIND,
        len: 0,
        data: [0u8; crate::SPLIT_USER_PAYLOAD_MAX_SIZE],
    };
    match postcard::to_slice(event, &mut packet.data) {
        Ok(bytes) => {
            packet.len = bytes.len() as u8;
            Some(packet)
        }
        Err(_) => None,
    }
}

/// Decode a SplitUserPacket back into an event
pub fn decode_split_event<E: SplitForwardable>(packet: &SplitUserPacket) -> Option<E> {
    if packet.kind != E::SPLIT_EVENT_KIND {
        return None;
    }
    let len = packet.len as usize;
    if len > packet.data.len() {
        return None;
    }
    postcard::from_bytes(&packet.data[..len]).ok()
}

/// Publisher wrapper that automatically forwards events to the split transport layer
pub struct SplitForwardingPublisher<P> {
    inner: P,
}

impl<P> SplitForwardingPublisher<P> {
    pub fn new(inner: P) -> Self {
        Self { inner }
    }
}

impl<P: EventPublisher> EventPublisher for SplitForwardingPublisher<P>
where
    P::Event: SplitForwardable,
{
    type Event = P::Event;

    fn publish(&self, message: P::Event) {
        // Forward to split transport
        if let Some(packet) = encode_split_event(&message) {
            SPLIT_FORWARD_CHANNEL
                .immediate_publisher()
                .publish_immediate(packet);
        }
        // Publish locally
        self.inner.publish(message);
    }
}

impl<P: AsyncEventPublisher> AsyncEventPublisher for SplitForwardingPublisher<P>
where
    P::Event: SplitForwardable,
{
    type Event = P::Event;

    async fn publish_async(&self, message: P::Event) {
        // Forward to split transport
        if let Some(packet) = encode_split_event(&message) {
            SPLIT_FORWARD_CHANNEL
                .immediate_publisher()
                .publish_immediate(packet);
        }
        // Publish locally
        self.inner.publish_async(message).await;
    }
}

/// Subscriber wrapper that receives events from both local and remote sources
pub struct SplitAwareSubscriber<S, E> {
    local: S,
    dispatch: embassy_sync::pubsub::Subscriber<
        'static,
        RawMutex,
        DispatchedSplitPacket,
        SPLIT_DISPATCH_EVENT_CHANNEL_SIZE,
        SPLIT_DISPATCH_EVENT_SUB_SIZE,
        SPLIT_DISPATCH_EVENT_PUB_SIZE,
    >,
    _phantom: PhantomData<E>,
}

impl<S, E> SplitAwareSubscriber<S, E> {
    pub fn new(local: S) -> Self {
        Self {
            local,
            dispatch: SPLIT_DISPATCH_CHANNEL
                .subscriber()
                .expect("split dispatch subs exceeded"),
            _phantom: PhantomData,
        }
    }
}

impl<S: EventSubscriber<Event = E>, E: SplitForwardable> EventSubscriber
    for SplitAwareSubscriber<S, E>
{
    type Event = E;

    async fn next_event(&mut self) -> E {
        loop {
            crate::futures::select_biased! {
                // Local events have priority
                e = self.local.next_event().fuse() => return e,
                // Remote events from split dispatch
                dispatched = self.dispatch.next_message_pure().fuse() => {
                    if let Some(e) = decode_split_event::<E>(&dispatched.packet) {
                        return e;
                    }
                    // Kind mismatch or decode failure, continue waiting
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use postcard::experimental::max_size::MaxSize;
    use serde::{Deserialize, Serialize};

    use super::{SplitForwardable, decode_split_event, encode_split_event};
    use crate::split::SplitUserPacket;

    // A minimal event type for testing
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize, MaxSize)]
    struct TestEvent {
        value: u16,
        flag: bool,
    }

    impl SplitForwardable for TestEvent {
        const SPLIT_EVENT_KIND: u16 = 0x1234;
    }

    // A second event type with a different kind
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize, MaxSize)]
    struct OtherEvent {
        value: u32,
    }

    impl SplitForwardable for OtherEvent {
        const SPLIT_EVENT_KIND: u16 = 0x5678;
    }

    #[test]
    fn encode_decode_roundtrip() {
        let original = TestEvent { value: 42, flag: true };
        let packet = encode_split_event(&original).expect("encode should succeed");

        assert_eq!(packet.kind, TestEvent::SPLIT_EVENT_KIND);
        assert!(packet.len > 0);

        let decoded: TestEvent = decode_split_event(&packet).expect("decode should succeed");
        assert_eq!(decoded, original);
    }

    #[test]
    fn decode_kind_mismatch_returns_none() {
        let event = TestEvent { value: 7, flag: false };
        let packet = encode_split_event(&event).expect("encode should succeed");

        // Try to decode as OtherEvent — kind won't match
        let result = decode_split_event::<OtherEvent>(&packet);
        assert!(result.is_none(), "kind mismatch should return None");
    }

    #[test]
    fn decode_corrupt_payload_returns_none() {
        // Build a packet with the right kind but garbage bytes that postcard
        // cannot deserialize into TestEvent.
        let corrupt = SplitUserPacket {
            kind: TestEvent::SPLIT_EVENT_KIND,
            len: crate::SPLIT_USER_PAYLOAD_MAX_SIZE as u8,
            data: [0xFF; crate::SPLIT_USER_PAYLOAD_MAX_SIZE],
        };

        let result = decode_split_event::<TestEvent>(&corrupt);
        assert!(result.is_none(), "corrupt payload should return None");
    }

    #[test]
    fn encoded_packet_has_correct_kind() {
        let event = OtherEvent { value: 0xDEAD_BEEF };
        let packet = encode_split_event(&event).expect("encode should succeed");
        assert_eq!(packet.kind, OtherEvent::SPLIT_EVENT_KIND);
    }

    #[test]
    fn decode_zero_len_packet_returns_none_for_nonempty_event() {
        let packet = SplitUserPacket {
            kind: TestEvent::SPLIT_EVENT_KIND,
            len: 0,
            data: [0u8; crate::SPLIT_USER_PAYLOAD_MAX_SIZE],
        };
        // A zero-length postcard buffer cannot represent a non-trivially-sized struct
        let result = decode_split_event::<TestEvent>(&packet);
        assert!(result.is_none(), "empty payload should not decode to a non-empty struct");
    }

    #[test]
    fn decode_oversized_len_returns_none() {
        let oversized_len = crate::SPLIT_USER_PAYLOAD_MAX_SIZE + 1;
        if oversized_len > u8::MAX as usize {
            return;
        }

        let packet = SplitUserPacket {
            kind: TestEvent::SPLIT_EVENT_KIND,
            len: oversized_len as u8,
            data: [0u8; crate::SPLIT_USER_PAYLOAD_MAX_SIZE],
        };
        let result = decode_split_event::<TestEvent>(&packet);
        assert!(result.is_none(), "oversized payload len should be rejected");
    }
}
