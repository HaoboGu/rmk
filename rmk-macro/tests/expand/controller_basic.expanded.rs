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
pub enum LedControllerControllerEventEnum {
    LedState(LedStateEvent),
    Brightness(BrightnessEvent),
}
#[automatically_derived]
impl ::core::clone::Clone for LedControllerControllerEventEnum {
    #[inline]
    fn clone(&self) -> LedControllerControllerEventEnum {
        match self {
            LedControllerControllerEventEnum::LedState(__self_0) => {
                LedControllerControllerEventEnum::LedState(
                    ::core::clone::Clone::clone(__self_0),
                )
            }
            LedControllerControllerEventEnum::Brightness(__self_0) => {
                LedControllerControllerEventEnum::Brightness(
                    ::core::clone::Clone::clone(__self_0),
                )
            }
        }
    }
}
/// Event subscriber for aggregated events
pub struct LedControllerControllerEventSubscriber {
    sub0: <LedStateEvent as ::rmk::event::ControllerSubscribeEvent>::Subscriber,
    sub1: <BrightnessEvent as ::rmk::event::ControllerSubscribeEvent>::Subscriber,
}
impl LedControllerControllerEventSubscriber {
    /// Create a new event subscriber
    pub fn new() -> Self {
        Self {
            sub0: <LedStateEvent as ::rmk::event::ControllerSubscribeEvent>::controller_subscriber(),
            sub1: <BrightnessEvent as ::rmk::event::ControllerSubscribeEvent>::controller_subscriber(),
        }
    }
}
impl ::rmk::event::EventSubscriber for LedControllerControllerEventSubscriber {
    type Event = LedControllerControllerEventEnum;
    async fn next_event(&mut self) -> Self::Event {
        use ::rmk::event::EventSubscriber;
        use ::rmk::futures::FutureExt;
        {
            use ::futures_util::__private as __futures_crate;
            {
                enum __PrivResult<_0, _1> {
                    _0(_0),
                    _1(_1),
                }
                let __select_result = {
                    let mut _0 = self.sub0.next_event().fuse();
                    let mut _1 = self.sub1.next_event().fuse();
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
                    __PrivResult::_0(event) => {
                        LedControllerControllerEventEnum::LedState(event)
                    }
                    __PrivResult::_1(event) => {
                        LedControllerControllerEventEnum::Brightness(event)
                    }
                }
            }
        }
    }
}
impl ::rmk::event::ControllerSubscribeEvent for LedControllerControllerEventEnum {
    type Subscriber = LedControllerControllerEventSubscriber;
    fn controller_subscriber() -> Self::Subscriber {
        LedControllerControllerEventSubscriber::new()
    }
}
impl ::rmk::controller::Controller for LedController {
    type Event = LedControllerControllerEventEnum;
    async fn process_event(&mut self, event: Self::Event) {
        match event {
            LedControllerControllerEventEnum::LedState(event) => {
                self.on_led_state_event(event).await
            }
            LedControllerControllerEventEnum::Brightness(event) => {
                self.on_brightness_event(event).await
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
