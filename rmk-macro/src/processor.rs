//! Unified processor macro implementation.
//!
//! Generates `Processor` trait implementations for event-driven processors.
//! Replaces both `#[input_processor]` and `#[controller]`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

use crate::runnable::{generate_event_enum_and_dispatch, generate_runnable};
use crate::utils::{AttributeParser, has_runnable_marker, is_rmk_attr};

/// Processor subscription config.
pub struct ProcessorConfig {
    pub event_types: Vec<syn::Path>,
    pub poll_interval_ms: Option<u64>,
}

/// Parse processor config from attribute tokens.
pub fn parse_processor_config(tokens: impl Into<TokenStream>) -> Result<ProcessorConfig, TokenStream> {
    let parser = AttributeParser::new(tokens)
        .map_err(|e| e.to_compile_error())?;

    parser.validate_keys(&["subscribe", "poll_interval"])?;

    Ok(ProcessorConfig {
        event_types: parser.get_path_array("subscribe")?,
        poll_interval_ms: parser.get_int("poll_interval")?,
    })
}

/// Implementation of the unified `#[processor]` macro.
pub fn processor_impl(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(item as DeriveInput);
    let config = match parse_processor_config(proc_macro2::TokenStream::from(attr)) {
        Ok(config) => config,
        Err(err) => return err.into(),
    };

    // Validate that subscribe list is not empty
    if config.event_types.is_empty() {
        return syn::Error::new_spanned(
            &input.ident,
            "#[processor] requires at least one event type in `subscribe`. \
             Use `#[processor(subscribe = [EventType])]`.",
        )
        .to_compile_error()
        .into();
    }

    let struct_name = &input.ident;
    let vis = &input.vis;
    let generics = &input.generics;
    let (impl_generics, _, where_clause) = generics.split_for_impl();
    let deduped_ty_generics = crate::utils::deduplicate_type_generics(generics);

    let has_marker = has_runnable_marker(&input.attrs);

    // Check for sibling #[input_device] attribute — if present, let input_device_impl
    // handle the combined Runnable generation.
    let has_input_device = input
        .attrs
        .iter()
        .any(|attr| is_rmk_attr(attr, "input_device"));

    // When a sibling #[input_device] exists and no marker yet, add a marker that
    // carries the processor config so input_device_impl can generate the combined Runnable.
    if has_input_device && !has_marker {
        let event_types = &config.event_types;
        let marker = if let Some(interval_ms) = config.poll_interval_ms {
            syn::parse_quote!(#[::rmk::macros::runnable_generated(subscribe = [#(#event_types),*], poll_interval = #interval_ms)])
        } else {
            syn::parse_quote!(#[::rmk::macros::runnable_generated(subscribe = [#(#event_types),*])])
        };
        input.attrs.push(marker);
    }

    // Generate event enum, subscriber, and dispatch body
    let (event_type_tokens, event_enum_def, event_subscriber_impl, process_body) =
        generate_event_enum_and_dispatch(
            struct_name,
            vis,
            &config.event_types,
            "Processor",
            quote! { ::rmk::event::SubscribableEvent },
            quote! { subscriber },
        );

    // PollingProcessor impl when poll_interval is set
    let polling_processor_impl = if let Some(interval_ms) = config.poll_interval_ms {
        quote! {
            impl #impl_generics ::rmk::processor::PollingProcessor for #struct_name #deduped_ty_generics #where_clause {
                fn interval(&self) -> ::embassy_time::Duration {
                    ::embassy_time::Duration::from_millis(#interval_ms)
                }

                async fn update(&mut self) {
                    self.poll().await;
                }
            }
        }
    } else {
        quote! {}
    };

    // Generate Runnable implementation
    // Skip if: marker was already present, OR a sibling #[input_device] will handle it
    let runnable_impl = if has_marker || has_input_device {
        quote! {}
    } else {
        let processor_config = ProcessorConfig {
            event_types: config.event_types.clone(),
            poll_interval_ms: config.poll_interval_ms,
        };
        generate_runnable(
            struct_name,
            generics,
            where_clause,
            None,
            Some(&processor_config),
        )
    };

    let expanded = quote! {
        #input

        #event_enum_def
        #event_subscriber_impl

        impl #impl_generics ::rmk::processor::Processor for #struct_name #deduped_ty_generics #where_clause {
            type Event = #event_type_tokens;
            type Subscriber = <#event_type_tokens as ::rmk::event::SubscribableEvent>::Subscriber;

            fn subscriber() -> Self::Subscriber {
                <#event_type_tokens as ::rmk::event::SubscribableEvent>::subscriber()
            }

            async fn process(&mut self, event: Self::Event) {
                #process_body
            }
        }

        #polling_processor_impl

        #runnable_impl
    };

    expanded.into()
}
