//! Unified event macro implementation.
//!
//! Generates static channels and trait implementations for events.
//! Supports both MPSC (Channel) and PubSub (PubSubChannel) modes.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

use crate::codegen::feature::{get_rmk_features, is_feature_enabled};
use crate::event_macros::utils::{AttributeParser, has_derive};
use crate::utils::to_upper_snake_case;

/// Configuration for the unified `#[event]` macro.
pub struct EventConfig {
    /// Buffer size of the channel
    pub channel_size: Option<TokenStream>,
    /// Max subscribers (pubsub only)
    pub subs: Option<TokenStream>,
    /// Max publishers (pubsub only)
    pub pubs: Option<TokenStream>,
    /// Split forwarding: None = not split, Some(None) = auto kind, Some(Some(n)) = explicit kind
    pub split: Option<Option<u16>>,
}

/// Channel type for event macro.
/// Automatically inferred: if `subs` or `pubs` is specified, PubSub is used; otherwise MPSC.
enum ChannelType {
    /// MPSC channel (embassy_sync::channel::Channel) - single consumer
    Mpsc,
    /// PubSub channel (embassy_sync::pubsub::PubSubChannel) - broadcast
    PubSub,
}

/// Parse event config from attribute tokens.
///
/// Channel type is automatically inferred: if `subs` or `pubs` is specified, PubSub is used;
/// otherwise MPSC.
///
/// Returns an error if unknown attribute keys are found.
pub fn parse_event_config(tokens: impl Into<TokenStream>) -> Result<EventConfig, TokenStream> {
    let parser = AttributeParser::new_validated(tokens, &["channel_size", "subs", "pubs", "split"])?;

    // Parse split attribute: `split = N` where N is the kind (0 = auto-hash)
    let split = parser.get_int::<u16>("split")?;
    let split = split.map(|kind| if kind == 0 { None } else { Some(kind) });

    Ok(EventConfig {
        channel_size: parser.get_expr_tokens("channel_size"),
        subs: parser.get_expr_tokens("subs"),
        pubs: parser.get_expr_tokens("pubs"),
        split,
    })
}

/// Generate MPSC channel static and trait implementations.
fn generate_mpsc_channel(
    type_name: &syn::Ident,
    ty_generics: &syn::TypeGenerics,
    impl_generics: &syn::ImplGenerics,
    where_clause: Option<&syn::WhereClause>,
    channel_size: Option<TokenStream>,
) -> (TokenStream, TokenStream) {
    let channel_name = syn::Ident::new(
        &format!(
            "{}_EVENT_CHANNEL",
            to_upper_snake_case(&type_name.to_string())
        ),
        type_name.span(),
    );

    let cap = channel_size.unwrap_or_else(|| quote! { 8 });

    let channel_static = quote! {
        #[doc(hidden)]
        static #channel_name: ::rmk::embassy_sync::channel::Channel<
            ::rmk::RawMutex,
            #type_name #ty_generics,
            { #cap }
        > = ::rmk::embassy_sync::channel::Channel::new();
    };

    let trait_impls = quote! {
        impl #impl_generics ::rmk::event::PublishableEvent for #type_name #ty_generics #where_clause {
            type Publisher = ::rmk::embassy_sync::channel::Sender<
                'static,
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #cap }
            >;

            fn publisher() -> Self::Publisher {
                #channel_name.sender()
            }
        }

        impl #impl_generics ::rmk::event::SubscribableEvent for #type_name #ty_generics #where_clause {
            type Subscriber = ::rmk::embassy_sync::channel::Receiver<
                'static,
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #cap }
            >;

            fn subscriber() -> Self::Subscriber {
                #channel_name.receiver()
            }
        }

        impl #impl_generics ::rmk::event::AsyncPublishableEvent for #type_name #ty_generics #where_clause {
            type AsyncPublisher = ::rmk::embassy_sync::channel::Sender<
                'static,
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #cap }
            >;

            fn publisher_async() -> Self::AsyncPublisher {
                #channel_name.sender()
            }
        }
    };

    (channel_static, trait_impls)
}

/// Generate PubSub channel static and trait implementations.
fn generate_pubsub_channel(
    type_name: &syn::Ident,
    ty_generics: &syn::TypeGenerics,
    impl_generics: &syn::ImplGenerics,
    where_clause: Option<&syn::WhereClause>,
    config: &EventConfig,
) -> (TokenStream, TokenStream) {
    let channel_name = syn::Ident::new(
        &format!(
            "{}_EVENT_CHANNEL",
            to_upper_snake_case(&type_name.to_string())
        ),
        type_name.span(),
    );

    let cap = config.channel_size.clone().unwrap_or_else(|| quote! { 1 });
    let subs_val = config.subs.clone().unwrap_or_else(|| quote! { 4 });
    let pubs_val = config.pubs.clone().unwrap_or_else(|| quote! { 1 });

    let channel_static = quote! {
        #[doc(hidden)]
        static #channel_name: ::rmk::embassy_sync::pubsub::PubSubChannel<
            ::rmk::RawMutex,
            #type_name #ty_generics,
            { #cap },
            { #subs_val },
            { #pubs_val }
        > = ::rmk::embassy_sync::pubsub::PubSubChannel::new();
    };

    let trait_impls = quote! {
        impl #impl_generics ::rmk::event::PublishableEvent for #type_name #ty_generics #where_clause {
            type Publisher = ::rmk::embassy_sync::pubsub::ImmediatePublisher<
                'static,
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #cap },
                { #subs_val },
                { #pubs_val }
            >;

            fn publisher() -> Self::Publisher {
                #channel_name.immediate_publisher()
            }
        }

        impl #impl_generics ::rmk::event::SubscribableEvent for #type_name #ty_generics #where_clause {
            type Subscriber = ::rmk::embassy_sync::pubsub::Subscriber<
                'static,
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #cap },
                { #subs_val },
                { #pubs_val }
            >;

            fn subscriber() -> Self::Subscriber {
                #channel_name.subscriber().expect(
                    concat!(
                        "Failed to create subscriber for ",
                        stringify!(#type_name),
                        ". The 'subs' limit has been exceeded. Increase the 'subs' parameter in #[event(subs = N)]."
                    )
                )
            }
        }

        impl #impl_generics ::rmk::event::AsyncPublishableEvent for #type_name #ty_generics #where_clause {
            type AsyncPublisher = ::rmk::embassy_sync::pubsub::Publisher<
                'static,
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #cap },
                { #subs_val },
                { #pubs_val }
            >;

            fn publisher_async() -> Self::AsyncPublisher {
                #channel_name.publisher().expect(
                    concat!(
                        "Failed to create async publisher for ",
                        stringify!(#type_name),
                        ". The 'pubs' limit has been exceeded. Increase the 'pubs' parameter in #[event(pubs = N)]."
                    )
                )
            }
        }
    };

    (channel_static, trait_impls)
}

/// Compute FNV-1a hash of a string at compile time (returns u16)
fn fnv1a_hash_u16(s: &str) -> u16 {
    let mut hash: u32 = 0x811c9dc5;
    for byte in s.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    hash as u16
}

/// Generate split forwarding code (SplitForwardable impl + linker symbol for collision detection)
fn generate_split_forwarding(
    type_name: &syn::Ident,
    ty_generics: &syn::TypeGenerics,
    impl_generics: &syn::ImplGenerics,
    where_clause: Option<&syn::WhereClause>,
    split_kind: Option<u16>,
) -> TokenStream {
    // Determine the kind value
    let kind_value = split_kind.unwrap_or_else(|| fnv1a_hash_u16(&type_name.to_string()));

    // Generate linker symbol for collision detection
    let symbol_name = syn::Ident::new(
        &format!("__RMK_SPLIT_EVENT_KIND_{}", kind_value),
        type_name.span(),
    );

    quote! {
        // SplitForwardable trait implementation
        impl #impl_generics ::rmk::split::forward::SplitForwardable for #type_name #ty_generics #where_clause {
            const SPLIT_EVENT_KIND: u16 = #kind_value;
        }

        // Linker symbol for compile-time collision detection (zero-cost)
        #[doc(hidden)]
        #[unsafe(no_mangle)]
        pub static #symbol_name: () = ();
    }
}

/// Generate split-aware trait impls (PublishableEvent / SubscribableEvent / AsyncPublishableEvent)
/// that wrap the local channel with split forwarding.
fn generate_split_trait_impls(
    type_name: &syn::Ident,
    ty_generics: &syn::TypeGenerics,
    impl_generics: &syn::ImplGenerics,
    where_clause: Option<&syn::WhereClause>,
    config: &EventConfig,
    channel_type: &ChannelType,
) -> TokenStream {
    let channel_name = syn::Ident::new(
        &format!("{}_EVENT_CHANNEL", to_upper_snake_case(&type_name.to_string())),
        type_name.span(),
    );

    match channel_type {
        ChannelType::Mpsc => {
            let cap = config.channel_size.clone().unwrap_or_else(|| quote! { 8 });
            quote! {
                impl #impl_generics ::rmk::event::PublishableEvent for #type_name #ty_generics #where_clause {
                    type Publisher = ::rmk::split::forward::SplitForwardingPublisher<
                        ::rmk::embassy_sync::channel::Sender<
                            'static, ::rmk::RawMutex, #type_name #ty_generics, { #cap }
                        >
                    >;
                    fn publisher() -> Self::Publisher {
                        ::rmk::split::forward::SplitForwardingPublisher::new(#channel_name.sender())
                    }
                }
                impl #impl_generics ::rmk::event::SubscribableEvent for #type_name #ty_generics #where_clause {
                    type Subscriber = ::rmk::split::forward::SplitAwareSubscriber<
                        ::rmk::embassy_sync::channel::Receiver<
                            'static, ::rmk::RawMutex, #type_name #ty_generics, { #cap }
                        >,
                        #type_name #ty_generics
                    >;
                    fn subscriber() -> Self::Subscriber {
                        ::rmk::split::forward::SplitAwareSubscriber::new(#channel_name.receiver())
                    }
                }
                impl #impl_generics ::rmk::event::AsyncPublishableEvent for #type_name #ty_generics #where_clause {
                    type AsyncPublisher = ::rmk::split::forward::SplitForwardingPublisher<
                        ::rmk::embassy_sync::channel::Sender<
                            'static, ::rmk::RawMutex, #type_name #ty_generics, { #cap }
                        >
                    >;
                    fn publisher_async() -> Self::AsyncPublisher {
                        ::rmk::split::forward::SplitForwardingPublisher::new(#channel_name.sender())
                    }
                }
            }
        }
        ChannelType::PubSub => {
            let cap = config.channel_size.clone().unwrap_or_else(|| quote! { 1 });
            let subs_val = config.subs.clone().unwrap_or_else(|| quote! { 4 });
            let pubs_val = config.pubs.clone().unwrap_or_else(|| quote! { 1 });
            quote! {
                impl #impl_generics ::rmk::event::PublishableEvent for #type_name #ty_generics #where_clause {
                    type Publisher = ::rmk::split::forward::SplitForwardingPublisher<
                        ::rmk::embassy_sync::pubsub::ImmediatePublisher<
                            'static, ::rmk::RawMutex,
                            #type_name #ty_generics,
                            { #cap }, { #subs_val }, { #pubs_val }
                        >
                    >;
                    fn publisher() -> Self::Publisher {
                        ::rmk::split::forward::SplitForwardingPublisher::new(
                            #channel_name.immediate_publisher()
                        )
                    }
                }
                impl #impl_generics ::rmk::event::SubscribableEvent for #type_name #ty_generics #where_clause {
                    type Subscriber = ::rmk::split::forward::SplitAwareSubscriber<
                        ::rmk::embassy_sync::pubsub::Subscriber<
                            'static, ::rmk::RawMutex,
                            #type_name #ty_generics,
                            { #cap }, { #subs_val }, { #pubs_val }
                        >,
                        #type_name #ty_generics
                    >;
                    fn subscriber() -> Self::Subscriber {
                        ::rmk::split::forward::SplitAwareSubscriber::new(
                            #channel_name.subscriber().expect(
                                concat!("Failed to create subscriber for ", stringify!(#type_name))
                            )
                        )
                    }
                }
                impl #impl_generics ::rmk::event::AsyncPublishableEvent for #type_name #ty_generics #where_clause {
                    type AsyncPublisher = ::rmk::split::forward::SplitForwardingPublisher<
                        ::rmk::embassy_sync::pubsub::Publisher<
                            'static, ::rmk::RawMutex,
                            #type_name #ty_generics,
                            { #cap }, { #subs_val }, { #pubs_val }
                        >
                    >;
                    fn publisher_async() -> Self::AsyncPublisher {
                        ::rmk::split::forward::SplitForwardingPublisher::new(
                            #channel_name.publisher().expect(
                                concat!("Failed to create async publisher for ", stringify!(#type_name))
                            )
                        )
                    }
                }
            }
        }
    }
}

/// Implementation of the unified `#[event]` macro.
pub fn event_impl(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    // Parse config
    let config = match parse_event_config(proc_macro2::TokenStream::from(attr)) {
        Ok(config) => config,
        Err(err) => return err.into(),
    };

    // Check split feature if split attribute is present
    if config.split.is_some() {
        let rmk_features = get_rmk_features();
        if !is_feature_enabled(&rmk_features, "split") {
            return quote! {
                compile_error!("#[event(split)] requires the `split` feature to be enabled in the rmk dependency");
            }.into();
        }

        // Validate that the event has required derives for split forwarding
        if !has_derive(&input.attrs, "Serialize") {
            return quote! {
                compile_error!("#[event(split)] requires the event to derive Serialize");
            }.into();
        }
        if !has_derive(&input.attrs, "Deserialize") {
            return quote! {
                compile_error!("#[event(split)] requires the event to derive Deserialize");
            }.into();
        }
        if !has_derive(&input.attrs, "MaxSize") {
            return quote! {
                compile_error!("#[event(split)] requires the event to derive MaxSize (from postcard::experimental::max_size::MaxSize)");
            }.into();
        }
    }

    // Validate event type
    if let Some(error) = validate_event_type(&input, "event") {
        return error.into();
    }

    let type_name = input.ident.clone();
    let generics = input.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Infer channel type: if subs or pubs is specified, use PubSub; otherwise MPSC
    let channel_type = if config.subs.is_some() || config.pubs.is_some() {
        ChannelType::PubSub
    } else {
        ChannelType::Mpsc
    };

    // Generate channel and trait impls based on channel type
    let (channel_static, trait_impls) = match channel_type {
        ChannelType::Mpsc => generate_mpsc_channel(
            &type_name,
            &ty_generics,
            &impl_generics,
            where_clause,
            config.channel_size.clone(),
        ),
        ChannelType::PubSub => generate_pubsub_channel(
            &type_name,
            &ty_generics,
            &impl_generics,
            where_clause,
            &config,
        ),
    };

    // Generate split forwarding code if split attribute is present
    let split_code = if let Some(split_kind) = config.split {
        let split_forwardable = generate_split_forwarding(
            &type_name,
            &ty_generics,
            &impl_generics,
            where_clause,
            split_kind,
        );

        let split_trait_impls = generate_split_trait_impls(
            &type_name,
            &ty_generics,
            &impl_generics,
            where_clause,
            &config,
            &channel_type,
        );

        quote! {
            #split_forwardable
            #split_trait_impls
        }
    } else {
        trait_impls
    };

    let expanded = quote! {
        #input

        #channel_static
        #split_code
    };

    expanded.into()
}

/// Validate event type for event macros.
///
/// Returns an error TokenStream if validation fails, None if valid.
pub fn validate_event_type(input: &syn::DeriveInput, macro_name: &str) -> Option<TokenStream> {
    // Validate input is a struct or enum
    if !matches!(input.data, syn::Data::Struct(_) | syn::Data::Enum(_)) {
        return Some(
            syn::Error::new_spanned(
                input,
                format!("#[{}] can only be applied to structs or enums", macro_name),
            )
            .to_compile_error(),
        );
    }

    // Reject generic types - static channels cannot be generic
    if !input.generics.params.is_empty() {
        return Some(
            syn::Error::new_spanned(
                &input.generics,
                format!(
                    "#[{}] does not support generic types. Static channels cannot be generic.",
                    macro_name
                ),
            )
            .to_compile_error(),
        );
    }

    // Verify Clone derive
    if !has_derive(&input.attrs, "Clone") {
        return Some(
            syn::Error::new_spanned(
                input,
                format!("#[{}] requires the struct to derive Clone", macro_name),
            )
            .to_compile_error(),
        );
    }

    None
}
