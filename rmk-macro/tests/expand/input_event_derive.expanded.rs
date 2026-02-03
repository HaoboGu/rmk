use rmk_macro::InputEvent;
#[input_event]
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
#[input_event]
pub struct PointingEvent {
    pub x: i16,
    pub y: i16,
}
#[automatically_derived]
impl ::core::clone::Clone for PointingEvent {
    #[inline]
    fn clone(&self) -> PointingEvent {
        let _: ::core::clone::AssertParamIsClone<i16>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for PointingEvent {}
#[automatically_derived]
impl ::core::fmt::Debug for PointingEvent {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field2_finish(
            f,
            "PointingEvent",
            "x",
            &self.x,
            "y",
            &&self.y,
        )
    }
}
pub enum MultiSensorEvent {
    Battery(BatteryEvent),
    Pointing(PointingEvent),
}
/// Publisher for the wrapper enum.
/// Routes each variant to its event channel.
pub struct MultiSensorEventPublisher;
impl ::rmk::event::AsyncEventPublisher<MultiSensorEvent> for MultiSensorEventPublisher {
    async fn publish_async(&self, event: MultiSensorEvent) {
        match event {
            MultiSensorEvent::Battery(e) => {
                ::rmk::event::publish_input_event_async(e).await
            }
            MultiSensorEvent::Pointing(e) => {
                ::rmk::event::publish_input_event_async(e).await
            }
        }
    }
}
impl ::rmk::event::EventPublisher<MultiSensorEvent> for MultiSensorEventPublisher {
    fn publish(&self, event: MultiSensorEvent) {
        match event {
            MultiSensorEvent::Battery(e) => ::rmk::event::publish_input_event(e),
            MultiSensorEvent::Pointing(e) => ::rmk::event::publish_input_event(e),
        }
    }
}
/// Placeholder subscriber for wrapper enums.
/// Wrapper enums have no channel.
/// Subscribe to concrete event types instead.
pub struct MultiSensorEventSubscriber;
impl ::rmk::event::EventSubscriber<MultiSensorEvent> for MultiSensorEventSubscriber {
    async fn next_event(&mut self) -> MultiSensorEvent {
        core::future::pending().await
    }
}
impl ::rmk::event::InputEvent for MultiSensorEvent {
    type Publisher = MultiSensorEventPublisher;
    type Subscriber = MultiSensorEventSubscriber;
    fn input_publisher() -> Self::Publisher {
        MultiSensorEventPublisher
    }
    fn input_subscriber() -> Self::Subscriber {
        MultiSensorEventSubscriber
    }
}
impl ::rmk::event::AsyncInputEvent for MultiSensorEvent {
    type AsyncPublisher = MultiSensorEventPublisher;
    fn input_publisher_async() -> Self::AsyncPublisher {
        MultiSensorEventPublisher
    }
}
impl From<BatteryEvent> for MultiSensorEvent {
    fn from(e: BatteryEvent) -> Self {
        MultiSensorEvent::Battery(e)
    }
}
impl From<PointingEvent> for MultiSensorEvent {
    fn from(e: PointingEvent) -> Self {
        MultiSensorEvent::Pointing(e)
    }
}
