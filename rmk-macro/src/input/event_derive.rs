use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, parse_macro_input};

/// Derive `InputEvent`/`AsyncInputEvent` for wrapper enums.
///
/// Generates:
/// - publisher/subscriber types for the enum
/// - `InputEvent`/`AsyncInputEvent` impls
/// - `From<Variant>` impls for each variant
///
/// Each variant is forwarded to its event channel.
/// This keeps `publish_input_event_async(wrapper).await` working.
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
    let subscriber_name = format_ident!("{}Subscriber", enum_name);

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

        impl ::rmk::event::AsyncEventPublisher<#enum_name #ty_generics> for #publisher_name {
            async fn publish_async(&self, event: #enum_name #ty_generics) {
                match event {
                    #(#async_publish_arms),*
                }
            }
        }

        impl ::rmk::event::EventPublisher<#enum_name #ty_generics> for #publisher_name {
            fn publish(&self, event: #enum_name #ty_generics) {
                match event {
                    #(#publish_arms),*
                }
            }
        }

        /// Placeholder subscriber for wrapper enums.
        ///
        /// **Note**: Wrapper enums route events to their concrete type channels.
        /// You cannot subscribe to wrapper enums directly.
        /// Subscribe to the individual event types (e.g., `BatteryEvent`, `PointingEvent`) instead.
        pub struct #subscriber_name;

        impl ::rmk::event::EventSubscriber<#enum_name #ty_generics> for #subscriber_name {
            async fn next_event(&mut self) -> #enum_name #ty_generics {
                unreachable!(
                    "Cannot subscribe to wrapper enum `{}` directly. Subscribe to the concrete event types instead.",
                    stringify!(#enum_name)
                )
            }
        }

        impl #impl_generics ::rmk::event::InputEvent for #enum_name #ty_generics #where_clause {
            type Publisher = #publisher_name;
            type Subscriber = #subscriber_name;

            fn input_publisher() -> Self::Publisher {
                #publisher_name
            }

            fn input_subscriber() -> Self::Subscriber {
                #subscriber_name
            }
        }

        impl #impl_generics ::rmk::event::AsyncInputEvent for #enum_name #ty_generics #where_clause {
            type AsyncPublisher = #publisher_name;

            fn input_publisher_async() -> Self::AsyncPublisher {
                #publisher_name
            }
        }

        #(#from_impls)*
    };

    expanded.into()
}
