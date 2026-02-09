use syn::{DeriveInput, parse_macro_input};

use super::channel::controller_channel::generate_controller_event_channel;
use super::channel::input_channel::{generate_input_event_channel, validate_event_type};
use super::parser::{
    parse_controller_event_channel_config, parse_input_event_channel_size_from_attr,
};
use super::utils::assemble_dual_event_output;

/// Generates controller event infrastructure.
///
/// This macro can be combined with `#[input_event]` on the same struct to create
/// a dual-channel event type that supports both input and controller event patterns.
/// The order of the two macros does not matter.
///
/// **Note**: Generic event types are not supported because static channels cannot be generic.
///
/// See `rmk::event::ControllerEvent` for usage.
pub fn controller_event_impl(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(item as DeriveInput);

    // Parse attributes
    let config = parse_controller_event_channel_config(proc_macro2::TokenStream::from(attr));

    // Validate event type
    if let Some(error) = validate_event_type(&input, "controller_event") {
        return error.into();
    }

    let type_name = input.ident.clone();
    let generics = input.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Generate controller event channel
    let primary_channel = generate_controller_event_channel(
        &type_name,
        &ty_generics,
        &impl_generics,
        where_clause,
        &config,
    );

    // Assemble output, handling optional dual-macro with input_event
    let expanded = assemble_dual_event_output(
        &mut input,
        "controller_event",
        "input_event",
        primary_channel,
        |input_attr| {
            let input_channel_size = parse_input_event_channel_size_from_attr(input_attr);
            generate_input_event_channel(
                &type_name,
                &ty_generics,
                &impl_generics,
                where_clause,
                input_channel_size,
            )
        },
    );

    expanded.into()
}
