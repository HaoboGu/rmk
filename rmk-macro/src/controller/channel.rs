//! Controller event channel generation.
//!
//! Generates static PubSub channels and trait implementations for controller events.

use proc_macro2::TokenStream;
use quote::quote;

use super::config::ControllerEventChannelConfig;
use crate::utils::to_upper_snake_case;

/// Generate controller event channel (PubSubChannel) and trait implementations.
///
/// Returns (channel_static, trait_impls) TokenStreams.
pub fn generate_controller_event_channel(
    type_name: &syn::Ident,
    ty_generics: &syn::TypeGenerics,
    impl_generics: &syn::ImplGenerics,
    where_clause: Option<&syn::WhereClause>,
    config: &ControllerEventChannelConfig,
) -> (TokenStream, TokenStream) {
    let channel_name = syn::Ident::new(
        &format!(
            "{}_CONTROLLER_CHANNEL",
            to_upper_snake_case(&type_name.to_string())
        ),
        type_name.span(),
    );

    let cap = config.channel_size.clone().unwrap_or_else(|| quote! { 1 });
    let subs_val = config.subs.clone().unwrap_or_else(|| quote! { 4 });
    let pubs_val = config.pubs.clone().unwrap_or_else(|| quote! { 1 });

    let channel_static = quote! {
        #[doc(hidden)]
        static #channel_name: ::embassy_sync::pubsub::PubSubChannel<
            ::rmk::RawMutex,
            #type_name #ty_generics,
            { #cap },
            { #subs_val },
            { #pubs_val }
        > = ::embassy_sync::pubsub::PubSubChannel::new();
    };

    let trait_impls = quote! {
        impl #impl_generics ::rmk::event::PublishableControllerEvent for #type_name #ty_generics #where_clause {
            type Publisher = ::embassy_sync::pubsub::ImmediatePublisher<
                'static,
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #cap },
                { #subs_val },
                { #pubs_val }
            >;

            fn controller_publisher() -> Self::Publisher {
                #channel_name.immediate_publisher()
            }
        }

        impl #impl_generics ::rmk::event::SubscribableControllerEvent for #type_name #ty_generics #where_clause {
            type Subscriber = ::embassy_sync::pubsub::Subscriber<
                'static,
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #cap },
                { #subs_val },
                { #pubs_val }
            >;

            fn controller_subscriber() -> Self::Subscriber {
                #channel_name.subscriber().expect(
                    concat!(
                        "Failed to create controller subscriber for ",
                        stringify!(#type_name),
                        ". The 'subs' limit has been exceeded. Increase the 'subs' parameter in #[controller_event(subs = N)]."
                    )
                )
            }
        }

        impl #impl_generics ::rmk::event::AsyncPublishableControllerEvent for #type_name #ty_generics #where_clause {
            type AsyncPublisher = ::embassy_sync::pubsub::Publisher<
                'static,
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #cap },
                { #subs_val },
                { #pubs_val }
            >;

            fn controller_publisher_async() -> Self::AsyncPublisher {
                #channel_name.publisher().expect(
                    concat!(
                        "Failed to create async controller publisher for ",
                        stringify!(#type_name),
                        ". The 'pubs' limit has been exceeded. Increase the 'pubs' parameter in #[controller_event(pubs = N)]."
                    )
                )
            }
        }
    };

    (channel_static, trait_impls)
}
