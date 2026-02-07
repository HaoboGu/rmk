use quote::quote;
use syn::{DeriveInput, parse_macro_input};

use super::channel::generate_controller_event_channel;
use super::parser::parse_controller_event_channel_config;
use crate::input::channel::{generate_input_event_channel, validate_event_type};
use crate::input::parser::parse_input_event_channel_size_from_attr;

/// Generates controller event infrastructure.
///
/// This macro can be combined with `#[input_event]` on the same struct to create
/// a dual-channel event type that supports both input and controller event patterns.
/// The order of the two macros does not matter.
///
/// **Note**: Generic event types are not supported because static channels cannot be generic.
///
/// See `rmk::event::ControllerEvent` for usage.
pub fn controller_event_impl(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(item as DeriveInput);

    // Parse attributes
    let config = parse_controller_event_channel_config(proc_macro2::TokenStream::from(attr));

    // Validate event type
    if let Some(error) = validate_event_type(&input, "controller_event") {
        return error.into();
    }

    // Check if input_event macro is also present
    let input_event_attr = input
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("input_event"))
        .cloned();

    let type_name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Generate controller event channel
    let (controller_channel_static, controller_trait_impls) =
        generate_controller_event_channel(type_name, &ty_generics, &impl_generics, where_clause, &config);

    // Remove both macros from attributes for the final struct definition
    input
        .attrs
        .retain(|attr| !attr.path().is_ident("input_event") && !attr.path().is_ident("controller_event"));

    let expanded = if let Some(input_attr) = input_event_attr.as_ref() {
        // input_event is also present, generate both sets of implementations
        let input_channel_size = parse_input_event_channel_size_from_attr(input_attr);
        let (input_channel_static, input_trait_impls) = generate_input_event_channel(
            type_name,
            &ty_generics,
            &impl_generics,
            where_clause,
            input_channel_size,
        );

        quote! {
            #input

            #controller_channel_static
            #controller_trait_impls

            #input_channel_static
            #input_trait_impls
        }
    } else {
        // Only controller_event
        quote! {
            #input

            #controller_channel_static
            #controller_trait_impls
        }
    };

    expanded.into()
}
