/// Local copy of `postcard_rpc::define_dispatch!` macro.
///
/// This is migrated from postcard-rpc so that rmk can use the published
/// crates.io version of postcard-rpc without depending on a local checkout.
///
/// All `$crate::` references to postcard-rpc internals have been replaced
/// with `::postcard_rpc::`.  Recursive invocations still use `$crate::`
/// which now resolves to this crate (`rmk`).
#[doc(hidden)]
#[macro_export]
macro_rules! define_dispatch {
    //////////////////////////////////////////////////////////////////////////////
    // ENDPOINT HANDLER EXPANSION ARMS
    //////////////////////////////////////////////////////////////////////////////

    // This is the "blocking execution" arm for defining an endpoint
    (@ep_arm blocking ($endpoint:ty) $handler:ident $context:ident $header:ident $req:ident $outputter:ident ($spawn_fn:path) $spawner:ident) => {
        {
            let reply = $handler($context, $header.clone(), $req);
            if $outputter.reply::<$endpoint>($header.seq_no, &reply).await.is_err() {
                let err = ::postcard_rpc::standard_icd::WireError::SerFailed;
                $outputter.error($header.seq_no, err).await
            } else {
                Ok(())
            }
        }
    };
    // This is the "async execution" arm for defining an endpoint
    (@ep_arm async ($endpoint:ty) $handler:ident $context:ident $header:ident $req:ident $outputter:ident ($spawn_fn:path) $spawner:ident) => {
        {
            let reply = $handler($context, $header.clone(), $req).await;
            if $outputter.reply::<$endpoint>($header.seq_no, &reply).await.is_err() {
                let err = ::postcard_rpc::standard_icd::WireError::SerFailed;
                $outputter.error($header.seq_no, err).await
            } else {
                Ok(())
            }
        }
    };
    // This is the "spawn an embassy task" arm for defining an endpoint
    (@ep_arm spawn ($endpoint:ty) $handler:ident $context:ident $header:ident $req:ident $outputter:ident ($spawn_fn:path) $spawner:ident) => {
        {
            let context = ::postcard_rpc::server::SpawnContext::spawn_ctxt($context);
            if $spawn_fn($spawner, $handler(context, $header.clone(), $req, $outputter.clone())).is_err() {
                let err = ::postcard_rpc::standard_icd::WireError::FailedToSpawn;
                $outputter.error($header.seq_no, err).await
            } else {
                Ok(())
            }
        }
    };

    //////////////////////////////////////////////////////////////////////////////
    // TOPIC HANDLER EXPANSION ARMS
    //////////////////////////////////////////////////////////////////////////////

    // This is the "blocking execution" arm for defining a topic
    (@tp_arm blocking $handler:ident $context:ident $header:ident $msg:ident $outputter:ident ($spawn_fn:path) $spawner:ident) => {
        {
            $handler($context, $header.clone(), $msg, $outputter);
        }
    };
    // This is the "async execution" arm for defining a topic
    (@tp_arm async $handler:ident $context:ident $header:ident $msg:ident $outputter:ident ($spawn_fn:path) $spawner:ident) => {
        {
            $handler($context, $header.clone(), $msg, $outputter).await;
        }
    };
    (@tp_arm spawn $handler:ident $context:ident $header:ident $msg:ident $outputter:ident ($spawn_fn:path) $spawner:ident) => {
        {
            let context = ::postcard_rpc::server::SpawnContext::spawn_ctxt($context);
            let _ = $spawn_fn($spawner, $handler(context, $header.clone(), $msg, $outputter.clone()));
        }
    };



    //////////////////////////////////////////////////////////////////////////////
    // DISPATCH TRAIT IMPL for a given key size N (1, 2, 4, or 8).
    //
    // Handles both generic and non-generic cases: when the generic lists are
    // empty, `impl<>` and `App<N>` are still valid Rust.
    //////////////////////////////////////////////////////////////////////////////
    (@matcher [$($decl_gen:tt)*] [$($use_gen:tt)*]
        $n:literal $app_name:ident $tx_impl:ty; $spawn_fn:ident $key_ty:ty; $key_kind:expr;
        $req_key_name:ident / $topic_key_name:ident = $bytes_ty:ty;
        ($($endpoint:ty | $ep_flavor:tt | $ep_handler:ident)*)
        ($($topic_in:ty | $tp_flavor:tt | $tp_handler:ident)*)
    ) => {
        impl<$($decl_gen)*> ::postcard_rpc::server::Dispatch for $app_name<$($use_gen)* $n> {
            type Tx = $tx_impl;

            fn min_key_len(&self) -> ::postcard_rpc::header::VarKeyKind {
                $key_kind
            }

            /// Handle dispatching of a single frame
            async fn handle(
                &mut self,
                tx: &::postcard_rpc::server::Sender<Self::Tx>,
                hdr: &::postcard_rpc::header::VarHeader,
                body: &[u8],
            ) -> Result<(), <Self::Tx as ::postcard_rpc::server::WireTx>::Error> {
                let key = hdr.key;
                let Ok(keyb) = <$key_ty>::try_from(&key) else {
                    let err = ::postcard_rpc::standard_icd::WireError::KeyTooSmall;
                    return tx.error(hdr.seq_no, err).await;
                };
                match keyb {
                    // Standard ICD endpoints
                    //
                    // WARNING! If you add any more standard icd endpoints, make sure you ALSO add them
                    // to the dupe check in @main!
                    <::postcard_rpc::standard_icd::PingEndpoint as ::postcard_rpc::Endpoint>::$req_key_name => {
                        // Can we deserialize the request?
                        let Ok(req) = ::postcard_rpc::postcard::from_bytes::<<::postcard_rpc::standard_icd::PingEndpoint as ::postcard_rpc::Endpoint>::Request>(body) else {
                            let err = ::postcard_rpc::standard_icd::WireError::DeserFailed;
                            return tx.error(hdr.seq_no, err).await;
                        };

                        tx.reply::<::postcard_rpc::standard_icd::PingEndpoint>(hdr.seq_no, &req).await
                    },
                    <::postcard_rpc::standard_icd::GetAllSchemasEndpoint as ::postcard_rpc::Endpoint>::$req_key_name => {
                        tx.send_all_schemas(hdr, self.device_map).await
                    }
                    // WARNING! If you add any more standard icd endpoints, make sure you ALSO add them
                    // to the dupe check in @main!
                    //
                    // end standard_icd endpoints
                    $(
                        <$endpoint as ::postcard_rpc::Endpoint>::$req_key_name => {
                            // Can we deserialize the request?
                            let Ok(req) = ::postcard_rpc::postcard::from_bytes::<<$endpoint as ::postcard_rpc::Endpoint>::Request>(body) else {
                                let err = ::postcard_rpc::standard_icd::WireError::DeserFailed;
                                return tx.error(hdr.seq_no, err).await;
                            };

                            // Store some items as named bindings, so we can use `ident` in the
                            // recursive macro expansion. Load bearing order: we borrow `context`
                            // from `dispatch` because we need `dispatch` AFTER `context`, so NLL
                            // allows this to still borrowck
                            let dispatch = self;
                            let context = &mut dispatch.context;
                            #[allow(unused)]
                            let spawninfo = &dispatch.spawn;

                            // This will expand to the right "flavor" of handler
                            $crate::define_dispatch!(@ep_arm $ep_flavor ($endpoint) $ep_handler context hdr req tx ($spawn_fn) spawninfo)
                        }
                    )*
                    $(
                        <$topic_in as ::postcard_rpc::Topic>::$topic_key_name => {
                            // Can we deserialize the request?
                            let Ok(msg) = ::postcard_rpc::postcard::from_bytes::<<$topic_in as ::postcard_rpc::Topic>::Message>(body) else {
                                // This is a topic, not much to be done
                                return Ok(());
                            };

                            // Store some items as named bindings, so we can use `ident` in the
                            // recursive macro expansion. Load bearing order: we borrow `context`
                            // from `dispatch` because we need `dispatch` AFTER `context`, so NLL
                            // allows this to still borrowck
                            let dispatch = self;
                            let context = &mut dispatch.context;
                            #[allow(unused)]
                            let spawninfo = &dispatch.spawn;

                            $crate::define_dispatch!(@tp_arm $tp_flavor $tp_handler context hdr msg tx ($spawn_fn) spawninfo);
                            Ok(())
                        }
                    )*
                    _other => {
                        // huh! We have no idea what this key is supposed to be!
                        let err = ::postcard_rpc::standard_icd::WireError::UnknownKey;
                        tx.error(hdr.seq_no, err).await
                    },
                }
            }
        }
    };

    //////////////////////////////////////////////////////////////////////////////
    // @strip_generics TT-MUNCHER
    //
    // Processes raw generic params from [...] into three lists:
    //   decl_gen  - declaration form (with bounds): 'a, Tx: WireTx, const ROW: usize,
    //   use_gen   - usage form (names only):        'a, Tx, ROW,
    //   phantom   - PhantomData entries:             &'a (), Tx,
    //////////////////////////////////////////////////////////////////////////////

    // Done: no more tokens
    (@strip_generics [$($cb:tt)*] [$($decl:tt)*] [$($use:tt)*] [$($phantom:tt)*]) => {
        $crate::define_dispatch! { @main [$($cb)*] [$($decl)*] [$($use)*] [$($phantom)*] }
    };

    // Lifetime with trailing comma and more
    (@strip_generics [$($cb:tt)*] [$($d:tt)*] [$($u:tt)*] [$($p:tt)*]
        $lt:lifetime , $($rest:tt)*
    ) => {
        $crate::define_dispatch! { @strip_generics [$($cb)*]
            [$($d)* $lt,]  [$($u)* $lt,]  [$($p)* & $lt (),]
            $($rest)*
        }
    };

    // Lifetime, last param (no trailing comma)
    (@strip_generics [$($cb:tt)*] [$($d:tt)*] [$($u:tt)*] [$($p:tt)*]
        $lt:lifetime
    ) => {
        $crate::define_dispatch! { @strip_generics [$($cb)*]
            [$($d)* $lt,]  [$($u)* $lt,]  [$($p)* & $lt (),]
        }
    };

    // Const generic with trailing comma and more
    (@strip_generics [$($cb:tt)*] [$($d:tt)*] [$($u:tt)*] [$($p:tt)*]
        const $name:ident : $ty:ty , $($rest:tt)*
    ) => {
        $crate::define_dispatch! { @strip_generics [$($cb)*]
            [$($d)* const $name: $ty,]  [$($u)* $name,]  [$($p)*]
            $($rest)*
        }
    };

    // Const generic, last param
    (@strip_generics [$($cb:tt)*] [$($d:tt)*] [$($u:tt)*] [$($p:tt)*]
        const $name:ident : $ty:ty
    ) => {
        $crate::define_dispatch! { @strip_generics [$($cb)*]
            [$($d)* const $name: $ty,]  [$($u)* $name,]  [$($p)*]
        }
    };

    // Type param with trait bound, trailing comma and more
    (@strip_generics [$($cb:tt)*] [$($d:tt)*] [$($u:tt)*] [$($p:tt)*]
        $name:ident : $bound:path , $($rest:tt)*
    ) => {
        $crate::define_dispatch! { @strip_generics [$($cb)*]
            [$($d)* $name: $bound,]  [$($u)* $name,]  [$($p)* $name,]
            $($rest)*
        }
    };

    // Type param with trait bound, last param
    (@strip_generics [$($cb:tt)*] [$($d:tt)*] [$($u:tt)*] [$($p:tt)*]
        $name:ident : $bound:path
    ) => {
        $crate::define_dispatch! { @strip_generics [$($cb)*]
            [$($d)* $name: $bound,]  [$($u)* $name,]  [$($p)* $name,]
        }
    };

    // Bare type param, trailing comma and more
    (@strip_generics [$($cb:tt)*] [$($d:tt)*] [$($u:tt)*] [$($p:tt)*]
        $name:ident , $($rest:tt)*
    ) => {
        $crate::define_dispatch! { @strip_generics [$($cb)*]
            [$($d)* $name,]  [$($u)* $name,]  [$($p)* $name,]
            $($rest)*
        }
    };

    // Bare type param, last param
    (@strip_generics [$($cb:tt)*] [$($d:tt)*] [$($u:tt)*] [$($p:tt)*]
        $name:ident
    ) => {
        $crate::define_dispatch! { @strip_generics [$($cb)*]
            [$($d)* $name,]  [$($u)* $name,]  [$($p)* $name,]
        }
    };

    //////////////////////////////////////////////////////////////////////////////
    // @main: Single unified expansion for both generic and non-generic cases.
    //
    // When decl_gen/use_gen/phantom are empty, `impl<>`, `type Foo<>`, and
    // `PhantomData<()>` are all valid Rust, so this one arm handles everything.
    //////////////////////////////////////////////////////////////////////////////
    (@main
        [
            $app_name:ident;
            $spawn_fn:ident; $tx_impl:ty; $spawn_impl:ty; $context_ty:ty;
            $endpoint_list:path;
            ($($endpoint:ty | $ep_flavor:tt | $ep_handler:ident)*);
            $topic_in_list:path;
            ($($topic_in:ty | $tp_flavor:tt | $tp_handler:ident)*);
            $topic_out_list:path;
        ]
        [$($decl_gen:tt)*]
        [$($use_gen:tt)*]
        [$($phantom:tt)*]
    ) => {
        // Here, we calculate how many bytes (1, 2, 4, or 8) are required to uniquely
        // match on the given messages we receive and send.
        mod sizer {
            use super::*;
            use ::postcard_rpc::Key;

            // Create a list of JUST the REQUEST keys from the endpoint report
            const EP_IN_KEYS_SZ: usize = $endpoint_list.endpoints.len();
            const EP_IN_KEYS: [Key; EP_IN_KEYS_SZ] = const {
                let mut keys = [unsafe { Key::from_bytes([0; 8]) }; EP_IN_KEYS_SZ];
                let mut i = 0;
                while i < EP_IN_KEYS_SZ {
                    keys[i] = $endpoint_list.endpoints[i].1;
                    i += 1;
                }
                keys
            };
            // Create a list of JUST the RESPONSE keys from the endpoint report
            const EP_OUT_KEYS_SZ: usize = $endpoint_list.endpoints.len();
            const EP_OUT_KEYS: [Key; EP_OUT_KEYS_SZ] = const {
                let mut keys = [unsafe { Key::from_bytes([0; 8]) }; EP_OUT_KEYS_SZ];
                let mut i = 0;
                while i < EP_OUT_KEYS_SZ {
                    keys[i] = $endpoint_list.endpoints[i].2;
                    i += 1;
                }
                keys
            };
            // Create a list of JUST the MESSAGE keys from the TOPICS IN report
            const TP_IN_KEYS_SZ: usize = $topic_in_list.topics.len();
            const TP_IN_KEYS: [Key; TP_IN_KEYS_SZ] = const {
                let mut keys = [unsafe { Key::from_bytes([0; 8]) }; TP_IN_KEYS_SZ];
                let mut i = 0;
                while i < TP_IN_KEYS_SZ {
                    keys[i] = $topic_in_list.topics[i].1;
                    i += 1;
                }
                keys
            };
            // Create a list of JUST the MESSAGE keys from the TOPICS OUT report
            const TP_OUT_KEYS_SZ: usize = $topic_out_list.topics.len();
            const TP_OUT_KEYS: [Key; TP_OUT_KEYS_SZ] = const {
                let mut keys = [unsafe { Key::from_bytes([0; 8]) }; TP_OUT_KEYS_SZ];
                let mut i = 0;
                while i < TP_OUT_KEYS_SZ {
                    keys[i] = $topic_out_list.topics[i].1;
                    i += 1;
                }
                keys
            };

            // This is a list of all REQUEST KEYS in the actual handlers
            //
            // This should be a SUBSET of the REQUEST KEYS in the Endpoint report
            const EP_HANDLER_IN_KEYS: &[Key] = &[
                $(<$endpoint as ::postcard_rpc::Endpoint>::REQ_KEY,)*
            ];
            // This is a list of all RESPONSE KEYS in the actual handlers
            //
            // This should be a SUBSET of the RESPONSE KEYS in the Endpoint report
            const EP_HANDLER_OUT_KEYS: &[Key] = &[
                $(<$endpoint as ::postcard_rpc::Endpoint>::RESP_KEY,)*
            ];
            // This is a list of all TOPIC KEYS in the actual handlers
            //
            // This should be a SUBSET of the TOPIC KEYS in the Topic IN report
            const TP_HANDLER_IN_KEYS: &[Key] = &[
                $(<$topic_in as ::postcard_rpc::Topic>::TOPIC_KEY,)*
            ];

            const fn a_is_subset_of_b(a: &[Key], b: &[Key]) -> bool {
                let mut i = 0;
                while i < a.len() {
                    let x = u64::from_le_bytes(a[i].to_bytes());
                    let mut matched = false;
                    let mut j = 0;
                    while j < b.len() {
                        let y = u64::from_le_bytes(b[j].to_bytes());
                        if x == y {
                            matched = true;
                            break;
                        }
                        j += 1;
                    }
                    if !matched {
                        return false;
                    }
                    i += 1;
                }
                true
            }

            pub const NEEDED_SZ_IN: usize = ::postcard_rpc::server::min_key_needed(&[
                &EP_IN_KEYS,
                &TP_IN_KEYS,
            ]);
            pub const NEEDED_SZ_OUT: usize = ::postcard_rpc::server::min_key_needed(&[
                &EP_OUT_KEYS,
                &TP_OUT_KEYS,
            ]);
            pub const NEEDED_SZ: usize = const {
                core::assert!(
                    a_is_subset_of_b(EP_HANDLER_IN_KEYS, &EP_IN_KEYS),
                    "All listed endpoint handlers must be listed in endpoints->list! Missing Requst Type found!",
                );
                core::assert!(
                    a_is_subset_of_b(EP_HANDLER_OUT_KEYS, &EP_OUT_KEYS),
                    "All listed endpoint handlers must be listed in endpoints->list! Missing Response Type found!",
                );
                core::assert!(
                    a_is_subset_of_b(TP_HANDLER_IN_KEYS, &TP_IN_KEYS),
                    "All listed endpoint handlers must be listed in endpoints->list! Missing Response Type found!",
                );
                if NEEDED_SZ_IN > NEEDED_SZ_OUT {
                    NEEDED_SZ_IN
                } else {
                    NEEDED_SZ_OUT
                }
            };

            // Duplicate check: build array of all dispatch keys (standard ICD + user
            // endpoints + topics) and verify no collisions at const-time.
            const PING_KEY: Key = <::postcard_rpc::standard_icd::PingEndpoint as ::postcard_rpc::Endpoint>::REQ_KEY;
            const SCHEMA_KEY: Key = <::postcard_rpc::standard_icd::GetAllSchemasEndpoint as ::postcard_rpc::Endpoint>::REQ_KEY;

            const ALL_DISPATCH_KEYS_LEN: usize = 2 $(+ { let _ = <$endpoint as ::postcard_rpc::Endpoint>::REQ_KEY; 1 })* $(+ { let _ = <$topic_in as ::postcard_rpc::Topic>::TOPIC_KEY; 1 })*;
            const ALL_DISPATCH_KEYS: [Key; ALL_DISPATCH_KEYS_LEN] = [
                PING_KEY,
                SCHEMA_KEY,
                $(<$endpoint as ::postcard_rpc::Endpoint>::REQ_KEY,)*
                $(<$topic_in as ::postcard_rpc::Topic>::TOPIC_KEY,)*
            ];
            const _: () = const {
                let len = ALL_DISPATCH_KEYS.len();
                let mut i = 0;
                while i < len {
                    let mut j = i + 1;
                    while j < len {
                        let a = u64::from_le_bytes(ALL_DISPATCH_KEYS[i].to_bytes());
                        let b = u64::from_le_bytes(ALL_DISPATCH_KEYS[j].to_bytes());
                        core::assert!(a != b, "Caught duplicate items. Is `omit_std` set? This is likely a bug in your code. See https://github.com/jamesmunns/postcard-rpc/issues/135.");
                        j += 1;
                    }
                    i += 1;
                }
            };
        }

        // This is the fun part.
        //
        // For... reasons, we need to generate a match function to allow for dispatching
        // different async handlers without degrading to dyn Future, because no alloc on
        // embedded systems.
        //
        // The easiest way I've found to achieve this is actually to implement this
        // handler for ALL of 1, 2, 4, 8, BUT to hide that from the user, and instead
        // use THIS alias to give them the one that they need.
        //
        // This is overly complicated because I'm mixing const-time capabilities with
        // macro-time capabilities. I'm very open to other suggestions that achieve the
        // same outcome.
        #[doc=concat!("This defines the postcard-rpc app implementation for ", stringify!($app_name))]
        pub type $app_name<$($decl_gen)*> = impls::$app_name<$($use_gen)* { sizer::NEEDED_SZ }>;

        mod impls {
            use super::*;

            pub struct $app_name<$($decl_gen)* const N: usize> {
                pub context: $context_ty,
                pub spawn: $spawn_impl,
                pub device_map: &'static ::postcard_rpc::DeviceMap,
                _phantom: core::marker::PhantomData<($($phantom)*)>,
            }

            impl<$($decl_gen)* const N: usize> $app_name<$($use_gen)* N> {
                /// Create a new instance of the dispatcher
                pub fn new(
                    context: $context_ty,
                    spawn: $spawn_impl,
                ) -> Self {
                    const MAP: &::postcard_rpc::DeviceMap = &::postcard_rpc::DeviceMap {
                        types: const {
                            const LISTS: &[&[&'static ::postcard_rpc::postcard_schema::schema::NamedType]] = &[
                                $endpoint_list.types,
                                $topic_in_list.types,
                                $topic_out_list.types,
                            ];
                            const TTL_COUNT: usize = $endpoint_list.types.len() + $topic_in_list.types.len() + $topic_out_list.types.len();

                            const BIG_RPT: ([Option<&'static ::postcard_rpc::postcard_schema::schema::NamedType>; TTL_COUNT], usize) = ::postcard_rpc::uniques::merge_nty_lists(LISTS);
                            const SMALL_RPT: [&'static ::postcard_rpc::postcard_schema::schema::NamedType; BIG_RPT.1] = ::postcard_rpc::uniques::cruncher(BIG_RPT.0.as_slice());
                            SMALL_RPT.as_slice()
                        },
                        endpoints: &$endpoint_list.endpoints,
                        topics_in: &$topic_in_list.topics,
                        topics_out: &$topic_out_list.topics,
                        min_key_len: const {
                            match sizer::NEEDED_SZ {
                                1 => ::postcard_rpc::header::VarKeyKind::Key1,
                                2 => ::postcard_rpc::header::VarKeyKind::Key2,
                                4 => ::postcard_rpc::header::VarKeyKind::Key4,
                                8 => ::postcard_rpc::header::VarKeyKind::Key8,
                                _ => core::unreachable!(),
                            }
                        }
                    };
                    $app_name {
                        context,
                        spawn,
                        device_map: MAP,
                        _phantom: core::marker::PhantomData,
                    }
                }
            }

            $crate::define_dispatch! {
                @matcher [$($decl_gen)*] [$($use_gen)*]
                1 $app_name $tx_impl; $spawn_fn ::postcard_rpc::Key1; ::postcard_rpc::header::VarKeyKind::Key1;
                REQ_KEY1 / TOPIC_KEY1 = u8;
                ($($endpoint | $ep_flavor | $ep_handler)*)
                ($($topic_in | $tp_flavor | $tp_handler)*)
            }
            $crate::define_dispatch! {
                @matcher [$($decl_gen)*] [$($use_gen)*]
                2 $app_name $tx_impl; $spawn_fn ::postcard_rpc::Key2; ::postcard_rpc::header::VarKeyKind::Key2;
                REQ_KEY2 / TOPIC_KEY2 = [u8; 2];
                ($($endpoint | $ep_flavor | $ep_handler)*)
                ($($topic_in | $tp_flavor | $tp_handler)*)
            }
            $crate::define_dispatch! {
                @matcher [$($decl_gen)*] [$($use_gen)*]
                4 $app_name $tx_impl; $spawn_fn ::postcard_rpc::Key4; ::postcard_rpc::header::VarKeyKind::Key4;
                REQ_KEY4 / TOPIC_KEY4 = [u8; 4];
                ($($endpoint | $ep_flavor | $ep_handler)*)
                ($($topic_in | $tp_flavor | $tp_handler)*)
            }
            $crate::define_dispatch! {
                @matcher [$($decl_gen)*] [$($use_gen)*]
                8 $app_name $tx_impl; $spawn_fn ::postcard_rpc::Key; ::postcard_rpc::header::VarKeyKind::Key8;
                REQ_KEY / TOPIC_KEY = [u8; 8];
                ($($endpoint | $ep_flavor | $ep_handler)*)
                ($($topic_in | $tp_flavor | $tp_handler)*)
            }
        }

    };

    //////////////////////////////////////////////////////////////////////////////
    // ENTRY POINTS
    //
    // All entry points pack their fields into a callback group and delegate to
    // @main (via @strip_generics for the generic variant).
    //////////////////////////////////////////////////////////////////////////////

    // ---- With generics, WITH spawn ----
    (
        app: $app_name:ident [$($raw_gen:tt)*];

        spawn_fn: $spawn_fn:ident;
        tx_impl: $tx_impl:ty;
        spawn_impl: $spawn_impl:ty;
        context: $context_ty:ty;

        endpoints: {
            list: $endpoint_list:path;

               | EndpointTy     | kind          | handler           |
               | $(-)*          | $(-)*         | $(-)*             |
            $( | $endpoint:ty   | $ep_flavor:tt | $ep_handler:ident  | )*
        };
        topics_in: {
            list: $topic_in_list:path;

               | TopicTy        | kind          | handler           |
               | $(-)*          | $(-)*         | $(-)*             |
            $( | $topic_in:ty   | $tp_flavor:tt | $tp_handler:ident  | )*
        };
        topics_out: {
            list: $topic_out_list:path;
        };
    ) => {
        $crate::define_dispatch! {
            @strip_generics
            [
                $app_name;
                $spawn_fn; $tx_impl; $spawn_impl; $context_ty;
                $endpoint_list;
                ($($endpoint | $ep_flavor | $ep_handler)*);
                $topic_in_list;
                ($($topic_in | $tp_flavor | $tp_handler)*);
                $topic_out_list;
            ]
            [] [] []
            $($raw_gen)*
        }
    };

    // ---- With generics, WITH spawn, empty topics_in ----
    (
        app: $app_name:ident [$($raw_gen:tt)*];

        spawn_fn: $spawn_fn:ident;
        tx_impl: $tx_impl:ty;
        spawn_impl: $spawn_impl:ty;
        context: $context_ty:ty;

        endpoints: {
            list: $endpoint_list:path;

               | EndpointTy     | kind          | handler           |
               | $(-)*          | $(-)*         | $(-)*             |
            $( | $endpoint:ty   | $ep_flavor:tt | $ep_handler:ident  | )*
        };
        topics_in: {
            list: $topic_in_list:path;
        };
        topics_out: {
            list: $topic_out_list:path;
        };
    ) => {
        $crate::define_dispatch! {
            @strip_generics
            [
                $app_name;
                $spawn_fn; $tx_impl; $spawn_impl; $context_ty;
                $endpoint_list;
                ($($endpoint | $ep_flavor | $ep_handler)*);
                $topic_in_list;
                ();
                $topic_out_list;
            ]
            [] [] []
            $($raw_gen)*
        }
    };

    // ---- With generics, NO spawn (defaults to NoSpawn/no_spawn) ----
    (
        app: $app_name:ident [$($raw_gen:tt)*];

        tx_impl: $tx_impl:ty;
        context: $context_ty:ty;

        endpoints: {
            list: $endpoint_list:path;

               | EndpointTy     | kind          | handler           |
               | $(-)*          | $(-)*         | $(-)*             |
            $( | $endpoint:ty   | $ep_flavor:tt | $ep_handler:ident  | )*
        };
        topics_in: {
            list: $topic_in_list:path;

               | TopicTy        | kind          | handler           |
               | $(-)*          | $(-)*         | $(-)*             |
            $( | $topic_in:ty   | $tp_flavor:tt | $tp_handler:ident  | )*
        };
        topics_out: {
            list: $topic_out_list:path;
        };
    ) => {
        $crate::define_dispatch! {
            @strip_generics
            [
                $app_name;
                no_spawn; $tx_impl; $crate::host::protocol::NoSpawn; $context_ty;
                $endpoint_list;
                ($($endpoint | $ep_flavor | $ep_handler)*);
                $topic_in_list;
                ($($topic_in | $tp_flavor | $tp_handler)*);
                $topic_out_list;
            ]
            [] [] []
            $($raw_gen)*
        }
    };

    // ---- With generics, NO spawn, empty topics_in ----
    (
        app: $app_name:ident [$($raw_gen:tt)*];

        tx_impl: $tx_impl:ty;
        context: $context_ty:ty;

        endpoints: {
            list: $endpoint_list:path;

               | EndpointTy     | kind          | handler           |
               | $(-)*          | $(-)*         | $(-)*             |
            $( | $endpoint:ty   | $ep_flavor:tt | $ep_handler:ident  | )*
        };
        topics_in: {
            list: $topic_in_list:path;
        };
        topics_out: {
            list: $topic_out_list:path;
        };
    ) => {
        $crate::define_dispatch! {
            @strip_generics
            [
                $app_name;
                no_spawn; $tx_impl; $crate::host::protocol::NoSpawn; $context_ty;
                $endpoint_list;
                ($($endpoint | $ep_flavor | $ep_handler)*);
                $topic_in_list;
                ();
                $topic_out_list;
            ]
            [] [] []
            $($raw_gen)*
        }
    };

    // ---- Without generics, WITH spawn (original syntax) ----
    (
        app: $app_name:ident;

        spawn_fn: $spawn_fn:ident;
        tx_impl: $tx_impl:ty;
        spawn_impl: $spawn_impl:ty;
        context: $context_ty:ty;

        endpoints: {
            list: $endpoint_list:path;

               | EndpointTy     | kind          | handler           |
               | $(-)*          | $(-)*         | $(-)*             |
            $( | $endpoint:ty   | $ep_flavor:tt | $ep_handler:ident  | )*
        };
        topics_in: {
            list: $topic_in_list:path;

               | TopicTy        | kind          | handler           |
               | $(-)*          | $(-)*         | $(-)*             |
            $( | $topic_in:ty   | $tp_flavor:tt | $tp_handler:ident  | )*
        };
        topics_out: {
            list: $topic_out_list:path;
        };
    ) => {
        $crate::define_dispatch! {
            @main
            [
                $app_name;
                $spawn_fn; $tx_impl; $spawn_impl; $context_ty;
                $endpoint_list;
                ($($endpoint | $ep_flavor | $ep_handler)*);
                $topic_in_list;
                ($($topic_in | $tp_flavor | $tp_handler)*);
                $topic_out_list;
            ]
            [] [] []
        }
    };

    // ---- Without generics, NO spawn ----
    (
        app: $app_name:ident;

        tx_impl: $tx_impl:ty;
        context: $context_ty:ty;

        endpoints: {
            list: $endpoint_list:path;

               | EndpointTy     | kind          | handler           |
               | $(-)*          | $(-)*         | $(-)*             |
            $( | $endpoint:ty   | $ep_flavor:tt | $ep_handler:ident  | )*
        };
        topics_in: {
            list: $topic_in_list:path;

               | TopicTy        | kind          | handler           |
               | $(-)*          | $(-)*         | $(-)*             |
            $( | $topic_in:ty   | $tp_flavor:tt | $tp_handler:ident  | )*
        };
        topics_out: {
            list: $topic_out_list:path;
        };
    ) => {
        $crate::define_dispatch! {
            @main
            [
                $app_name;
                no_spawn; $tx_impl; $crate::host::protocol::NoSpawn; $context_ty;
                $endpoint_list;
                ($($endpoint | $ep_flavor | $ep_handler)*);
                $topic_in_list;
                ($($topic_in | $tp_flavor | $tp_handler)*);
                $topic_out_list;
            ]
            [] [] []
        }
    };
}
