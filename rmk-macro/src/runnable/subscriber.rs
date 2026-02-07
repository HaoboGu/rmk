//! EventSubscriber generation for aggregated event handling.
//!
//! Used by both Controller and InputProcessor macros.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Path;

use super::naming::event_type_to_handler_method_name;

/// Generate EventSubscriber struct and its implementation.
///
/// This is a unified generator for both Controller and InputProcessor macros.
/// It generates:
/// - A subscriber struct that holds individual event subscribers
/// - `EventSubscriber` impl with `select_biased!` for event aggregation
/// - The corresponding event trait impl (`ControllerSubscribeEvent` or `InputSubscribeEvent`)
///
/// # Parameters
/// - `subscribe_trait_path`: The trait path (e.g., `::rmk::event::ControllerSubscribeEvent`)
/// - `subscriber_method`: The method name to call (e.g., `controller_subscriber`)
pub fn generate_event_subscriber(
    subscriber_name: &syn::Ident,
    event_types: &[Path],
    variant_names: &[syn::Ident],
    enum_name: &syn::Ident,
    vis: &syn::Visibility,
    subscribe_trait_path: TokenStream,
    subscriber_method: TokenStream,
) -> TokenStream {
    let num_events = event_types.len();

    // Subscriber field names
    let sub_fields: Vec<_> = (0..num_events).map(|i| format_ident!("sub{}", i)).collect();

    // Struct field definitions with types
    let field_defs: Vec<_> = event_types
        .iter()
        .zip(&sub_fields)
        .map(|(event_type, field_name)| {
            quote! {
                #field_name: <#event_type as #subscribe_trait_path>::Subscriber
            }
        })
        .collect();

    // Field initializations in new()
    let field_inits: Vec<_> = event_types
        .iter()
        .zip(&sub_fields)
        .map(|(event_type, field_name)| {
            quote! {
                #field_name: <#event_type as #subscribe_trait_path>::#subscriber_method()
            }
        })
        .collect();

    // select_biased! arms for next_event
    let select_arms: Vec<_> = sub_fields
        .iter()
        .zip(variant_names)
        .map(|(field_name, variant_name)| {
            quote! {
                event = self.#field_name.next_event().fuse() => #enum_name::#variant_name(event),
            }
        })
        .collect();

    quote! {
        /// Event subscriber for aggregated events
        #vis struct #subscriber_name {
            #(#field_defs),*
        }

        impl #subscriber_name {
            /// Create a new event subscriber
            pub fn new() -> Self {
                Self {
                    #(#field_inits),*
                }
            }
        }

        impl ::rmk::event::EventSubscriber for #subscriber_name {
            type Event = #enum_name;

            async fn next_event(&mut self) -> Self::Event {
                use ::rmk::event::EventSubscriber;
                use ::rmk::futures::FutureExt;

                ::rmk::futures::select_biased! {
                    #(#select_arms)*
                }
            }
        }

        impl #subscribe_trait_path for #enum_name {
            type Subscriber = #subscriber_name;

            fn #subscriber_method() -> Self::Subscriber {
                #subscriber_name::new()
            }
        }
    }
}

/// Generate process_event/process match arms.
///
/// This is used by both Controller and InputProcessor macros.
pub fn generate_event_match_arms(
    event_types: &[Path],
    variant_names: &[syn::Ident],
    enum_name: &syn::Ident,
) -> Vec<TokenStream> {
    event_types
        .iter()
        .zip(variant_names)
        .map(|(event_type, variant_name)| {
            let method_name = event_type_to_handler_method_name(event_type);
            quote! {
                #enum_name::#variant_name(event) => self.#method_name(event).await
            }
        })
        .collect()
}
