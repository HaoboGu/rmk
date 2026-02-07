use quote::quote;
use syn::{DeriveInput, parse_macro_input};

use super::channel::{generate_input_event_channel, validate_event_type};
use super::parser::parse_input_event_channel_size;
use crate::controller::channel::generate_controller_event_channel;
use crate::controller::parser::parse_controller_event_channel_config_from_attr;

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

    // Validate event type
    if let Some(error) = validate_event_type(&input, "input_event") {
        return error.into();
    }

    // Check if controller_event macro is also present
    let controller_event_attr = input
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("controller_event"))
        .cloned();

    let type_name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Generate input event channel
    let (input_channel_static, input_trait_impls) =
        generate_input_event_channel(type_name, &ty_generics, &impl_generics, where_clause, channel_size);

    // Remove both macros from attributes for the final struct definition
    input
        .attrs
        .retain(|attr| !attr.path().is_ident("input_event") && !attr.path().is_ident("controller_event"));

    let expanded = if let Some(ctrl_attr) = controller_event_attr.as_ref() {
        // controller_event is also present, generate both sets of implementations
        let ctrl_config = parse_controller_event_channel_config_from_attr(ctrl_attr);
        let (controller_channel_static, controller_trait_impls) =
            generate_controller_event_channel(type_name, &ty_generics, &impl_generics, where_clause, &ctrl_config);

        quote! {
            #input

            #input_channel_static
            #input_trait_impls

            #controller_channel_static
            #controller_trait_impls
        }
    } else {
        // Only input_event
        quote! {
            #input

            #input_channel_static
            #input_trait_impls
        }
    };

    expanded.into()
}
