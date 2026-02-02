use rmk_macro::controller;
pub struct LedStateEvent {
    pub on: bool,
}
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
pub enum PollingLedControllerEventEnum {
    Event0(LedStateEvent),
}
impl From<LedStateEvent> for PollingLedControllerEventEnum {
    fn from(e: LedStateEvent) -> Self {
        PollingLedControllerEventEnum::Event0(e)
    }
}
impl ::rmk::controller::Controller for PollingLedController {
    type Event = PollingLedControllerEventEnum;
    async fn process_event(&mut self, event: Self::Event) {
        match event {
            PollingLedControllerEventEnum::Event0(event) => {
                self.on_led_state_event(event).await
            }
        }
    }
    async fn next_message(&mut self) -> Self::Event {
        use ::rmk::event::EventSubscriber;
        use ::futures::FutureExt;
        let mut sub0 = <LedStateEvent as ::rmk::event::ControllerEvent>::controller_subscriber();
        {
            use ::futures_util::__private as __futures_crate;
            {
                enum __PrivResult<_0> {
                    _0(_0),
                }
                let __select_result = {
                    let mut _0 = sub0.next_event().fuse();
                    let mut __poll_fn = |__cx: &mut __futures_crate::task::Context<'_>| {
                        let mut __any_polled = false;
                        let mut _0 = |__cx: &mut __futures_crate::task::Context<'_>| {
                            let mut _0 = unsafe {
                                __futures_crate::Pin::new_unchecked(&mut _0)
                            };
                            if __futures_crate::future::FusedFuture::is_terminated(&_0) {
                                __futures_crate::None
                            } else {
                                __futures_crate::Some(
                                    __futures_crate::future::FutureExt::poll_unpin(
                                            &mut _0,
                                            __cx,
                                        )
                                        .map(__PrivResult::_0),
                                )
                            }
                        };
                        let _0: &mut dyn FnMut(
                            &mut __futures_crate::task::Context<'_>,
                        ) -> __futures_crate::Option<__futures_crate::task::Poll<_>> = &mut _0;
                        let mut __select_arr = [_0];
                        for poller in &mut __select_arr {
                            let poller: &mut &mut dyn FnMut(
                                &mut __futures_crate::task::Context<'_>,
                            ) -> __futures_crate::Option<
                                    __futures_crate::task::Poll<_>,
                                > = poller;
                            match poller(__cx) {
                                __futures_crate::Some(
                                    x @ __futures_crate::task::Poll::Ready(_),
                                ) => return x,
                                __futures_crate::Some(
                                    __futures_crate::task::Poll::Pending,
                                ) => {
                                    __any_polled = true;
                                }
                                __futures_crate::None => {}
                            }
                        }
                        if !__any_polled {
                            {
                                ::std::rt::begin_panic(
                                    "all futures in select! were completed,\
                    but no `complete =>` handler was provided",
                                );
                            }
                        } else {
                            __futures_crate::task::Poll::Pending
                        }
                    };
                    __futures_crate::future::poll_fn(__poll_fn).await
                };
                match __select_result {
                    __PrivResult::_0(event) => {
                        PollingLedControllerEventEnum::Event0(event)
                    }
                }
            }
        }
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
