use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Meta, parse_macro_input};

use super::config::InputDeviceConfig;
use super::parser::parse_input_device_config;
use crate::processor::ProcessorConfig;
use crate::runnable::generate_runnable;
use crate::utils::{
    AttributeParser, deduplicate_type_generics, has_runnable_marker, is_rmk_attr,
    is_runnable_generated_attr, to_snake_case,
};

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
    let device_config = match parse_input_device_config(proc_macro2::TokenStream::from(attr)) {
        Ok(config) => config,
        Err(err) => return err.into(),
    };

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

    // Check for runnable_generated marker (set by #[processor] when it expands first)
    let has_marker = has_runnable_marker(&input.attrs);

    // Check for processor attribute (for combined Runnable generation)
    let has_processor = input
        .attrs
        .iter()
        .any(|attr| is_rmk_attr(attr, "processor"));

    // Parse processor config: either from a sibling #[processor] attribute,
    // or from the #[runnable_generated(...)] marker that #[processor] left behind.
    let processor_config: Option<ProcessorConfig> = if has_processor {
        // #[input_device] expanded first — read config from sibling #[processor]
        match input.attrs.iter().find(|attr| is_rmk_attr(attr, "processor")) {
            Some(attr) => {
                if let Meta::List(meta_list) = &attr.meta {
                    let parser = match AttributeParser::new(meta_list.tokens.clone()) {
                        Ok(parser) => parser,
                        Err(err) => return err.to_compile_error().into(),
                    };

                    let event_types = match parser.get_path_array("subscribe") {
                        Ok(event_types) => event_types,
                        Err(err) => return err.into(),
                    };

                    Some(ProcessorConfig {
                        event_types,
                        poll_interval_ms: match parser.get_int("poll_interval") {
                            Ok(value) => value,
                            Err(err) => return err.into(),
                        },
                    })
                } else {
                    Some(ProcessorConfig {
                        event_types: vec![],
                        poll_interval_ms: None,
                    })
                }
            }
            None => None,
        }
    } else if has_marker {
        // #[processor] expanded first — extract config from the marker's args
        match input
            .attrs
            .iter()
            .find(|attr| is_runnable_generated_attr(attr))
        {
            Some(attr) => {
                if let Meta::List(meta_list) = &attr.meta {
                    let parser = match AttributeParser::new(meta_list.tokens.clone()) {
                        Ok(parser) => parser,
                        Err(err) => return err.to_compile_error().into(),
                    };

                    let event_types = match parser.get_path_array("subscribe") {
                        Ok(event_types) => event_types,
                        Err(err) => return err.into(),
                    };

                    if event_types.is_empty() {
                        None
                    } else {
                        Some(ProcessorConfig {
                            event_types,
                            poll_interval_ms: match parser.get_int("poll_interval") {
                                Ok(value) => value,
                                Err(err) => return err.into(),
                            },
                        })
                    }
                } else {
                    // Bare #[runnable_generated] with no args — no processor config
                    None
                }
            }
            None => None,
        }
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
    // When the marker is present but carries processor config (processor expanded first),
    // we still need to generate the combined Runnable here.
    let runnable_impl = if has_marker && processor_config.is_none() {
        // Bare marker with no processor config — another macro already generated Runnable
        quote! {}
    } else if has_marker && processor_config.is_some() {
        // Marker with processor config — processor expanded first, we generate combined Runnable
        let input_device_cfg = InputDeviceConfig {
            event_type: event_type.clone(),
        };
        generate_runnable(
            struct_name,
            generics,
            where_clause,
            Some(&input_device_cfg),
            processor_config.as_ref(),
        )
    } else {
        let input_device_cfg = InputDeviceConfig {
            event_type: event_type.clone(),
        };
        generate_runnable(
            struct_name,
            generics,
            where_clause,
            Some(&input_device_cfg),
            processor_config.as_ref(),
        )
    };

    // Remove attributes that would cause duplicate expansion or should not leak to output.
    input
        .attrs
        .retain(|attr| !is_rmk_attr(attr, "input_device") && !is_runnable_generated_attr(attr));

    // Add marker attribute if we generated Runnable and there are other macros.
    if !has_marker && has_processor {
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
