use rmk_macro::{controller, input_device};
pub struct SensorEvent {
    pub value: u16,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for SensorEvent {}
#[automatically_derived]
impl ::core::clone::Clone for SensorEvent {
    #[inline]
    fn clone(&self) -> SensorEvent {
        let _: ::core::clone::AssertParamIsClone<u16>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for SensorEvent {}
#[automatically_derived]
impl ::core::fmt::Debug for SensorEvent {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "SensorEvent",
            "value",
            &&self.value,
        )
    }
}
pub struct ConfigEvent {
    pub threshold: u16,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for ConfigEvent {}
#[automatically_derived]
impl ::core::clone::Clone for ConfigEvent {
    #[inline]
    fn clone(&self) -> ConfigEvent {
        let _: ::core::clone::AssertParamIsClone<u16>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for ConfigEvent {}
#[automatically_derived]
impl ::core::fmt::Debug for ConfigEvent {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "ConfigEvent",
            "threshold",
            &&self.threshold,
        )
    }
}
/// Test case: combined #[input_device] + #[controller] on the same struct.
/// This tests the runnable marker logic and select_biased! generation.
pub struct SensorController {
    pub threshold: u16,
}
impl ::rmk::controller::Controller for SensorController {
    type Event = ConfigEvent;
    async fn process_event(&mut self, event: Self::Event) {
        self.on_config_event(event).await
    }
}
impl ::rmk::input_device::InputDevice for SensorController {
    type Event = SensorEvent;
    async fn read_event(&mut self) -> Self::Event {
        self.read_sensor_event().await
    }
}
impl ::rmk::input_device::Runnable for SensorController {
    async fn run(&mut self) -> ! {
        use ::rmk::event::publish_input_event_async;
        use ::rmk::input_device::InputDevice;
        use ::rmk::event::SubscribableControllerEvent;
        use ::rmk::controller::Controller;
        use ::rmk::event::EventSubscriber;
        use ::rmk::futures::FutureExt;
        enum __RmkSelectEventSensorController {
            Input(SensorEvent),
            Controller(ConfigEvent),
        }
        let mut ctrl_sub0 = <ConfigEvent as ::rmk::event::SubscribableControllerEvent>::controller_subscriber();
        loop {
            let select_result = {
                {
                    use ::futures_util::__private as __futures_crate;
                    {
                        enum __PrivResult<_0, _1> {
                            _0(_0),
                            _1(_1),
                        }
                        let __select_result = {
                            let mut _0 = self.read_event().fuse();
                            let mut _1 = ctrl_sub0.next_event().fuse();
                            let mut __poll_fn = |
                                __cx: &mut __futures_crate::task::Context<'_>|
                            {
                                let mut __any_polled = false;
                                let mut _0 = |
                                    __cx: &mut __futures_crate::task::Context<'_>|
                                {
                                    let mut _0 = unsafe {
                                        __futures_crate::Pin::new_unchecked(&mut _0)
                                    };
                                    if __futures_crate::future::FusedFuture::is_terminated(
                                        &_0,
                                    ) {
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
                                ) -> __futures_crate::Option<
                                        __futures_crate::task::Poll<_>,
                                    > = &mut _0;
                                let mut _1 = |
                                    __cx: &mut __futures_crate::task::Context<'_>|
                                {
                                    let mut _1 = unsafe {
                                        __futures_crate::Pin::new_unchecked(&mut _1)
                                    };
                                    if __futures_crate::future::FusedFuture::is_terminated(
                                        &_1,
                                    ) {
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
                                ) -> __futures_crate::Option<
                                        __futures_crate::task::Poll<_>,
                                    > = &mut _1;
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
                                __RmkSelectEventSensorController::Input(event)
                            }
                            __PrivResult::_1(ctrl_event) => {
                                __RmkSelectEventSensorController::Controller(ctrl_event)
                            }
                        }
                    }
                }
            };
            match select_result {
                __RmkSelectEventSensorController::Input(event) => {
                    publish_input_event_async(event).await;
                }
                __RmkSelectEventSensorController::Controller(event) => {
                    <Self as ::rmk::controller::Controller>::process_event(self, event)
                        .await;
                }
            }
        }
    }
}
/// Test case: combined #[input_device] + #[controller] with polling.
/// This tests the timer arm placement in select_biased! (timer should be first).
pub struct PollingSensorController {
    pub threshold: u16,
    pub last_value: u16,
}
impl ::rmk::controller::Controller for PollingSensorController {
    type Event = ConfigEvent;
    async fn process_event(&mut self, event: Self::Event) {
        self.on_config_event(event).await
    }
}
impl ::rmk::controller::PollingController for PollingSensorController {
    fn interval(&self) -> ::embassy_time::Duration {
        ::embassy_time::Duration::from_millis(50u64)
    }
    async fn update(&mut self) {
        self.poll().await
    }
}
impl ::rmk::input_device::InputDevice for PollingSensorController {
    type Event = SensorEvent;
    async fn read_event(&mut self) -> Self::Event {
        self.read_sensor_event().await
    }
}
impl ::rmk::input_device::Runnable for PollingSensorController {
    async fn run(&mut self) -> ! {
        use ::rmk::event::publish_input_event_async;
        use ::rmk::input_device::InputDevice;
        use ::rmk::event::SubscribableControllerEvent;
        use ::rmk::controller::Controller;
        use ::rmk::event::EventSubscriber;
        use ::rmk::futures::FutureExt;
        use ::rmk::controller::PollingController;
        enum __RmkSelectEventPollingSensorController {
            Input(SensorEvent),
            Controller(ConfigEvent),
            Timer,
        }
        let mut ctrl_sub0 = <ConfigEvent as ::rmk::event::SubscribableControllerEvent>::controller_subscriber();
        let mut last = ::embassy_time::Instant::now();
        loop {
            let elapsed = last.elapsed();
            let interval = ::embassy_time::Duration::from_millis(50u64);
            let timer = ::embassy_time::Timer::after(
                interval.checked_sub(elapsed).unwrap_or(::embassy_time::Duration::MIN),
            );
            let select_result = {
                {
                    use ::futures_util::__private as __futures_crate;
                    {
                        enum __PrivResult<_0, _1, _2> {
                            _0(_0),
                            _1(_1),
                            _2(_2),
                        }
                        let __select_result = {
                            let mut _0 = timer.fuse();
                            let mut _1 = self.read_event().fuse();
                            let mut _2 = ctrl_sub0.next_event().fuse();
                            let mut __poll_fn = |
                                __cx: &mut __futures_crate::task::Context<'_>|
                            {
                                let mut __any_polled = false;
                                let mut _0 = |
                                    __cx: &mut __futures_crate::task::Context<'_>|
                                {
                                    let mut _0 = unsafe {
                                        __futures_crate::Pin::new_unchecked(&mut _0)
                                    };
                                    if __futures_crate::future::FusedFuture::is_terminated(
                                        &_0,
                                    ) {
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
                                ) -> __futures_crate::Option<
                                        __futures_crate::task::Poll<_>,
                                    > = &mut _0;
                                let mut _1 = |
                                    __cx: &mut __futures_crate::task::Context<'_>|
                                {
                                    let mut _1 = unsafe {
                                        __futures_crate::Pin::new_unchecked(&mut _1)
                                    };
                                    if __futures_crate::future::FusedFuture::is_terminated(
                                        &_1,
                                    ) {
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
                                ) -> __futures_crate::Option<
                                        __futures_crate::task::Poll<_>,
                                    > = &mut _1;
                                let mut _2 = |
                                    __cx: &mut __futures_crate::task::Context<'_>|
                                {
                                    let mut _2 = unsafe {
                                        __futures_crate::Pin::new_unchecked(&mut _2)
                                    };
                                    if __futures_crate::future::FusedFuture::is_terminated(
                                        &_2,
                                    ) {
                                        __futures_crate::None
                                    } else {
                                        __futures_crate::Some(
                                            __futures_crate::future::FutureExt::poll_unpin(
                                                    &mut _2,
                                                    __cx,
                                                )
                                                .map(__PrivResult::_2),
                                        )
                                    }
                                };
                                let _2: &mut dyn FnMut(
                                    &mut __futures_crate::task::Context<'_>,
                                ) -> __futures_crate::Option<
                                        __futures_crate::task::Poll<_>,
                                    > = &mut _2;
                                let mut __select_arr = [_0, _1, _2];
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
                            __PrivResult::_0(_) => {
                                __RmkSelectEventPollingSensorController::Timer
                            }
                            __PrivResult::_1(event) => {
                                __RmkSelectEventPollingSensorController::Input(event)
                            }
                            __PrivResult::_2(ctrl_event) => {
                                __RmkSelectEventPollingSensorController::Controller(
                                    ctrl_event,
                                )
                            }
                        }
                    }
                }
            };
            match select_result {
                __RmkSelectEventPollingSensorController::Input(event) => {
                    publish_input_event_async(event).await;
                }
                __RmkSelectEventPollingSensorController::Controller(event) => {
                    <Self as ::rmk::controller::Controller>::process_event(self, event)
                        .await;
                }
                __RmkSelectEventPollingSensorController::Timer => {
                    <Self as PollingController>::update(self).await;
                    last = ::embassy_time::Instant::now();
                }
            }
        }
    }
}
