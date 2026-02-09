use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Meta, parse_macro_input};

use super::config::{ControllerConfig, InputDeviceConfig};
use super::parser::{parse_controller_config, parse_input_device_config};
use super::runnable::generate_runnable;
use super::utils::{deduplicate_type_generics, has_runnable_marker, is_runnable_generated_attr};
use crate::utils::to_snake_case;

/// Generates InputDevice and Runnable trait implementations for single-event devices.
///
/// This macro is used to define InputDevice structs that publish a single event type:
/// ```rust,ignore
/// #[input_device(publish = BatteryEvent)]
/// pub struct BatteryReader { ... }
///
/// impl BatteryReader {
///     async fn read_battery_event(&mut self) -> BatteryEvent {
///         // Wait and return single event
///     }
/// }
/// ```
pub fn input_device_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as DeriveInput);

    // Parse attributes to extract event type using shared parser
    let device_config = parse_input_device_config(proc_macro2::TokenStream::from(attr));

    // Validate single event type
    if device_config.is_none() {
        return syn::Error::new_spanned(
            &input,
            "#[input_device] requires `publish` attribute with a single event type. Use `#[input_device(publish = EventType)]`",
        )
        .to_compile_error()
        .into();
    }

    let event_type = device_config.unwrap().event_type;

    // Validate input is a struct
    if !matches!(input.data, syn::Data::Struct(_)) {
        return syn::Error::new_spanned(&input, "#[input_device] can only be applied to structs")
            .to_compile_error()
            .into();
    }

    // Check for mutually exclusive attributes
    let has_input_processor = input
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("input_processor"));
    if has_input_processor {
        return syn::Error::new_spanned(
            &input,
            "#[input_device] and #[input_processor] are mutually exclusive. A struct cannot be both an input device and an input processor.",
        )
        .to_compile_error()
        .into();
    }

    // Check for runnable_generated marker
    let has_marker = has_runnable_marker(&input.attrs);

    // Check for controller attribute (for combined Runnable generation)
    let has_controller = input
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("controller"));

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
    let generics = &input.generics;
    let (impl_generics, _ty_generics, where_clause) = generics.split_for_impl();

    // Use deduplicated type generics to handle cfg-conditional generic parameters
    let deduped_ty_generics = deduplicate_type_generics(generics);

    // Generate method name from event type
    let type_name = event_type.segments.last().unwrap().ident.to_string();
    let base_name = type_name.strip_suffix("Event").unwrap_or(&type_name);
    let method_name = format_ident!("read_{}_event", to_snake_case(base_name));

    // Generate Runnable implementation
    let runnable_impl = if has_marker {
        // Skip Runnable generation if marker is present
        quote! {}
    } else {
        let input_device_cfg = InputDeviceConfig {
            event_type: event_type.clone(),
        };
        generate_runnable(
            struct_name,
            generics,
            where_clause,
            Some(&input_device_cfg),
            None, // no input_processor
            controller_config.as_ref(),
        )
    };

    // Remove attributes that would cause duplicate expansion or should not leak to output.
    input
        .attrs
        .retain(|attr| !attr.path().is_ident("input_device") && !is_runnable_generated_attr(attr));

    // Add marker attribute if we generated Runnable and there are other macros.
    if !has_marker && has_controller {
        input
            .attrs
            .push(syn::parse_quote!(#[::rmk::macros::runnable_generated]));
    }

    // Generate the complete output
    let expanded = quote! {
        #input

        impl #impl_generics ::rmk::input_device::InputDevice for #struct_name #deduped_ty_generics #where_clause {
            type Event = #event_type;

            async fn read_event(&mut self) -> Self::Event {
                self.#method_name().await
            }
        }

        #runnable_impl
    };

    expanded.into()
}
