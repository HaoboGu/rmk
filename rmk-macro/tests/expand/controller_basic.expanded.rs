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
pub struct BrightnessEvent {
    pub level: u8,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for BrightnessEvent {}
#[automatically_derived]
impl ::core::clone::Clone for BrightnessEvent {
    #[inline]
    fn clone(&self) -> BrightnessEvent {
        let _: ::core::clone::AssertParamIsClone<u8>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for BrightnessEvent {}
#[automatically_derived]
impl ::core::fmt::Debug for BrightnessEvent {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "BrightnessEvent",
            "level",
            &&self.level,
        )
    }
}
pub struct LedController {
    pub pin: u8,
}
pub enum LedControllerEventEnum {
    Event0(LedStateEvent),
    Event1(BrightnessEvent),
}
impl From<LedStateEvent> for LedControllerEventEnum {
    fn from(e: LedStateEvent) -> Self {
        LedControllerEventEnum::Event0(e)
    }
}
impl From<BrightnessEvent> for LedControllerEventEnum {
    fn from(e: BrightnessEvent) -> Self {
        LedControllerEventEnum::Event1(e)
    }
}
impl ::rmk::controller::Controller for LedController {
    type Event = LedControllerEventEnum;
    async fn process_event(&mut self, event: Self::Event) {
        match event {
            LedControllerEventEnum::Event0(event) => self.on_led_state_event(event).await,
            LedControllerEventEnum::Event1(event) => {
                self.on_brightness_event(event).await
            }
        }
    }
    async fn next_message(&mut self) -> Self::Event {
        use ::rmk::event::EventSubscriber;
        use ::rmk::futures::FutureExt;
        let mut sub0 = <LedStateEvent as ::rmk::event::ControllerEvent>::controller_subscriber();
        let mut sub1 = <BrightnessEvent as ::rmk::event::ControllerEvent>::controller_subscriber();
        {
            use ::futures_util::__private as __futures_crate;
            {
                enum __PrivResult<_0, _1> {
                    _0(_0),
                    _1(_1),
                }
                let __select_result = {
                    let mut _0 = sub0.next_event().fuse();
                    let mut _1 = sub1.next_event().fuse();
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
                        let mut _1 = |__cx: &mut __futures_crate::task::Context<'_>| {
                            let mut _1 = unsafe {
                                __futures_crate::Pin::new_unchecked(&mut _1)
                            };
                            if __futures_crate::future::FusedFuture::is_terminated(&_1) {
                                __futures_crate::None
                            } else {
                                __futures_crate::Some(
                                    __futures_crate::future::FutureExt::poll_unpin(
                                            &mut _1,
                                            __cx,
                                        )
                                        .map(__PrivResult::_1),
                                )
                            }
                        };
                        let _1: &mut dyn FnMut(
                            &mut __futures_crate::task::Context<'_>,
                        ) -> __futures_crate::Option<__futures_crate::task::Poll<_>> = &mut _1;
                        let mut __select_arr = [_0, _1];
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
                    __PrivResult::_0(event) => LedControllerEventEnum::Event0(event),
                    __PrivResult::_1(event) => LedControllerEventEnum::Event1(event),
                }
            }
        }
    }
}
impl ::rmk::input_device::Runnable for LedController {
    async fn run(&mut self) -> ! {
        use ::rmk::controller::EventController;
        self.event_loop().await
    }
}
