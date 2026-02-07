use quote::quote;
use syn::{DeriveInput, Meta, parse_macro_input};

use super::config::InputProcessorConfig;
use super::parser::parse_input_processor_config;
use crate::controller::config::ControllerConfig;
use crate::controller::parser::parse_controller_config;
use crate::runnable::{generate_event_enum_and_dispatch, generate_runnable};
use crate::utils::{deduplicate_type_generics, has_runnable_marker, is_runnable_generated_attr, reconstruct_type_def};

/// Generates InputProcessor trait implementation with automatic event routing.
///
/// See `rmk::input_device::InputProcessor` trait documentation for usage.
///
/// This macro is used to define InputProcessor structs:
/// ```rust,ignore
/// #[input_processor(subscribe = [BatteryEvent, ChargingStateEvent])]
/// pub struct BatteryProcessor { ... }
/// ```
pub fn input_processor_impl(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    // Parse attributes to extract event types using shared parser.
    let config = parse_input_processor_config(attr);

    if config.event_types.is_empty() {
        return syn::Error::new_spanned(
            input,
            "#[input_processor] requires `subscribe` attribute with at least one event type. Use `#[input_processor(subscribe = [EventType1, EventType2])]`"
        )
        .to_compile_error()
        .into();
    }

    // Validate input is a struct
    if !matches!(input.data, syn::Data::Struct(_)) {
        return syn::Error::new_spanned(input, "#[input_processor] can only be applied to structs")
            .to_compile_error()
            .into();
    }

    // Check for mutually exclusive attributes
    let has_input_device = input.attrs.iter().any(|attr| attr.path().is_ident("input_device"));
    if has_input_device {
        return syn::Error::new_spanned(
            &input,
            "#[input_processor] and #[input_device] are mutually exclusive. A struct cannot be both an input processor and an input device.",
        )
        .to_compile_error()
        .into();
    }

    // Check for runnable_generated marker
    let has_marker = has_runnable_marker(&input.attrs);

    // Check for controller attribute (for combined Runnable generation)
    let has_controller = input.attrs.iter().any(|attr| attr.path().is_ident("controller"));

    // Parse controller config if present (for combined Runnable)
    let controller_config: Option<ControllerConfig> = if has_controller {
        input
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("controller"))
            .map(|attr| {
                if let Meta::List(meta_list) = &attr.meta {
                    parse_controller_config(meta_list.tokens.clone())
                } else {
                    ControllerConfig {
                        event_types: vec![],
                        poll_interval_ms: None,
                    }
                }
            })
    } else {
        None
    };

    let struct_name = &input.ident;
    let vis = &input.vis;
    let generics = &input.generics;
    let (impl_generics, _ty_generics, where_clause) = generics.split_for_impl();

    // Use deduplicated type generics to handle cfg-conditional generic parameters
    let deduped_ty_generics = deduplicate_type_generics(generics);

    // Filter out input_processor attribute and runnable_generated marker from output
    let attrs: Vec<_> = input
        .attrs
        .iter()
        .filter(|attr| !attr.path().is_ident("input_processor") && !is_runnable_generated_attr(attr))
        .collect();

    // Reconstruct the struct definition
    let struct_def = reconstruct_type_def(&input);

    // Generate event enum, subscriber, and dispatch body
    let (event_type_tokens, event_enum_def, event_subscriber_impl, process_body) = generate_event_enum_and_dispatch(
        struct_name,
        vis,
        &config.event_types,
        "Input",
        quote! { ::rmk::event::SubscribableInputEvent },
        quote! { input_subscriber },
    );

    // Generate Runnable implementation
    let runnable_impl = if has_marker {
        // Skip Runnable generation if marker is present
        quote! {}
    } else {
        let processor_cfg = InputProcessorConfig {
            event_types: config.event_types.clone(),
        };
        generate_runnable(
            struct_name,
            generics,
            where_clause,
            None, // no input_device
            Some(&processor_cfg),
            controller_config.as_ref(),
        )
    };

    // Add marker attribute if we generated Runnable and there are other macros
    let marker_attr = if !has_marker && has_controller {
        quote! { #[::rmk::macros::runnable_generated] }
    } else {
        quote! {}
    };

    // Generate the complete output
    let expanded = quote! {
        #(#attrs)*
        #marker_attr
        #struct_def

        #event_enum_def

        #event_subscriber_impl

        #runnable_impl

        impl #impl_generics ::rmk::input_device::InputProcessor for #struct_name #deduped_ty_generics #where_clause {
            type Event = #event_type_tokens;

            async fn process(&mut self, event: Self::Event) {
                #process_body
            }
        }
    };

    expanded.into()
}
