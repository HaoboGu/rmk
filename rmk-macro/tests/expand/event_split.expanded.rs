//! Expand tests for #[event(split)] macro.
//!
//! Tests:
//! - Split event with auto kind (split = 0) using PubSub channel
//! - Split event with explicit kind using MPSC channel
use rmk_macro::event;
/// Split event with auto kind and PubSub channel
mod split_pubsub {
    use super::event;
    pub struct CustomSplitEvent {
        pub value: u16,
        pub flag: bool,
    }
    #[automatically_derived]
    #[doc(hidden)]
    unsafe impl ::core::clone::TrivialClone for CustomSplitEvent {}
    #[automatically_derived]
    impl ::core::clone::Clone for CustomSplitEvent {
        #[inline]
        fn clone(&self) -> CustomSplitEvent {
            let _: ::core::clone::AssertParamIsClone<u16>;
            let _: ::core::clone::AssertParamIsClone<bool>;
            *self
        }
    }
    #[automatically_derived]
    impl ::core::marker::Copy for CustomSplitEvent {}
    #[automatically_derived]
    impl ::core::fmt::Debug for CustomSplitEvent {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field2_finish(
                f,
                "CustomSplitEvent",
                "value",
                &self.value,
                "flag",
                &&self.flag,
            )
        }
    }
    #[doc(hidden)]
    #[allow(
        non_upper_case_globals,
        unused_attributes,
        unused_qualifications,
        clippy::absolute_paths,
    )]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl _serde::Serialize for CustomSplitEvent {
            fn serialize<__S>(
                &self,
                __serializer: __S,
            ) -> _serde::__private228::Result<__S::Ok, __S::Error>
            where
                __S: _serde::Serializer,
            {
                let mut __serde_state = _serde::Serializer::serialize_struct(
                    __serializer,
                    "CustomSplitEvent",
                    false as usize + 1 + 1,
                )?;
                _serde::ser::SerializeStruct::serialize_field(
                    &mut __serde_state,
                    "value",
                    &self.value,
                )?;
                _serde::ser::SerializeStruct::serialize_field(
                    &mut __serde_state,
                    "flag",
                    &self.flag,
                )?;
                _serde::ser::SerializeStruct::end(__serde_state)
            }
        }
    };
    #[doc(hidden)]
    #[allow(
        non_upper_case_globals,
        unused_attributes,
        unused_qualifications,
        clippy::absolute_paths,
    )]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl<'de> _serde::Deserialize<'de> for CustomSplitEvent {
            fn deserialize<__D>(
                __deserializer: __D,
            ) -> _serde::__private228::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<'de>,
            {
                #[allow(non_camel_case_types)]
                #[doc(hidden)]
                enum __Field {
                    __field0,
                    __field1,
                    __ignore,
                }
                #[doc(hidden)]
                struct __FieldVisitor;
                #[automatically_derived]
                impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                    type Value = __Field;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private228::Formatter,
                    ) -> _serde::__private228::fmt::Result {
                        _serde::__private228::Formatter::write_str(
                            __formatter,
                            "field identifier",
                        )
                    }
                    fn visit_u64<__E>(
                        self,
                        __value: u64,
                    ) -> _serde::__private228::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            0u64 => _serde::__private228::Ok(__Field::__field0),
                            1u64 => _serde::__private228::Ok(__Field::__field1),
                            _ => _serde::__private228::Ok(__Field::__ignore),
                        }
                    }
                    fn visit_str<__E>(
                        self,
                        __value: &str,
                    ) -> _serde::__private228::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            "value" => _serde::__private228::Ok(__Field::__field0),
                            "flag" => _serde::__private228::Ok(__Field::__field1),
                            _ => _serde::__private228::Ok(__Field::__ignore),
                        }
                    }
                    fn visit_bytes<__E>(
                        self,
                        __value: &[u8],
                    ) -> _serde::__private228::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            b"value" => _serde::__private228::Ok(__Field::__field0),
                            b"flag" => _serde::__private228::Ok(__Field::__field1),
                            _ => _serde::__private228::Ok(__Field::__ignore),
                        }
                    }
                }
                #[automatically_derived]
                impl<'de> _serde::Deserialize<'de> for __Field {
                    #[inline]
                    fn deserialize<__D>(
                        __deserializer: __D,
                    ) -> _serde::__private228::Result<Self, __D::Error>
                    where
                        __D: _serde::Deserializer<'de>,
                    {
                        _serde::Deserializer::deserialize_identifier(
                            __deserializer,
                            __FieldVisitor,
                        )
                    }
                }
                #[doc(hidden)]
                struct __Visitor<'de> {
                    marker: _serde::__private228::PhantomData<CustomSplitEvent>,
                    lifetime: _serde::__private228::PhantomData<&'de ()>,
                }
                #[automatically_derived]
                impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                    type Value = CustomSplitEvent;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private228::Formatter,
                    ) -> _serde::__private228::fmt::Result {
                        _serde::__private228::Formatter::write_str(
                            __formatter,
                            "struct CustomSplitEvent",
                        )
                    }
                    #[inline]
                    fn visit_seq<__A>(
                        self,
                        mut __seq: __A,
                    ) -> _serde::__private228::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::SeqAccess<'de>,
                    {
                        let __field0 = match _serde::de::SeqAccess::next_element::<
                            u16,
                        >(&mut __seq)? {
                            _serde::__private228::Some(__value) => __value,
                            _serde::__private228::None => {
                                return _serde::__private228::Err(
                                    _serde::de::Error::invalid_length(
                                        0usize,
                                        &"struct CustomSplitEvent with 2 elements",
                                    ),
                                );
                            }
                        };
                        let __field1 = match _serde::de::SeqAccess::next_element::<
                            bool,
                        >(&mut __seq)? {
                            _serde::__private228::Some(__value) => __value,
                            _serde::__private228::None => {
                                return _serde::__private228::Err(
                                    _serde::de::Error::invalid_length(
                                        1usize,
                                        &"struct CustomSplitEvent with 2 elements",
                                    ),
                                );
                            }
                        };
                        _serde::__private228::Ok(CustomSplitEvent {
                            value: __field0,
                            flag: __field1,
                        })
                    }
                    #[inline]
                    fn visit_map<__A>(
                        self,
                        mut __map: __A,
                    ) -> _serde::__private228::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::MapAccess<'de>,
                    {
                        let mut __field0: _serde::__private228::Option<u16> = _serde::__private228::None;
                        let mut __field1: _serde::__private228::Option<bool> = _serde::__private228::None;
                        while let _serde::__private228::Some(__key) = _serde::de::MapAccess::next_key::<
                            __Field,
                        >(&mut __map)? {
                            match __key {
                                __Field::__field0 => {
                                    if _serde::__private228::Option::is_some(&__field0) {
                                        return _serde::__private228::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field("value"),
                                        );
                                    }
                                    __field0 = _serde::__private228::Some(
                                        _serde::de::MapAccess::next_value::<u16>(&mut __map)?,
                                    );
                                }
                                __Field::__field1 => {
                                    if _serde::__private228::Option::is_some(&__field1) {
                                        return _serde::__private228::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field("flag"),
                                        );
                                    }
                                    __field1 = _serde::__private228::Some(
                                        _serde::de::MapAccess::next_value::<bool>(&mut __map)?,
                                    );
                                }
                                _ => {
                                    let _ = _serde::de::MapAccess::next_value::<
                                        _serde::de::IgnoredAny,
                                    >(&mut __map)?;
                                }
                            }
                        }
                        let __field0 = match __field0 {
                            _serde::__private228::Some(__field0) => __field0,
                            _serde::__private228::None => {
                                _serde::__private228::de::missing_field("value")?
                            }
                        };
                        let __field1 = match __field1 {
                            _serde::__private228::Some(__field1) => __field1,
                            _serde::__private228::None => {
                                _serde::__private228::de::missing_field("flag")?
                            }
                        };
                        _serde::__private228::Ok(CustomSplitEvent {
                            value: __field0,
                            flag: __field1,
                        })
                    }
                }
                #[doc(hidden)]
                const FIELDS: &'static [&'static str] = &["value", "flag"];
                _serde::Deserializer::deserialize_struct(
                    __deserializer,
                    "CustomSplitEvent",
                    FIELDS,
                    __Visitor {
                        marker: _serde::__private228::PhantomData::<CustomSplitEvent>,
                        lifetime: _serde::__private228::PhantomData,
                    },
                )
            }
        }
    };
    impl ::postcard::experimental::max_size::MaxSize for CustomSplitEvent {
        const POSTCARD_MAX_SIZE: usize = 0
            + <u16 as ::postcard::experimental::max_size::MaxSize>::POSTCARD_MAX_SIZE
            + <bool as ::postcard::experimental::max_size::MaxSize>::POSTCARD_MAX_SIZE;
    }
    #[doc(hidden)]
    static CUSTOM_SPLIT_EVENT_EVENT_CHANNEL: ::rmk::embassy_sync::pubsub::PubSubChannel<
        ::rmk::RawMutex,
        CustomSplitEvent,
        { 1 },
        { 4 },
        { 2 },
    > = ::rmk::embassy_sync::pubsub::PubSubChannel::new();
    impl ::rmk::split::forward::SplitForwardable for CustomSplitEvent {
        const SPLIT_EVENT_KIND: u16 = 14674u16;
    }
    #[doc(hidden)]
    #[unsafe(no_mangle)]
    pub static __RMK_SPLIT_EVENT_KIND_14674: () = ();
    impl ::rmk::event::PublishableEvent for CustomSplitEvent {
        type Publisher = ::rmk::split::forward::SplitForwardingPublisher<
            ::rmk::embassy_sync::pubsub::ImmediatePublisher<
                'static,
                ::rmk::RawMutex,
                CustomSplitEvent,
                { 1 },
                { 4 },
                { 2 },
            >,
        >;
        fn publisher() -> Self::Publisher {
            ::rmk::split::forward::SplitForwardingPublisher::new(
                CUSTOM_SPLIT_EVENT_EVENT_CHANNEL.immediate_publisher(),
            )
        }
    }
    impl ::rmk::event::SubscribableEvent for CustomSplitEvent {
        type Subscriber = ::rmk::split::forward::SplitAwareSubscriber<
            ::rmk::embassy_sync::pubsub::Subscriber<
                'static,
                ::rmk::RawMutex,
                CustomSplitEvent,
                { 1 },
                { 4 },
                { 2 },
            >,
            CustomSplitEvent,
        >;
        fn subscriber() -> Self::Subscriber {
            ::rmk::split::forward::SplitAwareSubscriber::new(
                CUSTOM_SPLIT_EVENT_EVENT_CHANNEL
                    .subscriber()
                    .expect("Failed to create subscriber for CustomSplitEvent"),
            )
        }
    }
    impl ::rmk::event::AsyncPublishableEvent for CustomSplitEvent {
        type AsyncPublisher = ::rmk::split::forward::SplitForwardingPublisher<
            ::rmk::embassy_sync::pubsub::Publisher<
                'static,
                ::rmk::RawMutex,
                CustomSplitEvent,
                { 1 },
                { 4 },
                { 2 },
            >,
        >;
        fn publisher_async() -> Self::AsyncPublisher {
            ::rmk::split::forward::SplitForwardingPublisher::new(
                CUSTOM_SPLIT_EVENT_EVENT_CHANNEL
                    .publisher()
                    .expect("Failed to create async publisher for CustomSplitEvent"),
            )
        }
    }
}
/// Split event with explicit kind and MPSC channel
mod split_mpsc {
    use super::event;
    pub struct SensorEvent {
        pub reading: i16,
    }
    #[automatically_derived]
    #[doc(hidden)]
    unsafe impl ::core::clone::TrivialClone for SensorEvent {}
    #[automatically_derived]
    impl ::core::clone::Clone for SensorEvent {
        #[inline]
        fn clone(&self) -> SensorEvent {
            let _: ::core::clone::AssertParamIsClone<i16>;
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
                "reading",
                &&self.reading,
            )
        }
    }
    #[doc(hidden)]
    #[allow(
        non_upper_case_globals,
        unused_attributes,
        unused_qualifications,
        clippy::absolute_paths,
    )]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl _serde::Serialize for SensorEvent {
            fn serialize<__S>(
                &self,
                __serializer: __S,
            ) -> _serde::__private228::Result<__S::Ok, __S::Error>
            where
                __S: _serde::Serializer,
            {
                let mut __serde_state = _serde::Serializer::serialize_struct(
                    __serializer,
                    "SensorEvent",
                    false as usize + 1,
                )?;
                _serde::ser::SerializeStruct::serialize_field(
                    &mut __serde_state,
                    "reading",
                    &self.reading,
                )?;
                _serde::ser::SerializeStruct::end(__serde_state)
            }
        }
    };
    #[doc(hidden)]
    #[allow(
        non_upper_case_globals,
        unused_attributes,
        unused_qualifications,
        clippy::absolute_paths,
    )]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl<'de> _serde::Deserialize<'de> for SensorEvent {
            fn deserialize<__D>(
                __deserializer: __D,
            ) -> _serde::__private228::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<'de>,
            {
                #[allow(non_camel_case_types)]
                #[doc(hidden)]
                enum __Field {
                    __field0,
                    __ignore,
                }
                #[doc(hidden)]
                struct __FieldVisitor;
                #[automatically_derived]
                impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                    type Value = __Field;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private228::Formatter,
                    ) -> _serde::__private228::fmt::Result {
                        _serde::__private228::Formatter::write_str(
                            __formatter,
                            "field identifier",
                        )
                    }
                    fn visit_u64<__E>(
                        self,
                        __value: u64,
                    ) -> _serde::__private228::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            0u64 => _serde::__private228::Ok(__Field::__field0),
                            _ => _serde::__private228::Ok(__Field::__ignore),
                        }
                    }
                    fn visit_str<__E>(
                        self,
                        __value: &str,
                    ) -> _serde::__private228::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            "reading" => _serde::__private228::Ok(__Field::__field0),
                            _ => _serde::__private228::Ok(__Field::__ignore),
                        }
                    }
                    fn visit_bytes<__E>(
                        self,
                        __value: &[u8],
                    ) -> _serde::__private228::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            b"reading" => _serde::__private228::Ok(__Field::__field0),
                            _ => _serde::__private228::Ok(__Field::__ignore),
                        }
                    }
                }
                #[automatically_derived]
                impl<'de> _serde::Deserialize<'de> for __Field {
                    #[inline]
                    fn deserialize<__D>(
                        __deserializer: __D,
                    ) -> _serde::__private228::Result<Self, __D::Error>
                    where
                        __D: _serde::Deserializer<'de>,
                    {
                        _serde::Deserializer::deserialize_identifier(
                            __deserializer,
                            __FieldVisitor,
                        )
                    }
                }
                #[doc(hidden)]
                struct __Visitor<'de> {
                    marker: _serde::__private228::PhantomData<SensorEvent>,
                    lifetime: _serde::__private228::PhantomData<&'de ()>,
                }
                #[automatically_derived]
                impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                    type Value = SensorEvent;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private228::Formatter,
                    ) -> _serde::__private228::fmt::Result {
                        _serde::__private228::Formatter::write_str(
                            __formatter,
                            "struct SensorEvent",
                        )
                    }
                    #[inline]
                    fn visit_seq<__A>(
                        self,
                        mut __seq: __A,
                    ) -> _serde::__private228::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::SeqAccess<'de>,
                    {
                        let __field0 = match _serde::de::SeqAccess::next_element::<
                            i16,
                        >(&mut __seq)? {
                            _serde::__private228::Some(__value) => __value,
                            _serde::__private228::None => {
                                return _serde::__private228::Err(
                                    _serde::de::Error::invalid_length(
                                        0usize,
                                        &"struct SensorEvent with 1 element",
                                    ),
                                );
                            }
                        };
                        _serde::__private228::Ok(SensorEvent { reading: __field0 })
                    }
                    #[inline]
                    fn visit_map<__A>(
                        self,
                        mut __map: __A,
                    ) -> _serde::__private228::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::MapAccess<'de>,
                    {
                        let mut __field0: _serde::__private228::Option<i16> = _serde::__private228::None;
                        while let _serde::__private228::Some(__key) = _serde::de::MapAccess::next_key::<
                            __Field,
                        >(&mut __map)? {
                            match __key {
                                __Field::__field0 => {
                                    if _serde::__private228::Option::is_some(&__field0) {
                                        return _serde::__private228::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "reading",
                                            ),
                                        );
                                    }
                                    __field0 = _serde::__private228::Some(
                                        _serde::de::MapAccess::next_value::<i16>(&mut __map)?,
                                    );
                                }
                                _ => {
                                    let _ = _serde::de::MapAccess::next_value::<
                                        _serde::de::IgnoredAny,
                                    >(&mut __map)?;
                                }
                            }
                        }
                        let __field0 = match __field0 {
                            _serde::__private228::Some(__field0) => __field0,
                            _serde::__private228::None => {
                                _serde::__private228::de::missing_field("reading")?
                            }
                        };
                        _serde::__private228::Ok(SensorEvent { reading: __field0 })
                    }
                }
                #[doc(hidden)]
                const FIELDS: &'static [&'static str] = &["reading"];
                _serde::Deserializer::deserialize_struct(
                    __deserializer,
                    "SensorEvent",
                    FIELDS,
                    __Visitor {
                        marker: _serde::__private228::PhantomData::<SensorEvent>,
                        lifetime: _serde::__private228::PhantomData,
                    },
                )
            }
        }
    };
    impl ::postcard::experimental::max_size::MaxSize for SensorEvent {
        const POSTCARD_MAX_SIZE: usize = 0
            + <i16 as ::postcard::experimental::max_size::MaxSize>::POSTCARD_MAX_SIZE;
    }
    #[doc(hidden)]
    static SENSOR_EVENT_EVENT_CHANNEL: ::rmk::embassy_sync::channel::Channel<
        ::rmk::RawMutex,
        SensorEvent,
        { 8 },
    > = ::rmk::embassy_sync::channel::Channel::new();
    impl ::rmk::split::forward::SplitForwardable for SensorEvent {
        const SPLIT_EVENT_KIND: u16 = 42u16;
    }
    #[doc(hidden)]
    #[unsafe(no_mangle)]
    pub static __RMK_SPLIT_EVENT_KIND_42: () = ();
    impl ::rmk::event::PublishableEvent for SensorEvent {
        type Publisher = ::rmk::split::forward::SplitForwardingPublisher<
            ::rmk::embassy_sync::channel::Sender<
                'static,
                ::rmk::RawMutex,
                SensorEvent,
                { 8 },
            >,
        >;
        fn publisher() -> Self::Publisher {
            ::rmk::split::forward::SplitForwardingPublisher::new(
                SENSOR_EVENT_EVENT_CHANNEL.sender(),
            )
        }
    }
    impl ::rmk::event::SubscribableEvent for SensorEvent {
        type Subscriber = ::rmk::split::forward::SplitAwareSubscriber<
            ::rmk::embassy_sync::channel::Receiver<
                'static,
                ::rmk::RawMutex,
                SensorEvent,
                { 8 },
            >,
            SensorEvent,
        >;
        fn subscriber() -> Self::Subscriber {
            ::rmk::split::forward::SplitAwareSubscriber::new(
                SENSOR_EVENT_EVENT_CHANNEL.receiver(),
            )
        }
    }
    impl ::rmk::event::AsyncPublishableEvent for SensorEvent {
        type AsyncPublisher = ::rmk::split::forward::SplitForwardingPublisher<
            ::rmk::embassy_sync::channel::Sender<
                'static,
                ::rmk::RawMutex,
                SensorEvent,
                { 8 },
            >,
        >;
        fn publisher_async() -> Self::AsyncPublisher {
            ::rmk::split::forward::SplitForwardingPublisher::new(
                SENSOR_EVENT_EVENT_CHANNEL.sender(),
            )
        }
    }
}
