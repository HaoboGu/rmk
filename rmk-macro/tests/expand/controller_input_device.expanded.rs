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
pub struct ModeEvent {
    pub enabled: bool,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for ModeEvent {}
#[automatically_derived]
impl ::core::clone::Clone for ModeEvent {
    #[inline]
    fn clone(&self) -> ModeEvent {
        let _: ::core::clone::AssertParamIsClone<bool>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for ModeEvent {}
#[automatically_derived]
impl ::core::fmt::Debug for ModeEvent {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "ModeEvent",
            "enabled",
            &&self.enabled,
        )
    }
}
mod basic {
    use super::{ConfigEvent, SensorEvent, controller, input_device};
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
                Controller(<Self as ::rmk::controller::Controller>::Event),
            }
            let mut ctrl_sub = <<Self as ::rmk::controller::Controller>::Event as ::rmk::event::SubscribableControllerEvent>::controller_subscriber();
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
                                let mut _1 = ctrl_sub.next_event().fuse();
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
                        <Self as ::rmk::controller::Controller>::process_event(
                                self,
                                event,
                            )
                            .await;
                    }
                }
            }
        }
    }
}
mod polling {
    use super::{ConfigEvent, SensorEvent, controller, input_device};
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
                Controller(<Self as ::rmk::controller::Controller>::Event),
                Timer,
            }
            let mut ctrl_sub = <<Self as ::rmk::controller::Controller>::Event as ::rmk::event::SubscribableControllerEvent>::controller_subscriber();
            let mut last = ::embassy_time::Instant::now();
            loop {
                let elapsed = last.elapsed();
                let interval = ::embassy_time::Duration::from_millis(50u64);
                let timer = ::embassy_time::Timer::after(
                    interval
                        .checked_sub(elapsed)
                        .unwrap_or(::embassy_time::Duration::MIN),
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
                                let mut _2 = ctrl_sub.next_event().fuse();
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
                        <Self as ::rmk::controller::Controller>::process_event(
                                self,
                                event,
                            )
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
}
mod reversed {
    use super::{ConfigEvent, SensorEvent, controller, input_device};
    pub struct ReversedSensorController {
        pub threshold: u16,
    }
    impl ::rmk::input_device::InputDevice for ReversedSensorController {
        type Event = SensorEvent;
        async fn read_event(&mut self) -> Self::Event {
            self.read_sensor_event().await
        }
    }
    impl ::rmk::controller::Controller for ReversedSensorController {
        type Event = ConfigEvent;
        async fn process_event(&mut self, event: Self::Event) {
            self.on_config_event(event).await
        }
    }
    impl ::rmk::input_device::Runnable for ReversedSensorController {
        async fn run(&mut self) -> ! {
            use ::rmk::event::publish_input_event_async;
            use ::rmk::input_device::InputDevice;
            use ::rmk::event::SubscribableControllerEvent;
            use ::rmk::controller::Controller;
            use ::rmk::event::EventSubscriber;
            use ::rmk::futures::FutureExt;
            enum __RmkSelectEventReversedSensorController {
                Input(SensorEvent),
                Controller(<Self as ::rmk::controller::Controller>::Event),
            }
            let mut ctrl_sub = <<Self as ::rmk::controller::Controller>::Event as ::rmk::event::SubscribableControllerEvent>::controller_subscriber();
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
                                let mut _1 = ctrl_sub.next_event().fuse();
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
                                    __RmkSelectEventReversedSensorController::Input(event)
                                }
                                __PrivResult::_1(ctrl_event) => {
                                    __RmkSelectEventReversedSensorController::Controller(
                                        ctrl_event,
                                    )
                                }
                            }
                        }
                    }
                };
                match select_result {
                    __RmkSelectEventReversedSensorController::Input(event) => {
                        publish_input_event_async(event).await;
                    }
                    __RmkSelectEventReversedSensorController::Controller(event) => {
                        <Self as ::rmk::controller::Controller>::process_event(
                                self,
                                event,
                            )
                            .await;
                    }
                }
            }
        }
    }
}
mod reversed_polling {
    use super::{ConfigEvent, SensorEvent, controller, input_device};
    pub struct ReversedPollingSensorController {
        pub threshold: u16,
        pub last_value: u16,
    }
    impl ::rmk::input_device::InputDevice for ReversedPollingSensorController {
        type Event = SensorEvent;
        async fn read_event(&mut self) -> Self::Event {
            self.read_sensor_event().await
        }
    }
    impl ::rmk::controller::Controller for ReversedPollingSensorController {
        type Event = ConfigEvent;
        async fn process_event(&mut self, event: Self::Event) {
            self.on_config_event(event).await
        }
    }
    impl ::rmk::controller::PollingController for ReversedPollingSensorController {
        fn interval(&self) -> ::embassy_time::Duration {
            ::embassy_time::Duration::from_millis(50u64)
        }
        async fn update(&mut self) {
            self.poll().await
        }
    }
    impl ::rmk::input_device::Runnable for ReversedPollingSensorController {
        async fn run(&mut self) -> ! {
            use ::rmk::event::publish_input_event_async;
            use ::rmk::input_device::InputDevice;
            use ::rmk::event::SubscribableControllerEvent;
            use ::rmk::controller::Controller;
            use ::rmk::event::EventSubscriber;
            use ::rmk::futures::FutureExt;
            use ::rmk::controller::PollingController;
            enum __RmkSelectEventReversedPollingSensorController {
                Input(SensorEvent),
                Controller(<Self as ::rmk::controller::Controller>::Event),
                Timer,
            }
            let mut ctrl_sub = <<Self as ::rmk::controller::Controller>::Event as ::rmk::event::SubscribableControllerEvent>::controller_subscriber();
            let mut last = ::embassy_time::Instant::now();
            loop {
                let elapsed = last.elapsed();
                let interval = ::embassy_time::Duration::from_millis(50u64);
                let timer = ::embassy_time::Timer::after(
                    interval
                        .checked_sub(elapsed)
                        .unwrap_or(::embassy_time::Duration::MIN),
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
                                let mut _2 = ctrl_sub.next_event().fuse();
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
                                    __RmkSelectEventReversedPollingSensorController::Timer
                                }
                                __PrivResult::_1(event) => {
                                    __RmkSelectEventReversedPollingSensorController::Input(
                                        event,
                                    )
                                }
                                __PrivResult::_2(ctrl_event) => {
                                    __RmkSelectEventReversedPollingSensorController::Controller(
                                        ctrl_event,
                                    )
                                }
                            }
                        }
                    }
                };
                match select_result {
                    __RmkSelectEventReversedPollingSensorController::Input(event) => {
                        publish_input_event_async(event).await;
                    }
                    __RmkSelectEventReversedPollingSensorController::Controller(
                        event,
                    ) => {
                        <Self as ::rmk::controller::Controller>::process_event(
                                self,
                                event,
                            )
                            .await;
                    }
                    __RmkSelectEventReversedPollingSensorController::Timer => {
                        <Self as PollingController>::update(self).await;
                        last = ::embassy_time::Instant::now();
                    }
                }
            }
        }
    }
}
mod multi_event {
    use super::{ConfigEvent, ModeEvent, SensorEvent, controller, input_device};
    pub struct MultiEventSensorController {
        pub threshold: u16,
        pub mode: bool,
    }
    pub enum MultiEventSensorControllerControllerEventEnum {
        Config(ConfigEvent),
        Mode(ModeEvent),
    }
    #[automatically_derived]
    impl ::core::clone::Clone for MultiEventSensorControllerControllerEventEnum {
        #[inline]
        fn clone(&self) -> MultiEventSensorControllerControllerEventEnum {
            match self {
                MultiEventSensorControllerControllerEventEnum::Config(__self_0) => {
                    MultiEventSensorControllerControllerEventEnum::Config(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
                MultiEventSensorControllerControllerEventEnum::Mode(__self_0) => {
                    MultiEventSensorControllerControllerEventEnum::Mode(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
            }
        }
    }
    /// Event subscriber for aggregated events
    pub struct MultiEventSensorControllerControllerEventSubscriber {
        sub0: <ConfigEvent as ::rmk::event::SubscribableControllerEvent>::Subscriber,
        sub1: <ModeEvent as ::rmk::event::SubscribableControllerEvent>::Subscriber,
    }
    impl MultiEventSensorControllerControllerEventSubscriber {
        /// Create a new event subscriber
        pub fn new() -> Self {
            Self {
                sub0: <ConfigEvent as ::rmk::event::SubscribableControllerEvent>::controller_subscriber(),
                sub1: <ModeEvent as ::rmk::event::SubscribableControllerEvent>::controller_subscriber(),
            }
        }
    }
    impl ::rmk::event::EventSubscriber
    for MultiEventSensorControllerControllerEventSubscriber {
        type Event = MultiEventSensorControllerControllerEventEnum;
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
                        let mut __poll_fn = |
                            __cx: &mut __futures_crate::task::Context<'_>|
                        {
                            let mut __any_polled = false;
                            let mut _0 = |__cx: &mut __futures_crate::task::Context<'_>| {
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
                            let mut _1 = |__cx: &mut __futures_crate::task::Context<'_>| {
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
                            MultiEventSensorControllerControllerEventEnum::Config(event)
                        }
                        __PrivResult::_1(event) => {
                            MultiEventSensorControllerControllerEventEnum::Mode(event)
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::event::SubscribableControllerEvent
    for MultiEventSensorControllerControllerEventEnum {
        type Subscriber = MultiEventSensorControllerControllerEventSubscriber;
        fn controller_subscriber() -> Self::Subscriber {
            MultiEventSensorControllerControllerEventSubscriber::new()
        }
    }
    impl ::rmk::controller::Controller for MultiEventSensorController {
        type Event = MultiEventSensorControllerControllerEventEnum;
        async fn process_event(&mut self, event: Self::Event) {
            match event {
                MultiEventSensorControllerControllerEventEnum::Config(event) => {
                    self.on_config_event(event).await
                }
                MultiEventSensorControllerControllerEventEnum::Mode(event) => {
                    self.on_mode_event(event).await
                }
            }
        }
    }
    impl ::rmk::input_device::InputDevice for MultiEventSensorController {
        type Event = SensorEvent;
        async fn read_event(&mut self) -> Self::Event {
            self.read_sensor_event().await
        }
    }
    impl ::rmk::input_device::Runnable for MultiEventSensorController {
        async fn run(&mut self) -> ! {
            use ::rmk::event::publish_input_event_async;
            use ::rmk::input_device::InputDevice;
            use ::rmk::event::SubscribableControllerEvent;
            use ::rmk::controller::Controller;
            use ::rmk::event::EventSubscriber;
            use ::rmk::futures::FutureExt;
            enum __RmkSelectEventMultiEventSensorController {
                Input(SensorEvent),
                Controller(<Self as ::rmk::controller::Controller>::Event),
            }
            let mut ctrl_sub = <<Self as ::rmk::controller::Controller>::Event as ::rmk::event::SubscribableControllerEvent>::controller_subscriber();
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
                                let mut _1 = ctrl_sub.next_event().fuse();
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
                                    __RmkSelectEventMultiEventSensorController::Input(event)
                                }
                                __PrivResult::_1(ctrl_event) => {
                                    __RmkSelectEventMultiEventSensorController::Controller(
                                        ctrl_event,
                                    )
                                }
                            }
                        }
                    }
                };
                match select_result {
                    __RmkSelectEventMultiEventSensorController::Input(event) => {
                        publish_input_event_async(event).await;
                    }
                    __RmkSelectEventMultiEventSensorController::Controller(event) => {
                        <Self as ::rmk::controller::Controller>::process_event(
                                self,
                                event,
                            )
                            .await;
                    }
                }
            }
        }
    }
}
mod multi_event_polling {
    use super::{ConfigEvent, ModeEvent, SensorEvent, controller, input_device};
    pub struct PollingMultiEventSensorController {
        pub threshold: u16,
        pub mode: bool,
        pub last_value: u16,
    }
    pub enum PollingMultiEventSensorControllerControllerEventEnum {
        Config(ConfigEvent),
        Mode(ModeEvent),
    }
    #[automatically_derived]
    impl ::core::clone::Clone for PollingMultiEventSensorControllerControllerEventEnum {
        #[inline]
        fn clone(&self) -> PollingMultiEventSensorControllerControllerEventEnum {
            match self {
                PollingMultiEventSensorControllerControllerEventEnum::Config(
                    __self_0,
                ) => {
                    PollingMultiEventSensorControllerControllerEventEnum::Config(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
                PollingMultiEventSensorControllerControllerEventEnum::Mode(__self_0) => {
                    PollingMultiEventSensorControllerControllerEventEnum::Mode(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
            }
        }
    }
    /// Event subscriber for aggregated events
    pub struct PollingMultiEventSensorControllerControllerEventSubscriber {
        sub0: <ConfigEvent as ::rmk::event::SubscribableControllerEvent>::Subscriber,
        sub1: <ModeEvent as ::rmk::event::SubscribableControllerEvent>::Subscriber,
    }
    impl PollingMultiEventSensorControllerControllerEventSubscriber {
        /// Create a new event subscriber
        pub fn new() -> Self {
            Self {
                sub0: <ConfigEvent as ::rmk::event::SubscribableControllerEvent>::controller_subscriber(),
                sub1: <ModeEvent as ::rmk::event::SubscribableControllerEvent>::controller_subscriber(),
            }
        }
    }
    impl ::rmk::event::EventSubscriber
    for PollingMultiEventSensorControllerControllerEventSubscriber {
        type Event = PollingMultiEventSensorControllerControllerEventEnum;
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
                        let mut __poll_fn = |
                            __cx: &mut __futures_crate::task::Context<'_>|
                        {
                            let mut __any_polled = false;
                            let mut _0 = |__cx: &mut __futures_crate::task::Context<'_>| {
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
                            let mut _1 = |__cx: &mut __futures_crate::task::Context<'_>| {
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
                            PollingMultiEventSensorControllerControllerEventEnum::Config(
                                event,
                            )
                        }
                        __PrivResult::_1(event) => {
                            PollingMultiEventSensorControllerControllerEventEnum::Mode(
                                event,
                            )
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::event::SubscribableControllerEvent
    for PollingMultiEventSensorControllerControllerEventEnum {
        type Subscriber = PollingMultiEventSensorControllerControllerEventSubscriber;
        fn controller_subscriber() -> Self::Subscriber {
            PollingMultiEventSensorControllerControllerEventSubscriber::new()
        }
    }
    impl ::rmk::controller::Controller for PollingMultiEventSensorController {
        type Event = PollingMultiEventSensorControllerControllerEventEnum;
        async fn process_event(&mut self, event: Self::Event) {
            match event {
                PollingMultiEventSensorControllerControllerEventEnum::Config(event) => {
                    self.on_config_event(event).await
                }
                PollingMultiEventSensorControllerControllerEventEnum::Mode(event) => {
                    self.on_mode_event(event).await
                }
            }
        }
    }
    impl ::rmk::controller::PollingController for PollingMultiEventSensorController {
        fn interval(&self) -> ::embassy_time::Duration {
            ::embassy_time::Duration::from_millis(40u64)
        }
        async fn update(&mut self) {
            self.poll().await
        }
    }
    impl ::rmk::input_device::InputDevice for PollingMultiEventSensorController {
        type Event = SensorEvent;
        async fn read_event(&mut self) -> Self::Event {
            self.read_sensor_event().await
        }
    }
    impl ::rmk::input_device::Runnable for PollingMultiEventSensorController {
        async fn run(&mut self) -> ! {
            use ::rmk::event::publish_input_event_async;
            use ::rmk::input_device::InputDevice;
            use ::rmk::event::SubscribableControllerEvent;
            use ::rmk::controller::Controller;
            use ::rmk::event::EventSubscriber;
            use ::rmk::futures::FutureExt;
            use ::rmk::controller::PollingController;
            enum __RmkSelectEventPollingMultiEventSensorController {
                Input(SensorEvent),
                Controller(<Self as ::rmk::controller::Controller>::Event),
                Timer,
            }
            let mut ctrl_sub = <<Self as ::rmk::controller::Controller>::Event as ::rmk::event::SubscribableControllerEvent>::controller_subscriber();
            let mut last = ::embassy_time::Instant::now();
            loop {
                let elapsed = last.elapsed();
                let interval = ::embassy_time::Duration::from_millis(40u64);
                let timer = ::embassy_time::Timer::after(
                    interval
                        .checked_sub(elapsed)
                        .unwrap_or(::embassy_time::Duration::MIN),
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
                                let mut _2 = ctrl_sub.next_event().fuse();
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
                                    __RmkSelectEventPollingMultiEventSensorController::Timer
                                }
                                __PrivResult::_1(event) => {
                                    __RmkSelectEventPollingMultiEventSensorController::Input(
                                        event,
                                    )
                                }
                                __PrivResult::_2(ctrl_event) => {
                                    __RmkSelectEventPollingMultiEventSensorController::Controller(
                                        ctrl_event,
                                    )
                                }
                            }
                        }
                    }
                };
                match select_result {
                    __RmkSelectEventPollingMultiEventSensorController::Input(event) => {
                        publish_input_event_async(event).await;
                    }
                    __RmkSelectEventPollingMultiEventSensorController::Controller(
                        event,
                    ) => {
                        <Self as ::rmk::controller::Controller>::process_event(
                                self,
                                event,
                            )
                            .await;
                    }
                    __RmkSelectEventPollingMultiEventSensorController::Timer => {
                        <Self as PollingController>::update(self).await;
                        last = ::embassy_time::Instant::now();
                    }
                }
            }
        }
    }
}
mod multi_event_reversed {
    use super::{ConfigEvent, ModeEvent, SensorEvent, controller, input_device};
    pub struct ReversedMultiEventSensorController {
        pub threshold: u16,
        pub mode: bool,
    }
    impl ::rmk::input_device::InputDevice for ReversedMultiEventSensorController {
        type Event = SensorEvent;
        async fn read_event(&mut self) -> Self::Event {
            self.read_sensor_event().await
        }
    }
    pub enum ReversedMultiEventSensorControllerControllerEventEnum {
        Config(ConfigEvent),
        Mode(ModeEvent),
    }
    #[automatically_derived]
    impl ::core::clone::Clone for ReversedMultiEventSensorControllerControllerEventEnum {
        #[inline]
        fn clone(&self) -> ReversedMultiEventSensorControllerControllerEventEnum {
            match self {
                ReversedMultiEventSensorControllerControllerEventEnum::Config(
                    __self_0,
                ) => {
                    ReversedMultiEventSensorControllerControllerEventEnum::Config(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
                ReversedMultiEventSensorControllerControllerEventEnum::Mode(__self_0) => {
                    ReversedMultiEventSensorControllerControllerEventEnum::Mode(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
            }
        }
    }
    /// Event subscriber for aggregated events
    pub struct ReversedMultiEventSensorControllerControllerEventSubscriber {
        sub0: <ConfigEvent as ::rmk::event::SubscribableControllerEvent>::Subscriber,
        sub1: <ModeEvent as ::rmk::event::SubscribableControllerEvent>::Subscriber,
    }
    impl ReversedMultiEventSensorControllerControllerEventSubscriber {
        /// Create a new event subscriber
        pub fn new() -> Self {
            Self {
                sub0: <ConfigEvent as ::rmk::event::SubscribableControllerEvent>::controller_subscriber(),
                sub1: <ModeEvent as ::rmk::event::SubscribableControllerEvent>::controller_subscriber(),
            }
        }
    }
    impl ::rmk::event::EventSubscriber
    for ReversedMultiEventSensorControllerControllerEventSubscriber {
        type Event = ReversedMultiEventSensorControllerControllerEventEnum;
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
                        let mut __poll_fn = |
                            __cx: &mut __futures_crate::task::Context<'_>|
                        {
                            let mut __any_polled = false;
                            let mut _0 = |__cx: &mut __futures_crate::task::Context<'_>| {
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
                            let mut _1 = |__cx: &mut __futures_crate::task::Context<'_>| {
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
                            ReversedMultiEventSensorControllerControllerEventEnum::Config(
                                event,
                            )
                        }
                        __PrivResult::_1(event) => {
                            ReversedMultiEventSensorControllerControllerEventEnum::Mode(
                                event,
                            )
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::event::SubscribableControllerEvent
    for ReversedMultiEventSensorControllerControllerEventEnum {
        type Subscriber = ReversedMultiEventSensorControllerControllerEventSubscriber;
        fn controller_subscriber() -> Self::Subscriber {
            ReversedMultiEventSensorControllerControllerEventSubscriber::new()
        }
    }
    impl ::rmk::controller::Controller for ReversedMultiEventSensorController {
        type Event = ReversedMultiEventSensorControllerControllerEventEnum;
        async fn process_event(&mut self, event: Self::Event) {
            match event {
                ReversedMultiEventSensorControllerControllerEventEnum::Config(event) => {
                    self.on_config_event(event).await
                }
                ReversedMultiEventSensorControllerControllerEventEnum::Mode(event) => {
                    self.on_mode_event(event).await
                }
            }
        }
    }
    impl ::rmk::input_device::Runnable for ReversedMultiEventSensorController {
        async fn run(&mut self) -> ! {
            use ::rmk::event::publish_input_event_async;
            use ::rmk::input_device::InputDevice;
            use ::rmk::event::SubscribableControllerEvent;
            use ::rmk::controller::Controller;
            use ::rmk::event::EventSubscriber;
            use ::rmk::futures::FutureExt;
            enum __RmkSelectEventReversedMultiEventSensorController {
                Input(SensorEvent),
                Controller(<Self as ::rmk::controller::Controller>::Event),
            }
            let mut ctrl_sub = <<Self as ::rmk::controller::Controller>::Event as ::rmk::event::SubscribableControllerEvent>::controller_subscriber();
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
                                let mut _1 = ctrl_sub.next_event().fuse();
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
                                    __RmkSelectEventReversedMultiEventSensorController::Input(
                                        event,
                                    )
                                }
                                __PrivResult::_1(ctrl_event) => {
                                    __RmkSelectEventReversedMultiEventSensorController::Controller(
                                        ctrl_event,
                                    )
                                }
                            }
                        }
                    }
                };
                match select_result {
                    __RmkSelectEventReversedMultiEventSensorController::Input(event) => {
                        publish_input_event_async(event).await;
                    }
                    __RmkSelectEventReversedMultiEventSensorController::Controller(
                        event,
                    ) => {
                        <Self as ::rmk::controller::Controller>::process_event(
                                self,
                                event,
                            )
                            .await;
                    }
                }
            }
        }
    }
}
mod multi_event_reversed_polling {
    use super::{ConfigEvent, ModeEvent, SensorEvent, controller, input_device};
    pub struct ReversedPollingMultiEventSensorController {
        pub threshold: u16,
        pub mode: bool,
        pub last_value: u16,
    }
    impl ::rmk::input_device::InputDevice for ReversedPollingMultiEventSensorController {
        type Event = SensorEvent;
        async fn read_event(&mut self) -> Self::Event {
            self.read_sensor_event().await
        }
    }
    pub enum ReversedPollingMultiEventSensorControllerControllerEventEnum {
        Config(ConfigEvent),
        Mode(ModeEvent),
    }
    #[automatically_derived]
    impl ::core::clone::Clone
    for ReversedPollingMultiEventSensorControllerControllerEventEnum {
        #[inline]
        fn clone(&self) -> ReversedPollingMultiEventSensorControllerControllerEventEnum {
            match self {
                ReversedPollingMultiEventSensorControllerControllerEventEnum::Config(
                    __self_0,
                ) => {
                    ReversedPollingMultiEventSensorControllerControllerEventEnum::Config(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
                ReversedPollingMultiEventSensorControllerControllerEventEnum::Mode(
                    __self_0,
                ) => {
                    ReversedPollingMultiEventSensorControllerControllerEventEnum::Mode(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
            }
        }
    }
    /// Event subscriber for aggregated events
    pub struct ReversedPollingMultiEventSensorControllerControllerEventSubscriber {
        sub0: <ConfigEvent as ::rmk::event::SubscribableControllerEvent>::Subscriber,
        sub1: <ModeEvent as ::rmk::event::SubscribableControllerEvent>::Subscriber,
    }
    impl ReversedPollingMultiEventSensorControllerControllerEventSubscriber {
        /// Create a new event subscriber
        pub fn new() -> Self {
            Self {
                sub0: <ConfigEvent as ::rmk::event::SubscribableControllerEvent>::controller_subscriber(),
                sub1: <ModeEvent as ::rmk::event::SubscribableControllerEvent>::controller_subscriber(),
            }
        }
    }
    impl ::rmk::event::EventSubscriber
    for ReversedPollingMultiEventSensorControllerControllerEventSubscriber {
        type Event = ReversedPollingMultiEventSensorControllerControllerEventEnum;
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
                        let mut __poll_fn = |
                            __cx: &mut __futures_crate::task::Context<'_>|
                        {
                            let mut __any_polled = false;
                            let mut _0 = |__cx: &mut __futures_crate::task::Context<'_>| {
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
                            let mut _1 = |__cx: &mut __futures_crate::task::Context<'_>| {
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
                            ReversedPollingMultiEventSensorControllerControllerEventEnum::Config(
                                event,
                            )
                        }
                        __PrivResult::_1(event) => {
                            ReversedPollingMultiEventSensorControllerControllerEventEnum::Mode(
                                event,
                            )
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::event::SubscribableControllerEvent
    for ReversedPollingMultiEventSensorControllerControllerEventEnum {
        type Subscriber = ReversedPollingMultiEventSensorControllerControllerEventSubscriber;
        fn controller_subscriber() -> Self::Subscriber {
            ReversedPollingMultiEventSensorControllerControllerEventSubscriber::new()
        }
    }
    impl ::rmk::controller::Controller for ReversedPollingMultiEventSensorController {
        type Event = ReversedPollingMultiEventSensorControllerControllerEventEnum;
        async fn process_event(&mut self, event: Self::Event) {
            match event {
                ReversedPollingMultiEventSensorControllerControllerEventEnum::Config(
                    event,
                ) => self.on_config_event(event).await,
                ReversedPollingMultiEventSensorControllerControllerEventEnum::Mode(
                    event,
                ) => self.on_mode_event(event).await,
            }
        }
    }
    impl ::rmk::controller::PollingController
    for ReversedPollingMultiEventSensorController {
        fn interval(&self) -> ::embassy_time::Duration {
            ::embassy_time::Duration::from_millis(40u64)
        }
        async fn update(&mut self) {
            self.poll().await
        }
    }
    impl ::rmk::input_device::Runnable for ReversedPollingMultiEventSensorController {
        async fn run(&mut self) -> ! {
            use ::rmk::event::publish_input_event_async;
            use ::rmk::input_device::InputDevice;
            use ::rmk::event::SubscribableControllerEvent;
            use ::rmk::controller::Controller;
            use ::rmk::event::EventSubscriber;
            use ::rmk::futures::FutureExt;
            use ::rmk::controller::PollingController;
            enum __RmkSelectEventReversedPollingMultiEventSensorController {
                Input(SensorEvent),
                Controller(<Self as ::rmk::controller::Controller>::Event),
                Timer,
            }
            let mut ctrl_sub = <<Self as ::rmk::controller::Controller>::Event as ::rmk::event::SubscribableControllerEvent>::controller_subscriber();
            let mut last = ::embassy_time::Instant::now();
            loop {
                let elapsed = last.elapsed();
                let interval = ::embassy_time::Duration::from_millis(40u64);
                let timer = ::embassy_time::Timer::after(
                    interval
                        .checked_sub(elapsed)
                        .unwrap_or(::embassy_time::Duration::MIN),
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
                                let mut _2 = ctrl_sub.next_event().fuse();
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
                                    __RmkSelectEventReversedPollingMultiEventSensorController::Timer
                                }
                                __PrivResult::_1(event) => {
                                    __RmkSelectEventReversedPollingMultiEventSensorController::Input(
                                        event,
                                    )
                                }
                                __PrivResult::_2(ctrl_event) => {
                                    __RmkSelectEventReversedPollingMultiEventSensorController::Controller(
                                        ctrl_event,
                                    )
                                }
                            }
                        }
                    }
                };
                match select_result {
                    __RmkSelectEventReversedPollingMultiEventSensorController::Input(
                        event,
                    ) => {
                        publish_input_event_async(event).await;
                    }
                    __RmkSelectEventReversedPollingMultiEventSensorController::Controller(
                        event,
                    ) => {
                        <Self as ::rmk::controller::Controller>::process_event(
                                self,
                                event,
                            )
                            .await;
                    }
                    __RmkSelectEventReversedPollingMultiEventSensorController::Timer => {
                        <Self as PollingController>::update(self).await;
                        last = ::embassy_time::Instant::now();
                    }
                }
            }
        }
    }
}
