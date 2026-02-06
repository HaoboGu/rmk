use rmk_macro::input_processor;
pub struct KeyEvent {
    pub row: u8,
    pub col: u8,
    pub pressed: bool,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for KeyEvent {}
#[automatically_derived]
impl ::core::clone::Clone for KeyEvent {
    #[inline]
    fn clone(&self) -> KeyEvent {
        let _: ::core::clone::AssertParamIsClone<u8>;
        let _: ::core::clone::AssertParamIsClone<bool>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for KeyEvent {}
#[automatically_derived]
impl ::core::fmt::Debug for KeyEvent {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field3_finish(
            f,
            "KeyEvent",
            "row",
            &self.row,
            "col",
            &self.col,
            "pressed",
            &&self.pressed,
        )
    }
}
pub struct EncoderEvent {
    pub index: u8,
    pub direction: i8,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for EncoderEvent {}
#[automatically_derived]
impl ::core::clone::Clone for EncoderEvent {
    #[inline]
    fn clone(&self) -> EncoderEvent {
        let _: ::core::clone::AssertParamIsClone<u8>;
        let _: ::core::clone::AssertParamIsClone<i8>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for EncoderEvent {}
#[automatically_derived]
impl ::core::fmt::Debug for EncoderEvent {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field2_finish(
            f,
            "EncoderEvent",
            "index",
            &self.index,
            "direction",
            &&self.direction,
        )
    }
}
pub struct KeyProcessor;
pub enum KeyProcessorEventEnum {
    Key(KeyEvent),
    Encoder(EncoderEvent),
}
#[automatically_derived]
impl ::core::clone::Clone for KeyProcessorEventEnum {
    #[inline]
    fn clone(&self) -> KeyProcessorEventEnum {
        match self {
            KeyProcessorEventEnum::Key(__self_0) => {
                KeyProcessorEventEnum::Key(::core::clone::Clone::clone(__self_0))
            }
            KeyProcessorEventEnum::Encoder(__self_0) => {
                KeyProcessorEventEnum::Encoder(::core::clone::Clone::clone(__self_0))
            }
        }
    }
}
/// Event subscriber for aggregated events
pub struct KeyProcessorEventSubscriber {
    sub0: <KeyEvent as ::rmk::event::InputSubscribeEvent>::Subscriber,
    sub1: <EncoderEvent as ::rmk::event::InputSubscribeEvent>::Subscriber,
}
impl KeyProcessorEventSubscriber {
    /// Create a new event subscriber
    pub fn new() -> Self {
        Self {
            sub0: <KeyEvent as ::rmk::event::InputSubscribeEvent>::input_subscriber(),
            sub1: <EncoderEvent as ::rmk::event::InputSubscribeEvent>::input_subscriber(),
        }
    }
}
impl ::rmk::event::EventSubscriber for KeyProcessorEventSubscriber {
    type Event = KeyProcessorEventEnum;
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
                    __PrivResult::_0(event) => KeyProcessorEventEnum::Key(event),
                    __PrivResult::_1(event) => KeyProcessorEventEnum::Encoder(event),
                }
            }
        }
    }
}
impl ::rmk::event::InputSubscribeEvent for KeyProcessorEventEnum {
    type Subscriber = KeyProcessorEventSubscriber;
    fn input_subscriber() -> Self::Subscriber {
        KeyProcessorEventSubscriber::new()
    }
}
impl ::rmk::input_device::Runnable for KeyProcessor {
    async fn run(&mut self) -> ! {
        use ::rmk::input_device::InputProcessor;
        self.process_loop().await
    }
}
impl ::rmk::input_device::InputProcessor for KeyProcessor {
    type Event = KeyProcessorEventEnum;
    async fn process(&mut self, event: Self::Event) {
        match event {
            KeyProcessorEventEnum::Key(event) => self.on_key_event(event).await,
            KeyProcessorEventEnum::Encoder(event) => self.on_encoder_event(event).await,
        }
    }
}
