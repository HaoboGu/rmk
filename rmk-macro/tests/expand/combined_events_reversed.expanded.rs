use rmk_macro::{controller_event, input_event};
/// Test case: reversed order - #[controller_event] before #[input_event].
/// This should generate the same code as combined_events.rs.
pub struct DualChannelEventReversed {
    pub data: u16,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for DualChannelEventReversed {}
#[automatically_derived]
impl ::core::clone::Clone for DualChannelEventReversed {
    #[inline]
    fn clone(&self) -> DualChannelEventReversed {
        let _: ::core::clone::AssertParamIsClone<u16>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for DualChannelEventReversed {}
#[automatically_derived]
impl ::core::fmt::Debug for DualChannelEventReversed {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "DualChannelEventReversed",
            "data",
            &&self.data,
        )
    }
}
#[doc(hidden)]
static DUAL_CHANNEL_EVENT_REVERSED_CONTROLLER_CHANNEL: ::embassy_sync::pubsub::PubSubChannel<
    ::rmk::RawMutex,
    DualChannelEventReversed,
    { 1 },
    { 2 },
    { 1 },
> = ::embassy_sync::pubsub::PubSubChannel::new();
impl ::rmk::event::ControllerPublishEvent for DualChannelEventReversed {
    type Publisher = ::embassy_sync::pubsub::ImmediatePublisher<
        'static,
        ::rmk::RawMutex,
        DualChannelEventReversed,
        { 1 },
        { 2 },
        { 1 },
    >;
    fn controller_publisher() -> Self::Publisher {
        DUAL_CHANNEL_EVENT_REVERSED_CONTROLLER_CHANNEL.immediate_publisher()
    }
}
impl ::rmk::event::ControllerSubscribeEvent for DualChannelEventReversed {
    type Subscriber = ::embassy_sync::pubsub::Subscriber<
        'static,
        ::rmk::RawMutex,
        DualChannelEventReversed,
        { 1 },
        { 2 },
        { 1 },
    >;
    fn controller_subscriber() -> Self::Subscriber {
        DUAL_CHANNEL_EVENT_REVERSED_CONTROLLER_CHANNEL
            .subscriber()
            .expect(
                "Failed to create controller subscriber for DualChannelEventReversed. The \'subs\' limit has been exceeded. Increase the \'subs\' parameter in #[controller_event(subs = N)].",
            )
    }
}
impl ::rmk::event::AsyncControllerPublishEvent for DualChannelEventReversed {
    type AsyncPublisher = ::embassy_sync::pubsub::Publisher<
        'static,
        ::rmk::RawMutex,
        DualChannelEventReversed,
        { 1 },
        { 2 },
        { 1 },
    >;
    fn controller_publisher_async() -> Self::AsyncPublisher {
        DUAL_CHANNEL_EVENT_REVERSED_CONTROLLER_CHANNEL
            .publisher()
            .expect(
                "Failed to create async controller publisher for DualChannelEventReversed. The \'pubs\' limit has been exceeded. Increase the \'pubs\' parameter in #[controller_event(pubs = N)].",
            )
    }
}
#[doc(hidden)]
static DUAL_CHANNEL_EVENT_REVERSED_INPUT_CHANNEL: ::embassy_sync::channel::Channel<
    ::rmk::RawMutex,
    DualChannelEventReversed,
    { 8 },
> = ::embassy_sync::channel::Channel::new();
impl ::rmk::event::InputPublishEvent for DualChannelEventReversed {
    type Publisher = ::embassy_sync::channel::Sender<
        'static,
        ::rmk::RawMutex,
        DualChannelEventReversed,
        { 8 },
    >;
    fn input_publisher() -> Self::Publisher {
        DUAL_CHANNEL_EVENT_REVERSED_INPUT_CHANNEL.sender()
    }
}
impl ::rmk::event::InputSubscribeEvent for DualChannelEventReversed {
    type Subscriber = ::embassy_sync::channel::Receiver<
        'static,
        ::rmk::RawMutex,
        DualChannelEventReversed,
        { 8 },
    >;
    fn input_subscriber() -> Self::Subscriber {
        DUAL_CHANNEL_EVENT_REVERSED_INPUT_CHANNEL.receiver()
    }
}
impl ::rmk::event::AsyncInputPublishEvent for DualChannelEventReversed {
    type AsyncPublisher = ::embassy_sync::channel::Sender<
        'static,
        ::rmk::RawMutex,
        DualChannelEventReversed,
        { 8 },
    >;
    fn input_publisher_async() -> Self::AsyncPublisher {
        DUAL_CHANNEL_EVENT_REVERSED_INPUT_CHANNEL.sender()
    }
}
