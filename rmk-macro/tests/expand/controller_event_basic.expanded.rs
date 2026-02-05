use rmk_macro::controller_event;
pub struct BatteryEvent {
    pub level: u8,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for BatteryEvent {}
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
#[doc(hidden)]
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
        BATTERY_EVENT_CONTROLLER_CHANNEL
            .subscriber()
            .expect(
                "Failed to create controller subscriber for BatteryEvent. The \'subs\' limit has been exceeded. Increase the \'subs\' parameter in #[controller_event(subs = N)].",
            )
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
        BATTERY_EVENT_CONTROLLER_CHANNEL
            .publisher()
            .expect(
                "Failed to create async controller publisher for BatteryEvent. The \'pubs\' limit has been exceeded. Increase the \'pubs\' parameter in #[controller_event(pubs = N)].",
            )
    }
}
/// Battery state changed event
pub enum BatteryState {
    /// The battery state is not available
    NotAvailable,
    /// The value range is 0~100
    Normal(u8),
    /// Battery is currently charging
    Charging,
    /// Charging completed, ideally the battery level after charging completed is 100
    Charged,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for BatteryState {}
#[automatically_derived]
impl ::core::clone::Clone for BatteryState {
    #[inline]
    fn clone(&self) -> BatteryState {
        let _: ::core::clone::AssertParamIsClone<u8>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for BatteryState {}
#[automatically_derived]
impl ::core::fmt::Debug for BatteryState {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        match self {
            BatteryState::NotAvailable => {
                ::core::fmt::Formatter::write_str(f, "NotAvailable")
            }
            BatteryState::Normal(__self_0) => {
                ::core::fmt::Formatter::debug_tuple_field1_finish(f, "Normal", &__self_0)
            }
            BatteryState::Charging => ::core::fmt::Formatter::write_str(f, "Charging"),
            BatteryState::Charged => ::core::fmt::Formatter::write_str(f, "Charged"),
        }
    }
}
#[doc(hidden)]
static BATTERY_STATE_CONTROLLER_CHANNEL: ::embassy_sync::pubsub::PubSubChannel<
    ::rmk::RawMutex,
    BatteryState,
    { 8 },
    { 3 },
    { 2 },
> = ::embassy_sync::pubsub::PubSubChannel::new();
impl ::rmk::event::ControllerEvent for BatteryState {
    type Publisher = ::embassy_sync::pubsub::ImmediatePublisher<
        'static,
        ::rmk::RawMutex,
        BatteryState,
        { 8 },
        { 3 },
        { 2 },
    >;
    type Subscriber = ::embassy_sync::pubsub::Subscriber<
        'static,
        ::rmk::RawMutex,
        BatteryState,
        { 8 },
        { 3 },
        { 2 },
    >;
    fn controller_publisher() -> Self::Publisher {
        BATTERY_STATE_CONTROLLER_CHANNEL.immediate_publisher()
    }
    fn controller_subscriber() -> Self::Subscriber {
        BATTERY_STATE_CONTROLLER_CHANNEL
            .subscriber()
            .expect(
                "Failed to create controller subscriber for BatteryState. The \'subs\' limit has been exceeded. Increase the \'subs\' parameter in #[controller_event(subs = N)].",
            )
    }
}
impl ::rmk::event::AsyncControllerEvent for BatteryState {
    type AsyncPublisher = ::embassy_sync::pubsub::Publisher<
        'static,
        ::rmk::RawMutex,
        BatteryState,
        { 8 },
        { 3 },
        { 2 },
    >;
    fn controller_publisher_async() -> Self::AsyncPublisher {
        BATTERY_STATE_CONTROLLER_CHANNEL
            .publisher()
            .expect(
                "Failed to create async controller publisher for BatteryState. The \'pubs\' limit has been exceeded. Increase the \'pubs\' parameter in #[controller_event(pubs = N)].",
            )
    }
}
