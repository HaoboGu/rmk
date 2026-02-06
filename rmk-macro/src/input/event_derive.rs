use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, parse_macro_input};

/// Derive `InputPublishEvent`/`AsyncInputPublishEvent` for wrapper enums.
///
/// Generates:
/// - publisher type for the enum (routes to individual event channels)
/// - `InputPublishEvent`/`AsyncInputPublishEvent` impls
/// - `From<Variant>` impls for each variant
///
/// **Note**: Wrapper enums only implement publish traits, not subscribe traits.
/// This is because wrapper enums route events to their concrete type channels,
/// and you should subscribe to the individual event types instead.
///
/// # Example
///
/// ```rust,ignore
/// #[derive(InputEvent)]
/// pub enum MultiSensorEvent {
///     Battery(BatteryEvent),
///     Pointing(PointingEvent),
/// }
///
/// // Usage:
/// publish_input_event_async(MultiSensorEvent::Battery(event)).await;
/// ```
pub fn input_event_derive_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Ensure input is an enum.
    let data_enum = match &input.data {
        syn::Data::Enum(e) => e,
        _ => {
            return syn::Error::new_spanned(input, "#[derive(InputEvent)] can only be applied to enums")
                .to_compile_error()
                .into();
        }
    };

    let enum_name = &input.ident;
    let publisher_name = format_ident!("{}Publisher", enum_name);

    // Split generics for impl blocks.
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Collect variant info.
    let mut async_publish_arms = Vec::new();
    let mut publish_arms = Vec::new();
    let mut from_impls = Vec::new();

    for variant in &data_enum.variants {
        let variant_name = &variant.ident;

        // Require a single-field tuple variant.
        let inner_type = match &variant.fields {
            syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => &fields.unnamed.first().unwrap().ty,
            _ => {
                return syn::Error::new_spanned(
                    variant,
                    "Each variant must be a tuple variant with exactly one field, e.g., `Battery(BatteryEvent)`",
                )
                .to_compile_error()
                .into();
            }
        };

        // Sync publish arm.
        publish_arms.push(quote! {
            #enum_name::#variant_name(e) => ::rmk::event::publish_input_event(e)
        });

        // Async publish arm.
        async_publish_arms.push(quote! {
            #enum_name::#variant_name(e) => ::rmk::event::publish_input_event_async(e).await
        });

        // From impls (with generics).
        from_impls.push(quote! {
            impl #impl_generics From<#inner_type> for #enum_name #ty_generics #where_clause {
                fn from(e: #inner_type) -> Self {
                    #enum_name::#variant_name(e)
                }
            }
        });
    }

    let expanded = quote! {
        /// Publisher for the wrapper enum.
        /// Routes each variant to its event channel.
        pub struct #publisher_name;

        impl ::rmk::event::AsyncEventPublisher for #publisher_name {
            type Event = #enum_name #ty_generics;

            async fn publish_async(&self, event: #enum_name #ty_generics) {
                match event {
                    #(#async_publish_arms),*
                }
            }
        }

        impl ::rmk::event::EventPublisher for #publisher_name {
            type Event = #enum_name #ty_generics;

            fn publish(&self, event: #enum_name #ty_generics) {
                match event {
                    #(#publish_arms),*
                }
            }
        }

        impl #impl_generics ::rmk::event::InputPublishEvent for #enum_name #ty_generics #where_clause {
            type Publisher = #publisher_name;

            fn input_publisher() -> Self::Publisher {
                #publisher_name
            }
        }

        impl #impl_generics ::rmk::event::AsyncInputPublishEvent for #enum_name #ty_generics #where_clause {
            type AsyncPublisher = #publisher_name;

            fn input_publisher_async() -> Self::AsyncPublisher {
                #publisher_name
            }
        }

        #(#from_impls)*
    };

    expanded.into()
}
