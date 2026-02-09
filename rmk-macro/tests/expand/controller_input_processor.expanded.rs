use rmk_macro::{controller, input_processor};
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
    use super::{ConfigEvent, EncoderEvent, KeyEvent, controller, input_processor};
    pub struct HybridProcessorController;
    impl ::rmk::controller::Controller for HybridProcessorController {
        type Event = ConfigEvent;
        async fn process_event(&mut self, event: Self::Event) {
            self.on_config_event(event).await
        }
    }
    pub enum HybridProcessorControllerInputEventEnum {
        Key(KeyEvent),
        Encoder(EncoderEvent),
    }
    #[automatically_derived]
    impl ::core::clone::Clone for HybridProcessorControllerInputEventEnum {
        #[inline]
        fn clone(&self) -> HybridProcessorControllerInputEventEnum {
            match self {
                HybridProcessorControllerInputEventEnum::Key(__self_0) => {
                    HybridProcessorControllerInputEventEnum::Key(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
                HybridProcessorControllerInputEventEnum::Encoder(__self_0) => {
                    HybridProcessorControllerInputEventEnum::Encoder(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
            }
        }
    }
    /// Event subscriber for aggregated events
    pub struct HybridProcessorControllerInputEventSubscriber {
        sub0: <KeyEvent as ::rmk::event::SubscribableInputEvent>::Subscriber,
        sub1: <EncoderEvent as ::rmk::event::SubscribableInputEvent>::Subscriber,
    }
    impl HybridProcessorControllerInputEventSubscriber {
        /// Create a new event subscriber
        pub fn new() -> Self {
            Self {
                sub0: <KeyEvent as ::rmk::event::SubscribableInputEvent>::input_subscriber(),
                sub1: <EncoderEvent as ::rmk::event::SubscribableInputEvent>::input_subscriber(),
            }
        }
    }
    impl ::rmk::event::EventSubscriber
    for HybridProcessorControllerInputEventSubscriber {
        type Event = HybridProcessorControllerInputEventEnum;
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
                            HybridProcessorControllerInputEventEnum::Key(event)
                        }
                        __PrivResult::_1(event) => {
                            HybridProcessorControllerInputEventEnum::Encoder(event)
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::event::SubscribableInputEvent
    for HybridProcessorControllerInputEventEnum {
        type Subscriber = HybridProcessorControllerInputEventSubscriber;
        fn input_subscriber() -> Self::Subscriber {
            HybridProcessorControllerInputEventSubscriber::new()
        }
    }
    impl ::rmk::input_device::Runnable for HybridProcessorController {
        async fn run(&mut self) -> ! {
            use ::rmk::event::SubscribableInputEvent;
            use ::rmk::input_device::InputProcessor;
            use ::rmk::event::SubscribableControllerEvent;
            use ::rmk::controller::Controller;
            use ::rmk::event::EventSubscriber;
            use ::rmk::futures::FutureExt;
            let mut proc_sub = <<Self as ::rmk::input_device::InputProcessor>::Event as ::rmk::event::SubscribableInputEvent>::input_subscriber();
            let mut ctrl_sub = <<Self as ::rmk::controller::Controller>::Event as ::rmk::event::SubscribableControllerEvent>::controller_subscriber();
            loop {
                {
                    use ::futures_util::__private as __futures_crate;
                    {
                        enum __PrivResult<_0, _1> {
                            _0(_0),
                            _1(_1),
                        }
                        let __select_result = {
                            let mut _0 = proc_sub.next_event().fuse();
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
                            __PrivResult::_0(proc_event) => {
                                self.process(proc_event).await;
                            }
                            __PrivResult::_1(ctrl_event) => {
                                <Self as ::rmk::controller::Controller>::process_event(
                                        self,
                                        ctrl_event,
                                    )
                                    .await;
                            }
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::input_device::InputProcessor for HybridProcessorController {
        type Event = HybridProcessorControllerInputEventEnum;
        async fn process(&mut self, event: Self::Event) {
            match event {
                HybridProcessorControllerInputEventEnum::Key(event) => {
                    self.on_key_event(event).await
                }
                HybridProcessorControllerInputEventEnum::Encoder(event) => {
                    self.on_encoder_event(event).await
                }
            }
        }
    }
}
mod polling {
    use super::{ConfigEvent, EncoderEvent, KeyEvent, controller, input_processor};
    pub struct PollingHybridProcessorController;
    impl ::rmk::controller::Controller for PollingHybridProcessorController {
        type Event = ConfigEvent;
        async fn process_event(&mut self, event: Self::Event) {
            self.on_config_event(event).await
        }
    }
    impl ::rmk::controller::PollingController for PollingHybridProcessorController {
        fn interval(&self) -> ::embassy_time::Duration {
            ::embassy_time::Duration::from_millis(20u64)
        }
        async fn update(&mut self) {
            self.poll().await
        }
    }
    pub enum PollingHybridProcessorControllerInputEventEnum {
        Key(KeyEvent),
        Encoder(EncoderEvent),
    }
    #[automatically_derived]
    impl ::core::clone::Clone for PollingHybridProcessorControllerInputEventEnum {
        #[inline]
        fn clone(&self) -> PollingHybridProcessorControllerInputEventEnum {
            match self {
                PollingHybridProcessorControllerInputEventEnum::Key(__self_0) => {
                    PollingHybridProcessorControllerInputEventEnum::Key(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
                PollingHybridProcessorControllerInputEventEnum::Encoder(__self_0) => {
                    PollingHybridProcessorControllerInputEventEnum::Encoder(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
            }
        }
    }
    /// Event subscriber for aggregated events
    pub struct PollingHybridProcessorControllerInputEventSubscriber {
        sub0: <KeyEvent as ::rmk::event::SubscribableInputEvent>::Subscriber,
        sub1: <EncoderEvent as ::rmk::event::SubscribableInputEvent>::Subscriber,
    }
    impl PollingHybridProcessorControllerInputEventSubscriber {
        /// Create a new event subscriber
        pub fn new() -> Self {
            Self {
                sub0: <KeyEvent as ::rmk::event::SubscribableInputEvent>::input_subscriber(),
                sub1: <EncoderEvent as ::rmk::event::SubscribableInputEvent>::input_subscriber(),
            }
        }
    }
    impl ::rmk::event::EventSubscriber
    for PollingHybridProcessorControllerInputEventSubscriber {
        type Event = PollingHybridProcessorControllerInputEventEnum;
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
                            PollingHybridProcessorControllerInputEventEnum::Key(event)
                        }
                        __PrivResult::_1(event) => {
                            PollingHybridProcessorControllerInputEventEnum::Encoder(
                                event,
                            )
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::event::SubscribableInputEvent
    for PollingHybridProcessorControllerInputEventEnum {
        type Subscriber = PollingHybridProcessorControllerInputEventSubscriber;
        fn input_subscriber() -> Self::Subscriber {
            PollingHybridProcessorControllerInputEventSubscriber::new()
        }
    }
    impl ::rmk::input_device::Runnable for PollingHybridProcessorController {
        async fn run(&mut self) -> ! {
            use ::rmk::event::SubscribableInputEvent;
            use ::rmk::input_device::InputProcessor;
            use ::rmk::event::SubscribableControllerEvent;
            use ::rmk::controller::Controller;
            use ::rmk::event::EventSubscriber;
            use ::rmk::futures::FutureExt;
            use ::rmk::controller::PollingController;
            let mut proc_sub = <<Self as ::rmk::input_device::InputProcessor>::Event as ::rmk::event::SubscribableInputEvent>::input_subscriber();
            let mut ctrl_sub = <<Self as ::rmk::controller::Controller>::Event as ::rmk::event::SubscribableControllerEvent>::controller_subscriber();
            let mut last = ::embassy_time::Instant::now();
            loop {
                let elapsed = last.elapsed();
                let interval = ::embassy_time::Duration::from_millis(20u64);
                let timer = ::embassy_time::Timer::after(
                    interval
                        .checked_sub(elapsed)
                        .unwrap_or(::embassy_time::Duration::MIN),
                );
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
                            let mut _1 = proc_sub.next_event().fuse();
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
                                <Self as PollingController>::update(self).await;
                                last = ::embassy_time::Instant::now();
                            }
                            __PrivResult::_1(proc_event) => {
                                self.process(proc_event).await;
                            }
                            __PrivResult::_2(ctrl_event) => {
                                <Self as ::rmk::controller::Controller>::process_event(
                                        self,
                                        ctrl_event,
                                    )
                                    .await;
                            }
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::input_device::InputProcessor for PollingHybridProcessorController {
        type Event = PollingHybridProcessorControllerInputEventEnum;
        async fn process(&mut self, event: Self::Event) {
            match event {
                PollingHybridProcessorControllerInputEventEnum::Key(event) => {
                    self.on_key_event(event).await
                }
                PollingHybridProcessorControllerInputEventEnum::Encoder(event) => {
                    self.on_encoder_event(event).await
                }
            }
        }
    }
}
mod reversed {
    use super::{ConfigEvent, EncoderEvent, KeyEvent, controller, input_processor};
    pub struct ReversedHybridProcessorController;
    pub enum ReversedHybridProcessorControllerInputEventEnum {
        Key(KeyEvent),
        Encoder(EncoderEvent),
    }
    #[automatically_derived]
    impl ::core::clone::Clone for ReversedHybridProcessorControllerInputEventEnum {
        #[inline]
        fn clone(&self) -> ReversedHybridProcessorControllerInputEventEnum {
            match self {
                ReversedHybridProcessorControllerInputEventEnum::Key(__self_0) => {
                    ReversedHybridProcessorControllerInputEventEnum::Key(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
                ReversedHybridProcessorControllerInputEventEnum::Encoder(__self_0) => {
                    ReversedHybridProcessorControllerInputEventEnum::Encoder(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
            }
        }
    }
    /// Event subscriber for aggregated events
    pub struct ReversedHybridProcessorControllerInputEventSubscriber {
        sub0: <KeyEvent as ::rmk::event::SubscribableInputEvent>::Subscriber,
        sub1: <EncoderEvent as ::rmk::event::SubscribableInputEvent>::Subscriber,
    }
    impl ReversedHybridProcessorControllerInputEventSubscriber {
        /// Create a new event subscriber
        pub fn new() -> Self {
            Self {
                sub0: <KeyEvent as ::rmk::event::SubscribableInputEvent>::input_subscriber(),
                sub1: <EncoderEvent as ::rmk::event::SubscribableInputEvent>::input_subscriber(),
            }
        }
    }
    impl ::rmk::event::EventSubscriber
    for ReversedHybridProcessorControllerInputEventSubscriber {
        type Event = ReversedHybridProcessorControllerInputEventEnum;
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
                            ReversedHybridProcessorControllerInputEventEnum::Key(event)
                        }
                        __PrivResult::_1(event) => {
                            ReversedHybridProcessorControllerInputEventEnum::Encoder(
                                event,
                            )
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::event::SubscribableInputEvent
    for ReversedHybridProcessorControllerInputEventEnum {
        type Subscriber = ReversedHybridProcessorControllerInputEventSubscriber;
        fn input_subscriber() -> Self::Subscriber {
            ReversedHybridProcessorControllerInputEventSubscriber::new()
        }
    }
    impl ::rmk::input_device::InputProcessor for ReversedHybridProcessorController {
        type Event = ReversedHybridProcessorControllerInputEventEnum;
        async fn process(&mut self, event: Self::Event) {
            match event {
                ReversedHybridProcessorControllerInputEventEnum::Key(event) => {
                    self.on_key_event(event).await
                }
                ReversedHybridProcessorControllerInputEventEnum::Encoder(event) => {
                    self.on_encoder_event(event).await
                }
            }
        }
    }
    impl ::rmk::controller::Controller for ReversedHybridProcessorController {
        type Event = ConfigEvent;
        async fn process_event(&mut self, event: Self::Event) {
            self.on_config_event(event).await
        }
    }
    impl ::rmk::input_device::Runnable for ReversedHybridProcessorController {
        async fn run(&mut self) -> ! {
            use ::rmk::event::SubscribableInputEvent;
            use ::rmk::input_device::InputProcessor;
            use ::rmk::event::SubscribableControllerEvent;
            use ::rmk::controller::Controller;
            use ::rmk::event::EventSubscriber;
            use ::rmk::futures::FutureExt;
            let mut proc_sub = <<Self as ::rmk::input_device::InputProcessor>::Event as ::rmk::event::SubscribableInputEvent>::input_subscriber();
            let mut ctrl_sub = <<Self as ::rmk::controller::Controller>::Event as ::rmk::event::SubscribableControllerEvent>::controller_subscriber();
            loop {
                {
                    use ::futures_util::__private as __futures_crate;
                    {
                        enum __PrivResult<_0, _1> {
                            _0(_0),
                            _1(_1),
                        }
                        let __select_result = {
                            let mut _0 = proc_sub.next_event().fuse();
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
                            __PrivResult::_0(proc_event) => {
                                self.process(proc_event).await;
                            }
                            __PrivResult::_1(ctrl_event) => {
                                <Self as ::rmk::controller::Controller>::process_event(
                                        self,
                                        ctrl_event,
                                    )
                                    .await;
                            }
                        }
                    }
                }
            }
        }
    }
}
mod reversed_polling {
    use super::{ConfigEvent, EncoderEvent, KeyEvent, controller, input_processor};
    pub struct ReversedPollingHybridProcessorController;
    pub enum ReversedPollingHybridProcessorControllerInputEventEnum {
        Key(KeyEvent),
        Encoder(EncoderEvent),
    }
    #[automatically_derived]
    impl ::core::clone::Clone
    for ReversedPollingHybridProcessorControllerInputEventEnum {
        #[inline]
        fn clone(&self) -> ReversedPollingHybridProcessorControllerInputEventEnum {
            match self {
                ReversedPollingHybridProcessorControllerInputEventEnum::Key(__self_0) => {
                    ReversedPollingHybridProcessorControllerInputEventEnum::Key(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
                ReversedPollingHybridProcessorControllerInputEventEnum::Encoder(
                    __self_0,
                ) => {
                    ReversedPollingHybridProcessorControllerInputEventEnum::Encoder(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
            }
        }
    }
    /// Event subscriber for aggregated events
    pub struct ReversedPollingHybridProcessorControllerInputEventSubscriber {
        sub0: <KeyEvent as ::rmk::event::SubscribableInputEvent>::Subscriber,
        sub1: <EncoderEvent as ::rmk::event::SubscribableInputEvent>::Subscriber,
    }
    impl ReversedPollingHybridProcessorControllerInputEventSubscriber {
        /// Create a new event subscriber
        pub fn new() -> Self {
            Self {
                sub0: <KeyEvent as ::rmk::event::SubscribableInputEvent>::input_subscriber(),
                sub1: <EncoderEvent as ::rmk::event::SubscribableInputEvent>::input_subscriber(),
            }
        }
    }
    impl ::rmk::event::EventSubscriber
    for ReversedPollingHybridProcessorControllerInputEventSubscriber {
        type Event = ReversedPollingHybridProcessorControllerInputEventEnum;
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
                            ReversedPollingHybridProcessorControllerInputEventEnum::Key(
                                event,
                            )
                        }
                        __PrivResult::_1(event) => {
                            ReversedPollingHybridProcessorControllerInputEventEnum::Encoder(
                                event,
                            )
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::event::SubscribableInputEvent
    for ReversedPollingHybridProcessorControllerInputEventEnum {
        type Subscriber = ReversedPollingHybridProcessorControllerInputEventSubscriber;
        fn input_subscriber() -> Self::Subscriber {
            ReversedPollingHybridProcessorControllerInputEventSubscriber::new()
        }
    }
    impl ::rmk::input_device::InputProcessor
    for ReversedPollingHybridProcessorController {
        type Event = ReversedPollingHybridProcessorControllerInputEventEnum;
        async fn process(&mut self, event: Self::Event) {
            match event {
                ReversedPollingHybridProcessorControllerInputEventEnum::Key(event) => {
                    self.on_key_event(event).await
                }
                ReversedPollingHybridProcessorControllerInputEventEnum::Encoder(
                    event,
                ) => self.on_encoder_event(event).await,
            }
        }
    }
    impl ::rmk::controller::Controller for ReversedPollingHybridProcessorController {
        type Event = ConfigEvent;
        async fn process_event(&mut self, event: Self::Event) {
            self.on_config_event(event).await
        }
    }
    impl ::rmk::controller::PollingController
    for ReversedPollingHybridProcessorController {
        fn interval(&self) -> ::embassy_time::Duration {
            ::embassy_time::Duration::from_millis(20u64)
        }
        async fn update(&mut self) {
            self.poll().await
        }
    }
    impl ::rmk::input_device::Runnable for ReversedPollingHybridProcessorController {
        async fn run(&mut self) -> ! {
            use ::rmk::event::SubscribableInputEvent;
            use ::rmk::input_device::InputProcessor;
            use ::rmk::event::SubscribableControllerEvent;
            use ::rmk::controller::Controller;
            use ::rmk::event::EventSubscriber;
            use ::rmk::futures::FutureExt;
            use ::rmk::controller::PollingController;
            let mut proc_sub = <<Self as ::rmk::input_device::InputProcessor>::Event as ::rmk::event::SubscribableInputEvent>::input_subscriber();
            let mut ctrl_sub = <<Self as ::rmk::controller::Controller>::Event as ::rmk::event::SubscribableControllerEvent>::controller_subscriber();
            let mut last = ::embassy_time::Instant::now();
            loop {
                let elapsed = last.elapsed();
                let interval = ::embassy_time::Duration::from_millis(20u64);
                let timer = ::embassy_time::Timer::after(
                    interval
                        .checked_sub(elapsed)
                        .unwrap_or(::embassy_time::Duration::MIN),
                );
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
                            let mut _1 = proc_sub.next_event().fuse();
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
                                <Self as PollingController>::update(self).await;
                                last = ::embassy_time::Instant::now();
                            }
                            __PrivResult::_1(proc_event) => {
                                self.process(proc_event).await;
                            }
                            __PrivResult::_2(ctrl_event) => {
                                <Self as ::rmk::controller::Controller>::process_event(
                                        self,
                                        ctrl_event,
                                    )
                                    .await;
                            }
                        }
                    }
                }
            }
        }
    }
}
mod multi_event {
    use super::{
        ConfigEvent, EncoderEvent, KeyEvent, ModeEvent, controller, input_processor,
    };
    pub struct MultiControllerHybridProcessor;
    pub enum MultiControllerHybridProcessorControllerEventEnum {
        Config(ConfigEvent),
        Mode(ModeEvent),
    }
    #[automatically_derived]
    impl ::core::clone::Clone for MultiControllerHybridProcessorControllerEventEnum {
        #[inline]
        fn clone(&self) -> MultiControllerHybridProcessorControllerEventEnum {
            match self {
                MultiControllerHybridProcessorControllerEventEnum::Config(__self_0) => {
                    MultiControllerHybridProcessorControllerEventEnum::Config(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
                MultiControllerHybridProcessorControllerEventEnum::Mode(__self_0) => {
                    MultiControllerHybridProcessorControllerEventEnum::Mode(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
            }
        }
    }
    /// Event subscriber for aggregated events
    pub struct MultiControllerHybridProcessorControllerEventSubscriber {
        sub0: <ConfigEvent as ::rmk::event::SubscribableControllerEvent>::Subscriber,
        sub1: <ModeEvent as ::rmk::event::SubscribableControllerEvent>::Subscriber,
    }
    impl MultiControllerHybridProcessorControllerEventSubscriber {
        /// Create a new event subscriber
        pub fn new() -> Self {
            Self {
                sub0: <ConfigEvent as ::rmk::event::SubscribableControllerEvent>::controller_subscriber(),
                sub1: <ModeEvent as ::rmk::event::SubscribableControllerEvent>::controller_subscriber(),
            }
        }
    }
    impl ::rmk::event::EventSubscriber
    for MultiControllerHybridProcessorControllerEventSubscriber {
        type Event = MultiControllerHybridProcessorControllerEventEnum;
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
                            MultiControllerHybridProcessorControllerEventEnum::Config(
                                event,
                            )
                        }
                        __PrivResult::_1(event) => {
                            MultiControllerHybridProcessorControllerEventEnum::Mode(
                                event,
                            )
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::event::SubscribableControllerEvent
    for MultiControllerHybridProcessorControllerEventEnum {
        type Subscriber = MultiControllerHybridProcessorControllerEventSubscriber;
        fn controller_subscriber() -> Self::Subscriber {
            MultiControllerHybridProcessorControllerEventSubscriber::new()
        }
    }
    impl ::rmk::controller::Controller for MultiControllerHybridProcessor {
        type Event = MultiControllerHybridProcessorControllerEventEnum;
        async fn process_event(&mut self, event: Self::Event) {
            match event {
                MultiControllerHybridProcessorControllerEventEnum::Config(event) => {
                    self.on_config_event(event).await
                }
                MultiControllerHybridProcessorControllerEventEnum::Mode(event) => {
                    self.on_mode_event(event).await
                }
            }
        }
    }
    pub enum MultiControllerHybridProcessorInputEventEnum {
        Key(KeyEvent),
        Encoder(EncoderEvent),
    }
    #[automatically_derived]
    impl ::core::clone::Clone for MultiControllerHybridProcessorInputEventEnum {
        #[inline]
        fn clone(&self) -> MultiControllerHybridProcessorInputEventEnum {
            match self {
                MultiControllerHybridProcessorInputEventEnum::Key(__self_0) => {
                    MultiControllerHybridProcessorInputEventEnum::Key(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
                MultiControllerHybridProcessorInputEventEnum::Encoder(__self_0) => {
                    MultiControllerHybridProcessorInputEventEnum::Encoder(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
            }
        }
    }
    /// Event subscriber for aggregated events
    pub struct MultiControllerHybridProcessorInputEventSubscriber {
        sub0: <KeyEvent as ::rmk::event::SubscribableInputEvent>::Subscriber,
        sub1: <EncoderEvent as ::rmk::event::SubscribableInputEvent>::Subscriber,
    }
    impl MultiControllerHybridProcessorInputEventSubscriber {
        /// Create a new event subscriber
        pub fn new() -> Self {
            Self {
                sub0: <KeyEvent as ::rmk::event::SubscribableInputEvent>::input_subscriber(),
                sub1: <EncoderEvent as ::rmk::event::SubscribableInputEvent>::input_subscriber(),
            }
        }
    }
    impl ::rmk::event::EventSubscriber
    for MultiControllerHybridProcessorInputEventSubscriber {
        type Event = MultiControllerHybridProcessorInputEventEnum;
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
                            MultiControllerHybridProcessorInputEventEnum::Key(event)
                        }
                        __PrivResult::_1(event) => {
                            MultiControllerHybridProcessorInputEventEnum::Encoder(event)
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::event::SubscribableInputEvent
    for MultiControllerHybridProcessorInputEventEnum {
        type Subscriber = MultiControllerHybridProcessorInputEventSubscriber;
        fn input_subscriber() -> Self::Subscriber {
            MultiControllerHybridProcessorInputEventSubscriber::new()
        }
    }
    impl ::rmk::input_device::Runnable for MultiControllerHybridProcessor {
        async fn run(&mut self) -> ! {
            use ::rmk::event::SubscribableInputEvent;
            use ::rmk::input_device::InputProcessor;
            use ::rmk::event::SubscribableControllerEvent;
            use ::rmk::controller::Controller;
            use ::rmk::event::EventSubscriber;
            use ::rmk::futures::FutureExt;
            let mut proc_sub = <<Self as ::rmk::input_device::InputProcessor>::Event as ::rmk::event::SubscribableInputEvent>::input_subscriber();
            let mut ctrl_sub = <<Self as ::rmk::controller::Controller>::Event as ::rmk::event::SubscribableControllerEvent>::controller_subscriber();
            loop {
                {
                    use ::futures_util::__private as __futures_crate;
                    {
                        enum __PrivResult<_0, _1> {
                            _0(_0),
                            _1(_1),
                        }
                        let __select_result = {
                            let mut _0 = proc_sub.next_event().fuse();
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
                            __PrivResult::_0(proc_event) => {
                                self.process(proc_event).await;
                            }
                            __PrivResult::_1(ctrl_event) => {
                                <Self as ::rmk::controller::Controller>::process_event(
                                        self,
                                        ctrl_event,
                                    )
                                    .await;
                            }
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::input_device::InputProcessor for MultiControllerHybridProcessor {
        type Event = MultiControllerHybridProcessorInputEventEnum;
        async fn process(&mut self, event: Self::Event) {
            match event {
                MultiControllerHybridProcessorInputEventEnum::Key(event) => {
                    self.on_key_event(event).await
                }
                MultiControllerHybridProcessorInputEventEnum::Encoder(event) => {
                    self.on_encoder_event(event).await
                }
            }
        }
    }
}
mod multi_event_polling {
    use super::{
        ConfigEvent, EncoderEvent, KeyEvent, ModeEvent, controller, input_processor,
    };
    pub struct PollingMultiControllerHybridProcessor;
    pub enum PollingMultiControllerHybridProcessorControllerEventEnum {
        Config(ConfigEvent),
        Mode(ModeEvent),
    }
    #[automatically_derived]
    impl ::core::clone::Clone
    for PollingMultiControllerHybridProcessorControllerEventEnum {
        #[inline]
        fn clone(&self) -> PollingMultiControllerHybridProcessorControllerEventEnum {
            match self {
                PollingMultiControllerHybridProcessorControllerEventEnum::Config(
                    __self_0,
                ) => {
                    PollingMultiControllerHybridProcessorControllerEventEnum::Config(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
                PollingMultiControllerHybridProcessorControllerEventEnum::Mode(
                    __self_0,
                ) => {
                    PollingMultiControllerHybridProcessorControllerEventEnum::Mode(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
            }
        }
    }
    /// Event subscriber for aggregated events
    pub struct PollingMultiControllerHybridProcessorControllerEventSubscriber {
        sub0: <ConfigEvent as ::rmk::event::SubscribableControllerEvent>::Subscriber,
        sub1: <ModeEvent as ::rmk::event::SubscribableControllerEvent>::Subscriber,
    }
    impl PollingMultiControllerHybridProcessorControllerEventSubscriber {
        /// Create a new event subscriber
        pub fn new() -> Self {
            Self {
                sub0: <ConfigEvent as ::rmk::event::SubscribableControllerEvent>::controller_subscriber(),
                sub1: <ModeEvent as ::rmk::event::SubscribableControllerEvent>::controller_subscriber(),
            }
        }
    }
    impl ::rmk::event::EventSubscriber
    for PollingMultiControllerHybridProcessorControllerEventSubscriber {
        type Event = PollingMultiControllerHybridProcessorControllerEventEnum;
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
                            PollingMultiControllerHybridProcessorControllerEventEnum::Config(
                                event,
                            )
                        }
                        __PrivResult::_1(event) => {
                            PollingMultiControllerHybridProcessorControllerEventEnum::Mode(
                                event,
                            )
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::event::SubscribableControllerEvent
    for PollingMultiControllerHybridProcessorControllerEventEnum {
        type Subscriber = PollingMultiControllerHybridProcessorControllerEventSubscriber;
        fn controller_subscriber() -> Self::Subscriber {
            PollingMultiControllerHybridProcessorControllerEventSubscriber::new()
        }
    }
    impl ::rmk::controller::Controller for PollingMultiControllerHybridProcessor {
        type Event = PollingMultiControllerHybridProcessorControllerEventEnum;
        async fn process_event(&mut self, event: Self::Event) {
            match event {
                PollingMultiControllerHybridProcessorControllerEventEnum::Config(
                    event,
                ) => self.on_config_event(event).await,
                PollingMultiControllerHybridProcessorControllerEventEnum::Mode(event) => {
                    self.on_mode_event(event).await
                }
            }
        }
    }
    impl ::rmk::controller::PollingController for PollingMultiControllerHybridProcessor {
        fn interval(&self) -> ::embassy_time::Duration {
            ::embassy_time::Duration::from_millis(20u64)
        }
        async fn update(&mut self) {
            self.poll().await
        }
    }
    pub enum PollingMultiControllerHybridProcessorInputEventEnum {
        Key(KeyEvent),
        Encoder(EncoderEvent),
    }
    #[automatically_derived]
    impl ::core::clone::Clone for PollingMultiControllerHybridProcessorInputEventEnum {
        #[inline]
        fn clone(&self) -> PollingMultiControllerHybridProcessorInputEventEnum {
            match self {
                PollingMultiControllerHybridProcessorInputEventEnum::Key(__self_0) => {
                    PollingMultiControllerHybridProcessorInputEventEnum::Key(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
                PollingMultiControllerHybridProcessorInputEventEnum::Encoder(
                    __self_0,
                ) => {
                    PollingMultiControllerHybridProcessorInputEventEnum::Encoder(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
            }
        }
    }
    /// Event subscriber for aggregated events
    pub struct PollingMultiControllerHybridProcessorInputEventSubscriber {
        sub0: <KeyEvent as ::rmk::event::SubscribableInputEvent>::Subscriber,
        sub1: <EncoderEvent as ::rmk::event::SubscribableInputEvent>::Subscriber,
    }
    impl PollingMultiControllerHybridProcessorInputEventSubscriber {
        /// Create a new event subscriber
        pub fn new() -> Self {
            Self {
                sub0: <KeyEvent as ::rmk::event::SubscribableInputEvent>::input_subscriber(),
                sub1: <EncoderEvent as ::rmk::event::SubscribableInputEvent>::input_subscriber(),
            }
        }
    }
    impl ::rmk::event::EventSubscriber
    for PollingMultiControllerHybridProcessorInputEventSubscriber {
        type Event = PollingMultiControllerHybridProcessorInputEventEnum;
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
                            PollingMultiControllerHybridProcessorInputEventEnum::Key(
                                event,
                            )
                        }
                        __PrivResult::_1(event) => {
                            PollingMultiControllerHybridProcessorInputEventEnum::Encoder(
                                event,
                            )
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::event::SubscribableInputEvent
    for PollingMultiControllerHybridProcessorInputEventEnum {
        type Subscriber = PollingMultiControllerHybridProcessorInputEventSubscriber;
        fn input_subscriber() -> Self::Subscriber {
            PollingMultiControllerHybridProcessorInputEventSubscriber::new()
        }
    }
    impl ::rmk::input_device::Runnable for PollingMultiControllerHybridProcessor {
        async fn run(&mut self) -> ! {
            use ::rmk::event::SubscribableInputEvent;
            use ::rmk::input_device::InputProcessor;
            use ::rmk::event::SubscribableControllerEvent;
            use ::rmk::controller::Controller;
            use ::rmk::event::EventSubscriber;
            use ::rmk::futures::FutureExt;
            use ::rmk::controller::PollingController;
            let mut proc_sub = <<Self as ::rmk::input_device::InputProcessor>::Event as ::rmk::event::SubscribableInputEvent>::input_subscriber();
            let mut ctrl_sub = <<Self as ::rmk::controller::Controller>::Event as ::rmk::event::SubscribableControllerEvent>::controller_subscriber();
            let mut last = ::embassy_time::Instant::now();
            loop {
                let elapsed = last.elapsed();
                let interval = ::embassy_time::Duration::from_millis(20u64);
                let timer = ::embassy_time::Timer::after(
                    interval
                        .checked_sub(elapsed)
                        .unwrap_or(::embassy_time::Duration::MIN),
                );
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
                            let mut _1 = proc_sub.next_event().fuse();
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
                                <Self as PollingController>::update(self).await;
                                last = ::embassy_time::Instant::now();
                            }
                            __PrivResult::_1(proc_event) => {
                                self.process(proc_event).await;
                            }
                            __PrivResult::_2(ctrl_event) => {
                                <Self as ::rmk::controller::Controller>::process_event(
                                        self,
                                        ctrl_event,
                                    )
                                    .await;
                            }
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::input_device::InputProcessor for PollingMultiControllerHybridProcessor {
        type Event = PollingMultiControllerHybridProcessorInputEventEnum;
        async fn process(&mut self, event: Self::Event) {
            match event {
                PollingMultiControllerHybridProcessorInputEventEnum::Key(event) => {
                    self.on_key_event(event).await
                }
                PollingMultiControllerHybridProcessorInputEventEnum::Encoder(event) => {
                    self.on_encoder_event(event).await
                }
            }
        }
    }
}
mod multi_event_reversed {
    use super::{
        ConfigEvent, EncoderEvent, KeyEvent, ModeEvent, controller, input_processor,
    };
    pub struct ReversedMultiControllerHybridProcessor;
    pub enum ReversedMultiControllerHybridProcessorInputEventEnum {
        Key(KeyEvent),
        Encoder(EncoderEvent),
    }
    #[automatically_derived]
    impl ::core::clone::Clone for ReversedMultiControllerHybridProcessorInputEventEnum {
        #[inline]
        fn clone(&self) -> ReversedMultiControllerHybridProcessorInputEventEnum {
            match self {
                ReversedMultiControllerHybridProcessorInputEventEnum::Key(__self_0) => {
                    ReversedMultiControllerHybridProcessorInputEventEnum::Key(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
                ReversedMultiControllerHybridProcessorInputEventEnum::Encoder(
                    __self_0,
                ) => {
                    ReversedMultiControllerHybridProcessorInputEventEnum::Encoder(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
            }
        }
    }
    /// Event subscriber for aggregated events
    pub struct ReversedMultiControllerHybridProcessorInputEventSubscriber {
        sub0: <KeyEvent as ::rmk::event::SubscribableInputEvent>::Subscriber,
        sub1: <EncoderEvent as ::rmk::event::SubscribableInputEvent>::Subscriber,
    }
    impl ReversedMultiControllerHybridProcessorInputEventSubscriber {
        /// Create a new event subscriber
        pub fn new() -> Self {
            Self {
                sub0: <KeyEvent as ::rmk::event::SubscribableInputEvent>::input_subscriber(),
                sub1: <EncoderEvent as ::rmk::event::SubscribableInputEvent>::input_subscriber(),
            }
        }
    }
    impl ::rmk::event::EventSubscriber
    for ReversedMultiControllerHybridProcessorInputEventSubscriber {
        type Event = ReversedMultiControllerHybridProcessorInputEventEnum;
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
                            ReversedMultiControllerHybridProcessorInputEventEnum::Key(
                                event,
                            )
                        }
                        __PrivResult::_1(event) => {
                            ReversedMultiControllerHybridProcessorInputEventEnum::Encoder(
                                event,
                            )
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::event::SubscribableInputEvent
    for ReversedMultiControllerHybridProcessorInputEventEnum {
        type Subscriber = ReversedMultiControllerHybridProcessorInputEventSubscriber;
        fn input_subscriber() -> Self::Subscriber {
            ReversedMultiControllerHybridProcessorInputEventSubscriber::new()
        }
    }
    impl ::rmk::input_device::InputProcessor for ReversedMultiControllerHybridProcessor {
        type Event = ReversedMultiControllerHybridProcessorInputEventEnum;
        async fn process(&mut self, event: Self::Event) {
            match event {
                ReversedMultiControllerHybridProcessorInputEventEnum::Key(event) => {
                    self.on_key_event(event).await
                }
                ReversedMultiControllerHybridProcessorInputEventEnum::Encoder(event) => {
                    self.on_encoder_event(event).await
                }
            }
        }
    }
    pub enum ReversedMultiControllerHybridProcessorControllerEventEnum {
        Config(ConfigEvent),
        Mode(ModeEvent),
    }
    #[automatically_derived]
    impl ::core::clone::Clone
    for ReversedMultiControllerHybridProcessorControllerEventEnum {
        #[inline]
        fn clone(&self) -> ReversedMultiControllerHybridProcessorControllerEventEnum {
            match self {
                ReversedMultiControllerHybridProcessorControllerEventEnum::Config(
                    __self_0,
                ) => {
                    ReversedMultiControllerHybridProcessorControllerEventEnum::Config(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
                ReversedMultiControllerHybridProcessorControllerEventEnum::Mode(
                    __self_0,
                ) => {
                    ReversedMultiControllerHybridProcessorControllerEventEnum::Mode(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
            }
        }
    }
    /// Event subscriber for aggregated events
    pub struct ReversedMultiControllerHybridProcessorControllerEventSubscriber {
        sub0: <ConfigEvent as ::rmk::event::SubscribableControllerEvent>::Subscriber,
        sub1: <ModeEvent as ::rmk::event::SubscribableControllerEvent>::Subscriber,
    }
    impl ReversedMultiControllerHybridProcessorControllerEventSubscriber {
        /// Create a new event subscriber
        pub fn new() -> Self {
            Self {
                sub0: <ConfigEvent as ::rmk::event::SubscribableControllerEvent>::controller_subscriber(),
                sub1: <ModeEvent as ::rmk::event::SubscribableControllerEvent>::controller_subscriber(),
            }
        }
    }
    impl ::rmk::event::EventSubscriber
    for ReversedMultiControllerHybridProcessorControllerEventSubscriber {
        type Event = ReversedMultiControllerHybridProcessorControllerEventEnum;
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
                            ReversedMultiControllerHybridProcessorControllerEventEnum::Config(
                                event,
                            )
                        }
                        __PrivResult::_1(event) => {
                            ReversedMultiControllerHybridProcessorControllerEventEnum::Mode(
                                event,
                            )
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::event::SubscribableControllerEvent
    for ReversedMultiControllerHybridProcessorControllerEventEnum {
        type Subscriber = ReversedMultiControllerHybridProcessorControllerEventSubscriber;
        fn controller_subscriber() -> Self::Subscriber {
            ReversedMultiControllerHybridProcessorControllerEventSubscriber::new()
        }
    }
    impl ::rmk::controller::Controller for ReversedMultiControllerHybridProcessor {
        type Event = ReversedMultiControllerHybridProcessorControllerEventEnum;
        async fn process_event(&mut self, event: Self::Event) {
            match event {
                ReversedMultiControllerHybridProcessorControllerEventEnum::Config(
                    event,
                ) => self.on_config_event(event).await,
                ReversedMultiControllerHybridProcessorControllerEventEnum::Mode(
                    event,
                ) => self.on_mode_event(event).await,
            }
        }
    }
    impl ::rmk::input_device::Runnable for ReversedMultiControllerHybridProcessor {
        async fn run(&mut self) -> ! {
            use ::rmk::event::SubscribableInputEvent;
            use ::rmk::input_device::InputProcessor;
            use ::rmk::event::SubscribableControllerEvent;
            use ::rmk::controller::Controller;
            use ::rmk::event::EventSubscriber;
            use ::rmk::futures::FutureExt;
            let mut proc_sub = <<Self as ::rmk::input_device::InputProcessor>::Event as ::rmk::event::SubscribableInputEvent>::input_subscriber();
            let mut ctrl_sub = <<Self as ::rmk::controller::Controller>::Event as ::rmk::event::SubscribableControllerEvent>::controller_subscriber();
            loop {
                {
                    use ::futures_util::__private as __futures_crate;
                    {
                        enum __PrivResult<_0, _1> {
                            _0(_0),
                            _1(_1),
                        }
                        let __select_result = {
                            let mut _0 = proc_sub.next_event().fuse();
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
                            __PrivResult::_0(proc_event) => {
                                self.process(proc_event).await;
                            }
                            __PrivResult::_1(ctrl_event) => {
                                <Self as ::rmk::controller::Controller>::process_event(
                                        self,
                                        ctrl_event,
                                    )
                                    .await;
                            }
                        }
                    }
                }
            }
        }
    }
}
mod multi_event_reversed_polling {
    use super::{
        ConfigEvent, EncoderEvent, KeyEvent, ModeEvent, controller, input_processor,
    };
    pub struct ReversedPollingMultiControllerHybridProcessor;
    pub enum ReversedPollingMultiControllerHybridProcessorInputEventEnum {
        Key(KeyEvent),
        Encoder(EncoderEvent),
    }
    #[automatically_derived]
    impl ::core::clone::Clone
    for ReversedPollingMultiControllerHybridProcessorInputEventEnum {
        #[inline]
        fn clone(&self) -> ReversedPollingMultiControllerHybridProcessorInputEventEnum {
            match self {
                ReversedPollingMultiControllerHybridProcessorInputEventEnum::Key(
                    __self_0,
                ) => {
                    ReversedPollingMultiControllerHybridProcessorInputEventEnum::Key(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
                ReversedPollingMultiControllerHybridProcessorInputEventEnum::Encoder(
                    __self_0,
                ) => {
                    ReversedPollingMultiControllerHybridProcessorInputEventEnum::Encoder(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
            }
        }
    }
    /// Event subscriber for aggregated events
    pub struct ReversedPollingMultiControllerHybridProcessorInputEventSubscriber {
        sub0: <KeyEvent as ::rmk::event::SubscribableInputEvent>::Subscriber,
        sub1: <EncoderEvent as ::rmk::event::SubscribableInputEvent>::Subscriber,
    }
    impl ReversedPollingMultiControllerHybridProcessorInputEventSubscriber {
        /// Create a new event subscriber
        pub fn new() -> Self {
            Self {
                sub0: <KeyEvent as ::rmk::event::SubscribableInputEvent>::input_subscriber(),
                sub1: <EncoderEvent as ::rmk::event::SubscribableInputEvent>::input_subscriber(),
            }
        }
    }
    impl ::rmk::event::EventSubscriber
    for ReversedPollingMultiControllerHybridProcessorInputEventSubscriber {
        type Event = ReversedPollingMultiControllerHybridProcessorInputEventEnum;
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
                            ReversedPollingMultiControllerHybridProcessorInputEventEnum::Key(
                                event,
                            )
                        }
                        __PrivResult::_1(event) => {
                            ReversedPollingMultiControllerHybridProcessorInputEventEnum::Encoder(
                                event,
                            )
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::event::SubscribableInputEvent
    for ReversedPollingMultiControllerHybridProcessorInputEventEnum {
        type Subscriber = ReversedPollingMultiControllerHybridProcessorInputEventSubscriber;
        fn input_subscriber() -> Self::Subscriber {
            ReversedPollingMultiControllerHybridProcessorInputEventSubscriber::new()
        }
    }
    impl ::rmk::input_device::InputProcessor
    for ReversedPollingMultiControllerHybridProcessor {
        type Event = ReversedPollingMultiControllerHybridProcessorInputEventEnum;
        async fn process(&mut self, event: Self::Event) {
            match event {
                ReversedPollingMultiControllerHybridProcessorInputEventEnum::Key(
                    event,
                ) => self.on_key_event(event).await,
                ReversedPollingMultiControllerHybridProcessorInputEventEnum::Encoder(
                    event,
                ) => self.on_encoder_event(event).await,
            }
        }
    }
    pub enum ReversedPollingMultiControllerHybridProcessorControllerEventEnum {
        Config(ConfigEvent),
        Mode(ModeEvent),
    }
    #[automatically_derived]
    impl ::core::clone::Clone
    for ReversedPollingMultiControllerHybridProcessorControllerEventEnum {
        #[inline]
        fn clone(
            &self,
        ) -> ReversedPollingMultiControllerHybridProcessorControllerEventEnum {
            match self {
                ReversedPollingMultiControllerHybridProcessorControllerEventEnum::Config(
                    __self_0,
                ) => {
                    ReversedPollingMultiControllerHybridProcessorControllerEventEnum::Config(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
                ReversedPollingMultiControllerHybridProcessorControllerEventEnum::Mode(
                    __self_0,
                ) => {
                    ReversedPollingMultiControllerHybridProcessorControllerEventEnum::Mode(
                        ::core::clone::Clone::clone(__self_0),
                    )
                }
            }
        }
    }
    /// Event subscriber for aggregated events
    pub struct ReversedPollingMultiControllerHybridProcessorControllerEventSubscriber {
        sub0: <ConfigEvent as ::rmk::event::SubscribableControllerEvent>::Subscriber,
        sub1: <ModeEvent as ::rmk::event::SubscribableControllerEvent>::Subscriber,
    }
    impl ReversedPollingMultiControllerHybridProcessorControllerEventSubscriber {
        /// Create a new event subscriber
        pub fn new() -> Self {
            Self {
                sub0: <ConfigEvent as ::rmk::event::SubscribableControllerEvent>::controller_subscriber(),
                sub1: <ModeEvent as ::rmk::event::SubscribableControllerEvent>::controller_subscriber(),
            }
        }
    }
    impl ::rmk::event::EventSubscriber
    for ReversedPollingMultiControllerHybridProcessorControllerEventSubscriber {
        type Event = ReversedPollingMultiControllerHybridProcessorControllerEventEnum;
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
                            ReversedPollingMultiControllerHybridProcessorControllerEventEnum::Config(
                                event,
                            )
                        }
                        __PrivResult::_1(event) => {
                            ReversedPollingMultiControllerHybridProcessorControllerEventEnum::Mode(
                                event,
                            )
                        }
                    }
                }
            }
        }
    }
    impl ::rmk::event::SubscribableControllerEvent
    for ReversedPollingMultiControllerHybridProcessorControllerEventEnum {
        type Subscriber = ReversedPollingMultiControllerHybridProcessorControllerEventSubscriber;
        fn controller_subscriber() -> Self::Subscriber {
            ReversedPollingMultiControllerHybridProcessorControllerEventSubscriber::new()
        }
    }
    impl ::rmk::controller::Controller
    for ReversedPollingMultiControllerHybridProcessor {
        type Event = ReversedPollingMultiControllerHybridProcessorControllerEventEnum;
        async fn process_event(&mut self, event: Self::Event) {
            match event {
                ReversedPollingMultiControllerHybridProcessorControllerEventEnum::Config(
                    event,
                ) => self.on_config_event(event).await,
                ReversedPollingMultiControllerHybridProcessorControllerEventEnum::Mode(
                    event,
                ) => self.on_mode_event(event).await,
            }
        }
    }
    impl ::rmk::controller::PollingController
    for ReversedPollingMultiControllerHybridProcessor {
        fn interval(&self) -> ::embassy_time::Duration {
            ::embassy_time::Duration::from_millis(20u64)
        }
        async fn update(&mut self) {
            self.poll().await
        }
    }
    impl ::rmk::input_device::Runnable
    for ReversedPollingMultiControllerHybridProcessor {
        async fn run(&mut self) -> ! {
            use ::rmk::event::SubscribableInputEvent;
            use ::rmk::input_device::InputProcessor;
            use ::rmk::event::SubscribableControllerEvent;
            use ::rmk::controller::Controller;
            use ::rmk::event::EventSubscriber;
            use ::rmk::futures::FutureExt;
            use ::rmk::controller::PollingController;
            let mut proc_sub = <<Self as ::rmk::input_device::InputProcessor>::Event as ::rmk::event::SubscribableInputEvent>::input_subscriber();
            let mut ctrl_sub = <<Self as ::rmk::controller::Controller>::Event as ::rmk::event::SubscribableControllerEvent>::controller_subscriber();
            let mut last = ::embassy_time::Instant::now();
            loop {
                let elapsed = last.elapsed();
                let interval = ::embassy_time::Duration::from_millis(20u64);
                let timer = ::embassy_time::Timer::after(
                    interval
                        .checked_sub(elapsed)
                        .unwrap_or(::embassy_time::Duration::MIN),
                );
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
                            let mut _1 = proc_sub.next_event().fuse();
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
                                <Self as PollingController>::update(self).await;
                                last = ::embassy_time::Instant::now();
                            }
                            __PrivResult::_1(proc_event) => {
                                self.process(proc_event).await;
                            }
                            __PrivResult::_2(ctrl_event) => {
                                <Self as ::rmk::controller::Controller>::process_event(
                                        self,
                                        ctrl_event,
                                    )
                                    .await;
                            }
                        }
                    }
                }
            }
        }
    }
}
