use quote::quote;
use syn::{DeriveInput, Meta, parse_macro_input};

use super::config::{ControllerConfig, InputDeviceConfig, InputProcessorConfig};
use super::parser::{
    parse_controller_config, parse_input_device_config, parse_input_processor_config,
};
use super::runnable::{generate_event_enum_and_dispatch, generate_runnable};
use super::utils::{
    deduplicate_type_generics, has_runnable_marker, is_runnable_generated_attr,
    reconstruct_type_def,
};

/// Generate a `Controller` impl for a `#[controller(...)]` struct.
/// Supports `subscribe = [...]` and optional `poll_interval = N`.
/// See `rmk::controller::Controller` for details.
///
/// Example:
/// ```rust,ignore
/// #[controller(subscribe = [BatteryEvent, ChargingStateEvent])]
/// pub struct BatteryLedController { ... }
/// ```
pub fn controller_impl(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    // Parse subscribe/poll attributes using shared parser.
    let config = match parse_controller_config(attr) {
        Ok(config) => config,
        Err(err) => return err.into(),
    };

    if config.event_types.is_empty() {
        return syn::Error::new_spanned(
            input,
            "#[controller] requires `subscribe` attribute with at least one event type. Use `#[controller(subscribe = [EventType1, EventType2])]`"
        )
        .to_compile_error()
        .into();
    }

    // Require a struct input.
    if !matches!(input.data, syn::Data::Struct(_)) {
        return syn::Error::new_spanned(input, "#[controller] can only be applied to structs")
            .to_compile_error()
            .into();
    }

    // Check runnable_generated marker.
    let has_marker = has_runnable_marker(&input.attrs);

    // Detect input_device/input_processor for combined Runnable generation.
    let has_input_device = input
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("input_device"));
    let has_input_processor = input
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("input_processor"));

    // Parse input_device config if present.
    let input_device_config: Option<InputDeviceConfig> = if has_input_device {
        let parsed = input
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("input_device"))
            .map(|attr| {
                if let Meta::List(meta_list) = &attr.meta {
                    parse_input_device_config(meta_list.tokens.clone())
                } else {
                    Ok(None)
                }
            })
            .transpose();

        match parsed {
            Ok(config) => config.flatten(),
            Err(err) => return err.into(),
        }
    } else {
        None
    };

    // Parse input_processor config if present.
    let input_processor_config: Option<InputProcessorConfig> = if has_input_processor {
        let parsed = input
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("input_processor"))
            .map(|attr| {
                if let Meta::List(meta_list) = &attr.meta {
                    parse_input_processor_config(meta_list.tokens.clone())
                } else {
                    Ok(InputProcessorConfig {
                        event_types: vec![],
                    })
                }
            })
            .transpose();

        match parsed {
            Ok(config) => config,
            Err(err) => return err.into(),
        }
    } else {
        None
    };

    let struct_name = &input.ident;
    let vis = &input.vis;
    let generics = &input.generics;
    let (impl_generics, _ty_generics, where_clause) = generics.split_for_impl();

    // Dedup cfg-conditional generics for type position.
    let deduped_ty_generics = deduplicate_type_generics(generics);

    // Drop controller/runnable marker attrs from output.
    let attrs: Vec<_> = input
        .attrs
        .iter()
        .filter(|attr| !attr.path().is_ident("controller") && !is_runnable_generated_attr(attr))
        .collect();

    // Rebuild struct definition.
    let struct_def = reconstruct_type_def(&input);

    // Generate event enum, subscriber, and dispatch body
    let (event_type_tokens, event_enum_def, event_subscriber_impl, process_event_body) =
        generate_event_enum_and_dispatch(
            struct_name,
            vis,
            &config.event_types,
            "Controller",
            quote! { ::rmk::event::SubscribableControllerEvent },
            quote! { controller_subscriber },
        );

    // PollingController impl when poll_interval is set.
    let polling_controller_impl = if let Some(interval_ms) = config.poll_interval_ms {
        quote! {
            impl #impl_generics ::rmk::controller::PollingController for #struct_name #deduped_ty_generics #where_clause {
                fn interval(&self) -> ::embassy_time::Duration {
                    ::embassy_time::Duration::from_millis(#interval_ms)
                }

                async fn update(&mut self) {
                    self.poll().await
                }
            }
        }
    } else {
        quote! {}
    };

    // Runnable impl (if not generated elsewhere).
    let runnable_impl = if has_marker {
        // Skip when another macro already generated it.
        quote! {}
    } else {
        let controller_cfg = ControllerConfig {
            event_types: config.event_types.clone(),
            poll_interval_ms: config.poll_interval_ms,
        };

        generate_runnable(
            struct_name,
            generics,
            where_clause,
            input_device_config.as_ref(),
            input_processor_config.as_ref(),
            Some(&controller_cfg),
        )
    };

    // Add runnable_generated marker for combined macros.
    let marker_attr = if !has_marker && (has_input_device || has_input_processor) {
        quote! { #[::rmk::macros::runnable_generated] }
    } else {
        quote! {}
    };

    // Assemble output.
    let expanded = quote! {
        #(#attrs)*
        #marker_attr
        #struct_def

        #event_enum_def

        #event_subscriber_impl

        impl #impl_generics ::rmk::controller::Controller for #struct_name #deduped_ty_generics #where_clause {
            type Event = #event_type_tokens;

            async fn process_event(&mut self, event: Self::Event) {
                #process_event_body
            }
        }

        #polling_controller_impl

        #runnable_impl
    };

    expanded.into()
}
