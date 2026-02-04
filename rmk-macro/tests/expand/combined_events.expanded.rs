use rmk_macro::{controller_event, input_event};
pub struct DualChannelEvent {
    pub data: u16,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for DualChannelEvent {}
#[automatically_derived]
impl ::core::clone::Clone for DualChannelEvent {
    #[inline]
    fn clone(&self) -> DualChannelEvent {
        let _: ::core::clone::AssertParamIsClone<u16>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for DualChannelEvent {}
#[automatically_derived]
impl ::core::fmt::Debug for DualChannelEvent {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "DualChannelEvent",
            "data",
            &&self.data,
        )
    }
}
static DUAL_CHANNEL_EVENT_INPUT_CHANNEL: ::embassy_sync::channel::Channel<
    ::rmk::RawMutex,
    DualChannelEvent,
    { 8 },
> = ::embassy_sync::channel::Channel::new();
impl ::rmk::event::InputEvent for DualChannelEvent {
    type Publisher = ::embassy_sync::channel::Sender<
        'static,
        ::rmk::RawMutex,
        DualChannelEvent,
        { 8 },
    >;
    type Subscriber = ::embassy_sync::channel::Receiver<
        'static,
        ::rmk::RawMutex,
        DualChannelEvent,
        { 8 },
    >;
    fn input_publisher() -> Self::Publisher {
        DUAL_CHANNEL_EVENT_INPUT_CHANNEL.sender()
    }
    fn input_subscriber() -> Self::Subscriber {
        DUAL_CHANNEL_EVENT_INPUT_CHANNEL.receiver()
    }
}
impl ::rmk::event::AsyncInputEvent for DualChannelEvent {
    type AsyncPublisher = ::embassy_sync::channel::Sender<
        'static,
        ::rmk::RawMutex,
        DualChannelEvent,
        { 8 },
    >;
    fn input_publisher_async() -> Self::AsyncPublisher {
        DUAL_CHANNEL_EVENT_INPUT_CHANNEL.sender()
    }
}
static DUAL_CHANNEL_EVENT_CONTROLLER_CHANNEL: ::embassy_sync::pubsub::PubSubChannel<
    ::rmk::RawMutex,
    DualChannelEvent,
    { 1 },
    { 2 },
    { 1 },
> = ::embassy_sync::pubsub::PubSubChannel::new();
impl ::rmk::event::ControllerEvent for DualChannelEvent {
    type Publisher = ::embassy_sync::pubsub::ImmediatePublisher<
        'static,
        ::rmk::RawMutex,
        DualChannelEvent,
        { 1 },
        { 2 },
        { 1 },
    >;
    type Subscriber = ::embassy_sync::pubsub::Subscriber<
        'static,
        ::rmk::RawMutex,
        DualChannelEvent,
        { 1 },
        { 2 },
        { 1 },
    >;
    fn controller_publisher() -> Self::Publisher {
        DUAL_CHANNEL_EVENT_CONTROLLER_CHANNEL.immediate_publisher()
    }
    fn controller_subscriber() -> Self::Subscriber {
        DUAL_CHANNEL_EVENT_CONTROLLER_CHANNEL.subscriber().unwrap()
    }
}
impl ::rmk::event::AsyncControllerEvent for DualChannelEvent {
    type AsyncPublisher = ::embassy_sync::pubsub::Publisher<
        'static,
        ::rmk::RawMutex,
        DualChannelEvent,
        { 1 },
        { 2 },
        { 1 },
    >;
    fn controller_publisher_async() -> Self::AsyncPublisher {
        DUAL_CHANNEL_EVENT_CONTROLLER_CHANNEL.publisher().unwrap()
    }
}
