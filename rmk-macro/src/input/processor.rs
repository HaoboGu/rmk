use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::{DeriveInput, Meta, Path, parse_macro_input};

use super::runnable::{
    ControllerConfig, InputProcessorConfig, deduplicate_type_generics, event_type_to_handler_method_name,
    generate_runnable, has_runnable_marker, is_runnable_generated_attr, parse_controller_config, reconstruct_type_def,
};

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

    // Parse attributes to extract event types
    let config = parse_processor_attributes(attr);

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

    // Generate internal enum name
    let enum_name = format_ident!("{}EventEnum", struct_name);

    // Generate enum variants and related code
    let enum_variants: Vec<_> = config
        .event_types
        .iter()
        .enumerate()
        .map(|(idx, event_type)| {
            let variant_name = format_ident!("Event{}", idx);
            quote! { #variant_name(#event_type) }
        })
        .collect();

    // Generate match arms for process method
    let process_event_arms: Vec<_> = config
        .event_types
        .iter()
        .enumerate()
        .map(|(idx, event_type)| {
            let variant_name = format_ident!("Event{}", idx);
            let method_name = event_type_to_handler_method_name(event_type);
            quote! {
                #enum_name::#variant_name(event) => self.#method_name(event).await
            }
        })
        .collect();

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
        quote! { #[::rmk::runnable_generated] }
    } else {
        quote! {}
    };

    // Generate the complete output
    let expanded = quote! {
        #(#attrs)*
        #marker_attr
        #vis #struct_def

        // Internal enum for event routing
        #vis enum #enum_name {
            #(#enum_variants),*
        }

        #runnable_impl

        impl #impl_generics ::rmk::input_device::InputProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER> for #struct_name #deduped_ty_generics #where_clause {
            type Event = #enum_name;

            async fn process(&mut self, event: Self::Event) {
                match event {
                    #(#process_event_arms),*
                }
            }

            fn get_keymap(&self) -> &::core::cell::RefCell<::rmk::KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>> {
                self.keymap
            }
        }
    };

    expanded.into()
}

/// InputProcessor attribute configuration
struct ProcessorConfig {
    event_types: Vec<Path>,
}

/// Parse #[input_processor] subscribe attribute
fn parse_processor_attributes(attr: proc_macro::TokenStream) -> ProcessorConfig {
    use syn::punctuated::Punctuated;
    use syn::{ExprArray, Token};

    let mut event_types = Vec::new();

    // Parse as Meta::List containing name-value pairs
    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let attr2: proc_macro2::TokenStream = attr.into();

    match parser.parse2(attr2) {
        Ok(parsed) => {
            for meta in parsed {
                if let Meta::NameValue(nv) = meta {
                    if nv.path.is_ident("subscribe") {
                        // Parse the array of event types
                        if let syn::Expr::Array(ExprArray { elems, .. }) = nv.value {
                            for elem in elems {
                                if let syn::Expr::Path(expr_path) = elem {
                                    event_types.push(expr_path.path);
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            panic!("Failed to parse input_processor attributes: {}", e);
        }
    }

    ProcessorConfig { event_types }
}
