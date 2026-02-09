use rmk_macro::{InputEvent, input_device};
pub struct PointingEvent {}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for PointingEvent {}
#[automatically_derived]
impl ::core::clone::Clone for PointingEvent {
    #[inline]
    fn clone(&self) -> PointingEvent {
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for PointingEvent {}
#[automatically_derived]
impl ::core::fmt::Debug for PointingEvent {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::write_str(f, "PointingEvent")
    }
}
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
pub enum NrfAdcEvent {
    Pointing(PointingEvent),
    Battery(BatteryEvent),
}
/// Publisher for the wrapper enum.
/// Routes each variant to its event channel.
pub struct NrfAdcEventPublisher;
impl ::rmk::event::AsyncEventPublisher for NrfAdcEventPublisher {
    type Event = NrfAdcEvent;
    async fn publish_async(&self, event: NrfAdcEvent) {
        match event {
            NrfAdcEvent::Pointing(e) => ::rmk::event::publish_event_async(e).await,
            NrfAdcEvent::Battery(e) => ::rmk::event::publish_event_async(e).await,
        }
    }
}
impl ::rmk::event::EventPublisher for NrfAdcEventPublisher {
    type Event = NrfAdcEvent;
    fn publish(&self, event: NrfAdcEvent) {
        match event {
            NrfAdcEvent::Pointing(e) => ::rmk::event::publish_event(e),
            NrfAdcEvent::Battery(e) => ::rmk::event::publish_event(e),
        }
    }
}
impl ::rmk::event::PublishableEvent for NrfAdcEvent {
    type Publisher = NrfAdcEventPublisher;
    fn publisher() -> Self::Publisher {
        NrfAdcEventPublisher
    }
}
impl ::rmk::event::AsyncPublishableEvent for NrfAdcEvent {
    type AsyncPublisher = NrfAdcEventPublisher;
    fn publisher_async() -> Self::AsyncPublisher {
        NrfAdcEventPublisher
    }
}
impl From<PointingEvent> for NrfAdcEvent {
    fn from(e: PointingEvent) -> Self {
        NrfAdcEvent::Pointing(e)
    }
}
impl From<BatteryEvent> for NrfAdcEvent {
    fn from(e: BatteryEvent) -> Self {
        NrfAdcEvent::Battery(e)
    }
}
#[automatically_derived]
impl ::core::clone::Clone for NrfAdcEvent {
    #[inline]
    fn clone(&self) -> NrfAdcEvent {
        match self {
            NrfAdcEvent::Pointing(__self_0) => {
                NrfAdcEvent::Pointing(::core::clone::Clone::clone(__self_0))
            }
            NrfAdcEvent::Battery(__self_0) => {
                NrfAdcEvent::Battery(::core::clone::Clone::clone(__self_0))
            }
        }
    }
}
#[automatically_derived]
impl ::core::fmt::Debug for NrfAdcEvent {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        match self {
            NrfAdcEvent::Pointing(__self_0) => {
                ::core::fmt::Formatter::debug_tuple_field1_finish(
                    f,
                    "Pointing",
                    &__self_0,
                )
            }
            NrfAdcEvent::Battery(__self_0) => {
                ::core::fmt::Formatter::debug_tuple_field1_finish(
                    f,
                    "Battery",
                    &__self_0,
                )
            }
        }
    }
}
mod basic {
    use super::{BatteryEvent, input_device};
    pub struct BatteryReader {
        pub pin: u8,
    }
    impl ::rmk::input_device::InputDevice for BatteryReader {
        type Event = BatteryEvent;
        async fn read_event(&mut self) -> Self::Event {
            self.read_battery_event().await
        }
    }
    impl ::rmk::input_device::Runnable for BatteryReader {
        async fn run(&mut self) -> ! {
            use ::rmk::event::publish_event_async;
            use ::rmk::input_device::InputDevice;
            loop {
                let event = self.read_event().await;
                publish_event_async(event).await;
            }
        }
    }
}
mod multi_event {
    use super::{NrfAdcEvent, input_device};
    pub struct NrfAdc<'a, const PIN_NUM: usize, const EVENT_NUM: usize> {
        saadc: Saadc<'a, PIN_NUM>,
        polling_interval: Duration,
        light_sleep: Option<Duration>,
        buf: [[i16; PIN_NUM]; 2],
        event_type: [AnalogEventType; EVENT_NUM],
        event_state: u8,
        channel_state: u8,
        buf_state: bool,
        adc_state: AdcState,
        active_instant: Instant,
    }
    impl<
        'a,
        const PIN_NUM: usize,
        const EVENT_NUM: usize,
    > ::rmk::input_device::InputDevice for NrfAdc<'a, PIN_NUM, EVENT_NUM> {
        type Event = NrfAdcEvent;
        async fn read_event(&mut self) -> Self::Event {
            self.read_nrf_adc_event().await
        }
    }
    impl<'a, const PIN_NUM: usize, const EVENT_NUM: usize> ::rmk::input_device::Runnable
    for NrfAdc<'a, PIN_NUM, EVENT_NUM> {
        async fn run(&mut self) -> ! {
            use ::rmk::event::publish_event_async;
            use ::rmk::input_device::InputDevice;
            loop {
                let event = self.read_event().await;
                publish_event_async(event).await;
            }
        }
    }
}
