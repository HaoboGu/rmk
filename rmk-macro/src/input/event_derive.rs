use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

/// Derive macro for multi-event enums that generates automatic event dispatch.
///
/// This macro generates:
/// - A `publish()` method that dispatches events to the correct channel based on variant
/// - `From<EventType>` impls for each variant for convenient construction
///
/// # Example
///
/// ```ignore
/// #[derive(InputEvent)]
/// pub enum MultiSensorEvent {
///     Battery(BatteryEvent),
///     Pointing(PointingEvent),
/// }
/// ```
pub fn input_event_derive_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Validate it's an enum
    let data_enum = match &input.data {
        syn::Data::Enum(e) => e,
        _ => {
            return syn::Error::new_spanned(input, "#[derive(InputEvent)] can only be applied to enums")
                .to_compile_error()
                .into()
        }
    };

    let enum_name = &input.ident;
    let vis = &input.vis;

    // Extract generics for generic enum support
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Collect variant info
    let mut publish_arms = Vec::new();
    let mut from_impls = Vec::new();

    for variant in &data_enum.variants {
        let variant_name = &variant.ident;

        // Validate tuple variant with one field
        let inner_type = match &variant.fields {
            syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => &fields.unnamed.first().unwrap().ty,
            _ => {
                return syn::Error::new_spanned(
                    variant,
                    "Each variant must be a tuple variant with exactly one field, e.g., `Battery(BatteryEvent)`",
                )
                .to_compile_error()
                .into()
            }
        };

        // Generate publish match arm
        publish_arms.push(quote! {
            #enum_name::#variant_name(e) => ::rmk::event::publish_input_event_async(e).await
        });

        // Generate From impl with generics
        from_impls.push(quote! {
            impl #impl_generics From<#inner_type> for #enum_name #ty_generics #where_clause {
                fn from(e: #inner_type) -> Self {
                    #enum_name::#variant_name(e)
                }
            }
        });
    }

    let expanded = quote! {
        impl #impl_generics #enum_name #ty_generics #where_clause {
            /// Publish this event to the appropriate channel based on variant
            #vis async fn publish(self) {
                match self {
                    #(#publish_arms),*
                }
            }
        }

        #(#from_impls)*
    };

    expanded.into()
}
