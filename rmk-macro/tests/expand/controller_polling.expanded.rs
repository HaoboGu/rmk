use rmk_macro::controller;
pub struct LedStateEvent {
    pub on: bool,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for LedStateEvent {}
#[automatically_derived]
impl ::core::clone::Clone for LedStateEvent {
    #[inline]
    fn clone(&self) -> LedStateEvent {
        let _: ::core::clone::AssertParamIsClone<bool>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for LedStateEvent {}
#[automatically_derived]
impl ::core::fmt::Debug for LedStateEvent {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "LedStateEvent",
            "on",
            &&self.on,
        )
    }
}
pub struct PollingLedController {
    pub pin: u8,
}
impl ::rmk::controller::Controller for PollingLedController {
    type Event = LedStateEvent;
    async fn process_event(&mut self, event: Self::Event) {
        self.on_led_state_event(event).await
    }
}
impl ::rmk::controller::PollingController for PollingLedController {
    fn interval(&self) -> ::embassy_time::Duration {
        ::embassy_time::Duration::from_millis(100u64)
    }
    async fn update(&mut self) {
        self.poll().await
    }
}
impl ::rmk::input_device::Runnable for PollingLedController {
    async fn run(&mut self) -> ! {
        use ::rmk::controller::PollingController;
        self.polling_loop().await
    }
}
