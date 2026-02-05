use rmk_macro::input_event;
pub struct TestEvent {
    pub value: u8,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for TestEvent {}
#[automatically_derived]
impl ::core::clone::Clone for TestEvent {
    #[inline]
    fn clone(&self) -> TestEvent {
        let _: ::core::clone::AssertParamIsClone<u8>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for TestEvent {}
#[automatically_derived]
impl ::core::fmt::Debug for TestEvent {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "TestEvent",
            "value",
            &&self.value,
        )
    }
}
#[doc(hidden)]
static TEST_EVENT_INPUT_CHANNEL: ::embassy_sync::channel::Channel<
    ::rmk::RawMutex,
    TestEvent,
    { 16 },
> = ::embassy_sync::channel::Channel::new();
impl ::rmk::event::InputEvent for TestEvent {
    type Publisher = ::embassy_sync::channel::Sender<
        'static,
        ::rmk::RawMutex,
        TestEvent,
        { 16 },
    >;
    type Subscriber = ::embassy_sync::channel::Receiver<
        'static,
        ::rmk::RawMutex,
        TestEvent,
        { 16 },
    >;
    fn input_publisher() -> Self::Publisher {
        TEST_EVENT_INPUT_CHANNEL.sender()
    }
    fn input_subscriber() -> Self::Subscriber {
        TEST_EVENT_INPUT_CHANNEL.receiver()
    }
}
impl ::rmk::event::AsyncInputEvent for TestEvent {
    type AsyncPublisher = ::embassy_sync::channel::Sender<
        'static,
        ::rmk::RawMutex,
        TestEvent,
        { 16 },
    >;
    fn input_publisher_async() -> Self::AsyncPublisher {
        TEST_EVENT_INPUT_CHANNEL.sender()
    }
}
