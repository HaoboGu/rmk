use quote::quote;
use syn::{DeriveInput, parse_macro_input};

use super::runnable::{
    has_derive, parse_controller_event_channel_config_from_attr, parse_input_event_channel_size, to_upper_snake_case,
};

/// Generates input event infrastructure using embassy_sync::channel::Channel.
///
/// This macro can be combined with `#[controller_event]` on the same struct to create
/// a dual-channel event type that supports both input and controller event patterns.
/// The order of the two macros does not matter.
///
/// **Note**: Generic event types are not supported because static channels cannot be generic.
///
/// See `rmk::event::InputEvent` for usage.
pub fn input_event_impl(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(item as DeriveInput);

    // Parse attributes - only channel_size is used for Channel
    let channel_size = parse_input_event_channel_size(proc_macro2::TokenStream::from(attr));

    // Validate input is a struct or enum
    if !matches!(input.data, syn::Data::Struct(_) | syn::Data::Enum(_)) {
        return syn::Error::new_spanned(input, "#[input_event] can only be applied to structs or enums")
            .to_compile_error()
            .into();
    }

    // Reject generic types - static channels cannot be generic
    if !input.generics.params.is_empty() {
        return syn::Error::new_spanned(
            &input.generics,
            "#[input_event] does not support generic types. Static channels cannot be generic.",
        )
        .to_compile_error()
        .into();
    }

    // Verify Clone derive (Send is an auto trait, checked by the compiler)
    if !has_derive(&input.attrs, "Clone") {
        return syn::Error::new_spanned(input, "#[input_event] requires the struct to derive Clone")
            .to_compile_error()
            .into();
    }

    // Check if controller_event macro is also present and extract its parameters
    let controller_event_attr = input
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("controller_event"))
        .cloned();

    let type_name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Generate InputEvent channel and trait implementations
    let input_channel_name = syn::Ident::new(
        &format!("{}_INPUT_CHANNEL", to_upper_snake_case(&type_name.to_string())),
        type_name.span(),
    );

    let cap = channel_size.unwrap_or_else(|| quote! { 8 });

    let input_channel_static = quote! {
        #[doc(hidden)]
        static #input_channel_name: ::embassy_sync::channel::Channel<
            ::rmk::RawMutex,
            #type_name #ty_generics,
            { #cap }
        > = ::embassy_sync::channel::Channel::new();
    };

    let input_event_impl = quote! {
        impl #impl_generics ::rmk::event::InputPublishEvent for #type_name #ty_generics #where_clause {
            type Publisher = ::embassy_sync::channel::Sender<
                'static,
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #cap }
            >;

            fn input_publisher() -> Self::Publisher {
                #input_channel_name.sender()
            }
        }

        impl #impl_generics ::rmk::event::InputSubscribeEvent for #type_name #ty_generics #where_clause {
            type Subscriber = ::embassy_sync::channel::Receiver<
                'static,
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #cap }
            >;

            fn input_subscriber() -> Self::Subscriber {
                #input_channel_name.receiver()
            }
        }
    };

    let async_input_event_impl = quote! {
        impl #impl_generics ::rmk::event::AsyncInputPublishEvent for #type_name #ty_generics #where_clause {
            type AsyncPublisher = ::embassy_sync::channel::Sender<
                'static,
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #cap }
            >;

            fn input_publisher_async() -> Self::AsyncPublisher {
                #input_channel_name.sender()
            }
        }
    };

    // Remove both macros from attributes for the final struct definition.
    input
        .attrs
        .retain(|attr| !attr.path().is_ident("input_event") && !attr.path().is_ident("controller_event"));

    let expanded = if let Some(ctrl_attr) = controller_event_attr.as_ref() {
        // controller_event is also present, generate both sets of implementations
        let ctrl_config = parse_controller_event_channel_config_from_attr(ctrl_attr);

        let controller_channel_name = syn::Ident::new(
            &format!("{}_CONTROLLER_CHANNEL", to_upper_snake_case(&type_name.to_string())),
            type_name.span(),
        );

        let ctrl_cap = ctrl_config.channel_size.unwrap_or_else(|| quote! { 1 });
        let ctrl_subs_val = ctrl_config.subs.unwrap_or_else(|| quote! { 4 });
        let ctrl_pubs_val = ctrl_config.pubs.unwrap_or_else(|| quote! { 1 });

        let controller_channel_static = quote! {
            #[doc(hidden)]
            static #controller_channel_name: ::embassy_sync::pubsub::PubSubChannel<
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #ctrl_cap },
                { #ctrl_subs_val },
                { #ctrl_pubs_val }
            > = ::embassy_sync::pubsub::PubSubChannel::new();
        };

        let controller_event_impl = quote! {
            impl #impl_generics ::rmk::event::ControllerPublishEvent for #type_name #ty_generics #where_clause {
                type Publisher = ::embassy_sync::pubsub::ImmediatePublisher<
                    'static,
                    ::rmk::RawMutex,
                    #type_name #ty_generics,
                    { #ctrl_cap },
                    { #ctrl_subs_val },
                    { #ctrl_pubs_val }
                >;

                fn controller_publisher() -> Self::Publisher {
                    #controller_channel_name.immediate_publisher()
                }
            }

            impl #impl_generics ::rmk::event::ControllerSubscribeEvent for #type_name #ty_generics #where_clause {
                type Subscriber = ::embassy_sync::pubsub::Subscriber<
                    'static,
                    ::rmk::RawMutex,
                    #type_name #ty_generics,
                    { #ctrl_cap },
                    { #ctrl_subs_val },
                    { #ctrl_pubs_val }
                >;

                fn controller_subscriber() -> Self::Subscriber {
                    #controller_channel_name.subscriber().expect(
                        concat!(
                            "Failed to create controller subscriber for ",
                            stringify!(#type_name),
                            ". The 'subs' limit has been exceeded. Increase the 'subs' parameter in #[controller_event(subs = N)]."
                        )
                    )
                }
            }
        };

        let async_controller_event_impl = quote! {
            impl #impl_generics ::rmk::event::AsyncControllerPublishEvent for #type_name #ty_generics #where_clause {
                type AsyncPublisher = ::embassy_sync::pubsub::Publisher<
                    'static,
                    ::rmk::RawMutex,
                    #type_name #ty_generics,
                    { #ctrl_cap },
                    { #ctrl_subs_val },
                    { #ctrl_pubs_val }
                >;

                fn controller_publisher_async() -> Self::AsyncPublisher {
                    #controller_channel_name.publisher().expect(
                        concat!(
                            "Failed to create async controller publisher for ",
                            stringify!(#type_name),
                            ". The 'pubs' limit has been exceeded. Increase the 'pubs' parameter in #[controller_event(pubs = N)]."
                        )
                    )
                }
            }
        };

        quote! {
            #input

            #input_channel_static

            #input_event_impl

            #async_input_event_impl

            #controller_channel_static

            #controller_event_impl

            #async_controller_event_impl
        }
    } else {
        // Only input_event
        quote! {
            #input

            #input_channel_static

            #input_event_impl

            #async_input_event_impl
        }
    };

    expanded.into()
}
