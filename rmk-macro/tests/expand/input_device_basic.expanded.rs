use rmk_macro::input_device;
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
        use ::rmk::event::publish_input_event_async;
        use ::rmk::input_device::InputDevice;
        loop {
            let event = self.read_event().await;
            publish_input_event_async(event).await;
        }
    }
}
