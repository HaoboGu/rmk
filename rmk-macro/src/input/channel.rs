//! Input event channel generation.
//!
//! Generates static channels and trait implementations for input events.

use proc_macro2::TokenStream;
use quote::quote;

use crate::utils::{has_derive, to_upper_snake_case};

/// Generate input event channel (Channel) and trait implementations.
///
/// Returns (channel_static, trait_impls) TokenStreams.
pub fn generate_input_event_channel(
    type_name: &syn::Ident,
    ty_generics: &syn::TypeGenerics,
    impl_generics: &syn::ImplGenerics,
    where_clause: Option<&syn::WhereClause>,
    channel_size: Option<TokenStream>,
) -> (TokenStream, TokenStream) {
    let channel_name = syn::Ident::new(
        &format!("{}_INPUT_CHANNEL", to_upper_snake_case(&type_name.to_string())),
        type_name.span(),
    );

    let cap = channel_size.unwrap_or_else(|| quote! { 8 });

    let channel_static = quote! {
        #[doc(hidden)]
        static #channel_name: ::embassy_sync::channel::Channel<
            ::rmk::RawMutex,
            #type_name #ty_generics,
            { #cap }
        > = ::embassy_sync::channel::Channel::new();
    };

    let trait_impls = quote! {
        impl #impl_generics ::rmk::event::InputPublishEvent for #type_name #ty_generics #where_clause {
            type Publisher = ::embassy_sync::channel::Sender<
                'static,
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #cap }
            >;

            fn input_publisher() -> Self::Publisher {
                #channel_name.sender()
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
                #channel_name.receiver()
            }
        }

        impl #impl_generics ::rmk::event::AsyncInputPublishEvent for #type_name #ty_generics #where_clause {
            type AsyncPublisher = ::embassy_sync::channel::Sender<
                'static,
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #cap }
            >;

            fn input_publisher_async() -> Self::AsyncPublisher {
                #channel_name.sender()
            }
        }
    };

    (channel_static, trait_impls)
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
            syn::Error::new_spanned(input, format!("#[{}] requires the struct to derive Clone", macro_name))
                .to_compile_error(),
        );
    }

    None
}
