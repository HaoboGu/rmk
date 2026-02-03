use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::parse::Parser;
use syn::{Attribute, DeriveInput, Meta, parse_macro_input};

use crate::input::runnable::{has_derive, reconstruct_type_def, to_upper_snake_case};

/// Generates controller event infrastructure.
///
/// This macro can be combined with `#[input_event]` on the same struct to create
/// a dual-channel event type that supports both input and controller event patterns.
/// The order of the two macros does not matter.
///
/// See `rmk::event::ControllerEvent` for usage.
pub fn controller_event_impl(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    // Parse attributes
    let (channel_size, subs, pubs) = if attr.is_empty() {
        (None, None, None)
    } else {
        parse_controller_event_attributes(attr)
    };

    // Validate input is a struct or enum
    if !matches!(input.data, syn::Data::Struct(_) | syn::Data::Enum(_)) {
        return syn::Error::new_spanned(input, "#[controller_event] can only be applied to structs or enums")
            .to_compile_error()
            .into();
    }

    // Verify Clone + Copy derives
    if !has_derive(&input.attrs, "Clone") || !has_derive(&input.attrs, "Copy") {
        return syn::Error::new_spanned(
            input,
            "#[controller_event] requires the struct to derive Clone and Copy",
        )
        .to_compile_error()
        .into();
    }

    // Check if input_event macro is also present and extract its parameters
    let input_event_attr = input.attrs.iter().find(|attr| attr.path().is_ident("input_event"));

    let type_name = &input.ident;
    let vis = &input.vis;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Generate ControllerEvent channel and trait implementations
    let controller_channel_name = syn::Ident::new(
        &format!("{}_CONTROLLER_CHANNEL", to_upper_snake_case(&type_name.to_string())),
        type_name.span(),
    );

    let cap = channel_size.unwrap_or_else(|| quote::quote! { 1 });
    let subs_val = subs.unwrap_or_else(|| quote::quote! { 4 });
    let pubs_val = pubs.unwrap_or_else(|| quote::quote! { 1 });

    let controller_channel_static = quote! {
        static #controller_channel_name: ::embassy_sync::pubsub::PubSubChannel<
            ::rmk::RawMutex,
            #type_name #ty_generics,
            { #cap },
            { #subs_val },
            { #pubs_val }
        > = ::embassy_sync::pubsub::PubSubChannel::new();
    };

    let controller_event_impl = quote! {
        impl #impl_generics ::rmk::event::ControllerEvent for #type_name #ty_generics #where_clause {
            type Publisher = ::embassy_sync::pubsub::ImmediatePublisher<
                'static,
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #cap },
                { #subs_val },
                { #pubs_val }
            >;
            type Subscriber = ::embassy_sync::pubsub::Subscriber<
                'static,
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #cap },
                { #subs_val },
                { #pubs_val }
            >;

            fn controller_publisher() -> Self::Publisher {
                #controller_channel_name.immediate_publisher()
            }

            fn controller_subscriber() -> Self::Subscriber {
                #controller_channel_name.subscriber().unwrap()
            }
        }
    };

    let async_controller_event_impl = quote! {
        impl #impl_generics ::rmk::event::AsyncControllerEvent for #type_name #ty_generics #where_clause {
            type AsyncPublisher = ::embassy_sync::pubsub::Publisher<
                'static,
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #cap },
                { #subs_val },
                { #pubs_val }
            >;

            fn controller_publisher_async() -> Self::AsyncPublisher {
                #controller_channel_name.publisher().unwrap()
            }
        }
    };

    // Filter out both macros from attributes for the final struct definition
    let filtered_attrs: Vec<TokenStream> = input
        .attrs
        .iter()
        .filter(|attr| !attr.path().is_ident("input_event") && !attr.path().is_ident("controller_event"))
        .map(|attr| attr.to_token_stream())
        .collect();

    // Reconstruct the type definition (struct or enum)
    let type_def = reconstruct_type_def(&input);

    let expanded = if let Some(input_attr) = input_event_attr {
        // input_event is also present, generate both sets of implementations
        let input_channel_size = parse_input_event_attr_from_attribute(input_attr);

        let input_channel_name = syn::Ident::new(
            &format!("{}_INPUT_CHANNEL", to_upper_snake_case(&type_name.to_string())),
            type_name.span(),
        );

        let input_cap = input_channel_size.unwrap_or_else(|| quote::quote! { 8 });

        let input_channel_static = quote! {
            static #input_channel_name: ::embassy_sync::channel::Channel<
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #input_cap }
            > = ::embassy_sync::channel::Channel::new();
        };

        let input_event_impl = quote! {
            impl #impl_generics ::rmk::event::InputEvent for #type_name #ty_generics #where_clause {
                type Publisher = ::embassy_sync::channel::Sender<
                    'static,
                    ::rmk::RawMutex,
                    #type_name #ty_generics,
                    { #input_cap }
                >;
                type Subscriber = ::embassy_sync::channel::Receiver<
                    'static,
                    ::rmk::RawMutex,
                    #type_name #ty_generics,
                    { #input_cap }
                >;

                fn input_publisher() -> Self::Publisher {
                    #input_channel_name.sender()
                }

                fn input_subscriber() -> Self::Subscriber {
                    #input_channel_name.receiver()
                }
            }
        };

        let async_input_event_impl = quote! {
            impl #impl_generics ::rmk::event::AsyncInputEvent for #type_name #ty_generics #where_clause {
                type AsyncPublisher = ::embassy_sync::channel::Sender<
                    'static,
                    ::rmk::RawMutex,
                    #type_name #ty_generics,
                    { #input_cap }
                >;

                fn input_publisher_async() -> Self::AsyncPublisher {
                    #input_channel_name.sender()
                }
            }
        };

        quote! {
            #(#filtered_attrs)*
            #vis #type_def

            #controller_channel_static

            #controller_event_impl

            #async_controller_event_impl

            #input_channel_static

            #input_event_impl

            #async_input_event_impl
        }
    } else {
        // Only controller_event
        quote! {
            #(#filtered_attrs)*
            #vis #type_def

            #controller_channel_static

            #controller_event_impl

            #async_controller_event_impl
        }
    };

    expanded.into()
}

/// Parse controller_event macro attributes: (channel_size, subs, pubs)
fn parse_controller_event_attributes(
    attr: proc_macro::TokenStream,
) -> (
    Option<proc_macro2::TokenStream>,
    Option<proc_macro2::TokenStream>,
    Option<proc_macro2::TokenStream>,
) {
    use syn::Token;
    use syn::punctuated::Punctuated;

    let mut channel_size = None;
    let mut subs = None;
    let mut pubs = None;

    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let attr2: proc_macro2::TokenStream = attr.into();

    match parser.parse2(attr2) {
        Ok(parsed) => {
            for meta in parsed {
                if let Meta::NameValue(nv) = meta {
                    if nv.path.is_ident("channel_size") {
                        let expr = &nv.value;
                        channel_size = Some(quote::quote! { #expr });
                    } else if nv.path.is_ident("subs") {
                        let expr = &nv.value;
                        subs = Some(quote::quote! { #expr });
                    } else if nv.path.is_ident("pubs") {
                        let expr = &nv.value;
                        pubs = Some(quote::quote! { #expr });
                    }
                }
            }
        }
        Err(e) => {
            panic!("Failed to parse controller_event attributes: {}", e);
        }
    }

    (channel_size, subs, pubs)
}

/// Parse input_event parameters from an Attribute
fn parse_input_event_attr_from_attribute(attr: &Attribute) -> Option<proc_macro2::TokenStream> {
    use syn::Token;
    use syn::punctuated::Punctuated;

    let mut channel_size = None;

    if let Meta::List(meta_list) = &attr.meta {
        let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
        if let Ok(parsed) = parser.parse2(meta_list.tokens.clone()) {
            for meta in parsed {
                if let Meta::NameValue(nv) = meta
                    && nv.path.is_ident("channel_size")
                {
                    let expr = &nv.value;
                    channel_size = Some(quote::quote! { #expr });
                }
            }
        }
    }

    channel_size
}
