use rmk_macro::controller_event;
pub struct BatteryEvent {
    pub level: u8,
}
#[automatically_derived]
impl ::core::clone::Clone for BatteryEvent {
    #[inline]
    fn clone(&self) -> BatteryEvent {
        let _: ::core::clone::AssertParamIsClone<u8>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for BatteryEvent {}
#[automatically_derived]
impl ::core::fmt::Debug for BatteryEvent {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "BatteryEvent",
            "level",
            &&self.level,
        )
    }
}
static BATTERY_EVENT_CONTROLLER_CHANNEL: ::embassy_sync::pubsub::PubSubChannel<
    ::rmk::RawMutex,
    BatteryEvent,
    { 4 },
    { 2 },
    { 1 },
> = ::embassy_sync::pubsub::PubSubChannel::new();
impl ::rmk::event::ControllerEvent for BatteryEvent {
    type Publisher = ::embassy_sync::pubsub::ImmediatePublisher<
        'static,
        ::rmk::RawMutex,
        BatteryEvent,
        { 4 },
        { 2 },
        { 1 },
    >;
    type Subscriber = ::embassy_sync::pubsub::Subscriber<
        'static,
        ::rmk::RawMutex,
        BatteryEvent,
        { 4 },
        { 2 },
        { 1 },
    >;
    fn controller_publisher() -> Self::Publisher {
        BATTERY_EVENT_CONTROLLER_CHANNEL.immediate_publisher()
    }
    fn controller_subscriber() -> Self::Subscriber {
        BATTERY_EVENT_CONTROLLER_CHANNEL.subscriber().unwrap()
    }
}
impl ::rmk::event::AsyncControllerEvent for BatteryEvent {
    type AsyncPublisher = ::embassy_sync::pubsub::Publisher<
        'static,
        ::rmk::RawMutex,
        BatteryEvent,
        { 4 },
        { 2 },
        { 1 },
    >;
    fn controller_publisher_async() -> Self::AsyncPublisher {
        BATTERY_EVENT_CONTROLLER_CHANNEL.publisher().unwrap()
    }
}
