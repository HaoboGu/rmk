pub(crate) mod channel;
pub(crate) mod config;
pub(crate) mod event;
pub(crate) mod parser;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use rmk_config::{ChipModel, KeyboardTomlConfig, PinConfig};
use syn::{DeriveInput, ItemMod, Meta, parse_macro_input};

use crate::feature::{get_rmk_features, is_feature_enabled};
use crate::gpio_config::convert_gpio_str_to_output_pin;
use crate::input::config::{InputDeviceConfig, InputProcessorConfig};
use crate::input::parser::{parse_input_device_config, parse_input_processor_config};
use crate::runnable::{
    EventTraitType, event_type_to_handler_method_name, generate_event_match_arms, generate_event_subscriber,
    generate_runnable, generate_unique_variant_names,
};
use crate::utils::{deduplicate_type_generics, has_runnable_marker, is_runnable_generated_attr, reconstruct_type_def};

use self::config::ControllerConfig;
use self::parser::parse_controller_config;

/// Expand controller init/exec blocks from keyboard config.
/// Returns (initializers, executors).
pub(crate) fn expand_controller_init(
    keyboard_config: &KeyboardTomlConfig,
    item_mod: &ItemMod,
) -> (TokenStream, Vec<TokenStream>) {
    let controller_feature_enabled = is_feature_enabled(&get_rmk_features(), "controller");

    let mut initializers = TokenStream::new();
    let mut executors = vec![];

    let (i, e) = expand_light_controllers(keyboard_config, controller_feature_enabled);
    initializers.extend(i);
    executors.extend(e);

    // Custom controllers declared in the module.
    if let Some((_, items)) = &item_mod.content {
        items.iter().for_each(|item| {
            if let syn::Item::Fn(item_fn) = &item
                && let Some(attr) = item_fn.attrs.iter().find(|attr| attr.path().is_ident("register_controller")) {
                    let _ = attr.parse_nested_meta(|meta| {
                        if !controller_feature_enabled {
                            panic!("\"controller\" feature of RMK must be enabled to use the #[register_controller] attribute");
                        }
                        let (custom_init, custom_exec) = expand_custom_controller(item_fn);
                        initializers.extend(custom_init);

                        if meta.path.is_ident("event") {
                            // #[register_controller(event)]
                            executors.push(quote! { #custom_exec.event_loop() });
                            return Ok(());
                        } else if meta.path.is_ident("poll") {
                            // #[register_controller(poll)]
                            executors.push(quote! { #custom_exec.polling_loop() });
                            return Ok(());
                        }

                        panic!("#[register_controller] must specify execution mode with `event` or `poll`. Use `#[register_controller(event)]` or `#[register_controller(poll)]`")
                    });
                }
        });
    }

    (initializers, executors)
}

fn expand_light_controllers(
    keyboard_config: &KeyboardTomlConfig,
    controller_feature_enabled: bool,
) -> (TokenStream, Vec<TokenStream>) {
    let chip = keyboard_config.get_chip_model().unwrap();
    let light_config = keyboard_config.get_light_config();

    let mut initializers = TokenStream::new();
    let mut executors = vec![];

    create_keyboard_indicator_controller(
        &chip,
        &light_config.numslock,
        quote! { numlock_controller },
        quote! { NumLock },
        controller_feature_enabled,
        &mut initializers,
        &mut executors,
    );

    create_keyboard_indicator_controller(
        &chip,
        &light_config.scrolllock,
        quote! { scrolllock_controller },
        quote! { ScrollLock },
        controller_feature_enabled,
        &mut initializers,
        &mut executors,
    );

    create_keyboard_indicator_controller(
        &chip,
        &light_config.capslock,
        quote! { capslock_controller },
        quote! { CapsLock },
        controller_feature_enabled,
        &mut initializers,
        &mut executors,
    );

    (initializers, executors)
}

fn create_keyboard_indicator_controller(
    chip: &ChipModel,
    pin_config: &Option<PinConfig>,
    controller_ident: TokenStream,
    led_indicator_variant: TokenStream,
    controller_feature_enabled: bool,
    initializers: &mut TokenStream,
    executors: &mut Vec<TokenStream>,
) {
    if let Some(c) = pin_config {
        if !controller_feature_enabled {
            panic!("\"controller\" feature of RMK must be enabled to use the [light] configuration")
        }
        let p = convert_gpio_str_to_output_pin(chip, c.pin.clone(), c.low_active);
        let low_active = c.low_active;
        let controller_init = quote! {
            let mut #controller_ident = ::rmk::controller::led_indicator::KeyboardIndicatorController::new(
                #p,
                #low_active,
                ::rmk::types::led_indicator::LedIndicatorType::#led_indicator_variant,
            );
        };
        initializers.extend(controller_init);
        executors.push(quote! { #controller_ident.event_loop() });
    }
}

fn expand_custom_controller(fn_item: &syn::ItemFn) -> (TokenStream, &syn::Ident) {
    let task_name = &fn_item.sig.ident;

    let content = &fn_item.block.stmts;
    let initializer = quote! {
        let mut #task_name = {
            #(#content)*
        };
    };

    (initializer, task_name)
}

/// Generate a `Controller` impl for a `#[controller(...)]` struct.
/// Supports `subscribe = [...]` and optional `poll_interval = N`.
/// See `rmk::controller::Controller` for details.
///
/// Example:
/// ```rust,ignore
/// #[controller(subscribe = [BatteryEvent, ChargingStateEvent])]
/// pub struct BatteryLedController { ... }
/// ```
pub fn controller_impl(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    // Parse subscribe/poll attributes using shared parser.
    let config = parse_controller_config(attr);

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
    let has_input_device = input.attrs.iter().any(|attr| attr.path().is_ident("input_device"));
    let has_input_processor = input.attrs.iter().any(|attr| attr.path().is_ident("input_processor"));

    // Parse input_device config if present.
    let input_device_config: Option<InputDeviceConfig> = if has_input_device {
        input
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("input_device"))
            .and_then(|attr| {
                if let Meta::List(meta_list) = &attr.meta {
                    parse_input_device_config(meta_list.tokens.clone())
                } else {
                    None
                }
            })
    } else {
        None
    };

    // Parse input_processor config if present.
    let input_processor_config: Option<InputProcessorConfig> = if has_input_processor {
        input
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("input_processor"))
            .map(|attr| {
                if let Meta::List(meta_list) = &attr.meta {
                    parse_input_processor_config(meta_list.tokens.clone())
                } else {
                    InputProcessorConfig { event_types: vec![] }
                }
            })
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

    // Check if single event (no need for aggregated enum)
    let is_single_event = config.event_types.len() == 1;

    // Generate event-related code based on single vs multiple events
    let (event_type_tokens, event_enum_def, event_subscriber_impl, process_event_body) = if is_single_event {
        // Single event: use the event type directly, no enum needed
        let event_type = &config.event_types[0];
        let method_name = event_type_to_handler_method_name(event_type);

        (
            quote! { #event_type },
            quote! {}, // No enum definition
            quote! {}, // No custom EventSubscriber needed, use default
            quote! { self.#method_name(event).await },
        )
    } else {
        // Multiple events: generate aggregated enum
        let enum_name = format_ident!("{}EventEnum", struct_name);
        let variant_names = generate_unique_variant_names(&config.event_types);

        // Build enum variants
        let enum_variants: Vec<_> = config
            .event_types
            .iter()
            .zip(&variant_names)
            .map(|(event_type, variant_name)| {
                quote! { #variant_name(#event_type) }
            })
            .collect();

        // process_event match arms
        let process_event_arms = generate_event_match_arms(&config.event_types, &variant_names, &enum_name);

        // Generate EventSubscriber struct and impl
        let subscriber_impl = generate_event_subscriber(
            struct_name,
            &config.event_types,
            &variant_names,
            &enum_name,
            vis,
            EventTraitType::Controller,
        );

        let enum_def = quote! {
            #[derive(Clone)]
            #vis enum #enum_name {
                #(#enum_variants),*
            }
        };

        (
            quote! { #enum_name },
            enum_def,
            subscriber_impl,
            quote! {
                match event {
                    #(#process_event_arms),*
                }
            },
        )
    };

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
        #vis #struct_def

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
