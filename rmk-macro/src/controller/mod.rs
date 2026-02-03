pub(crate) mod event;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use rmk_config::{ChipModel, KeyboardTomlConfig, PinConfig};
use syn::parse::Parser;
use syn::{DeriveInput, ItemMod, Meta, Path, parse_macro_input};

use crate::feature::{get_rmk_features, is_feature_enabled};
use crate::gpio_config::convert_gpio_str_to_output_pin;
use crate::input::runnable::{
    ControllerConfig as SharedControllerConfig, InputDeviceConfig, InputProcessorConfig, deduplicate_type_generics,
    event_type_to_handler_method_name, generate_runnable, has_runnable_marker, is_runnable_generated_attr,
    parse_input_device_config, parse_input_processor_config, reconstruct_type_def,
};

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

    // Parse subscribe/poll attributes.
    let config = parse_controller_attributes(attr);

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

    // Internal event enum name.
    let enum_name = format_ident!("{}EventEnum", struct_name);

    // Build enum variants.
    let enum_variants: Vec<_> = config
        .event_types
        .iter()
        .enumerate()
        .map(|(idx, event_type)| {
            let variant_name = format_ident!("Event{}", idx);
            quote! { #variant_name(#event_type) }
        })
        .collect();

    // From impls for each event type.
    // Enum has no generics, so use plain impl.
    let from_impls: Vec<_> = config
        .event_types
        .iter()
        .enumerate()
        .map(|(idx, event_type)| {
            let variant_name = format_ident!("Event{}", idx);
            quote! {
                impl From<#event_type> for #enum_name {
                    fn from(e: #event_type) -> Self {
                        #enum_name::#variant_name(e)
                    }
                }
            }
        })
        .collect();

    // process_event match arms.
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

    // next_message implementation via select_biased.
    let next_message_impl = generate_next_message(&config.event_types, &enum_name);

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
        let controller_cfg = SharedControllerConfig {
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
        quote! { #[::rmk::runnable_generated] }
    } else {
        quote! {}
    };

    // Assemble output.
    let expanded = quote! {
        #(#attrs)*
        #marker_attr
        #vis #struct_def

        // Internal event enum for routing (match struct visibility).
        #vis enum #enum_name {
            #(#enum_variants),*
        }

        // From impls for event conversion.
        #(#from_impls)*

        impl #impl_generics ::rmk::controller::Controller for #struct_name #deduped_ty_generics #where_clause {
            type Event = #enum_name;

            async fn process_event(&mut self, event: Self::Event) {
                match event {
                    #(#process_event_arms),*
                }
            }

            #next_message_impl
        }

        #polling_controller_impl

        #runnable_impl
    };

    expanded.into()
}

/// Controller attribute config.
struct ControllerConfig {
    event_types: Vec<Path>,
    poll_interval_ms: Option<u64>,
}

/// Parse #[controller(...)] attribute.
fn parse_controller_attributes(attr: proc_macro::TokenStream) -> ControllerConfig {
    use syn::punctuated::Punctuated;
    use syn::{ExprArray, Token};

    let mut event_types = Vec::new();
    let mut poll_interval_ms = None;

    // Parse Meta::List name-value pairs.
    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let attr2: proc_macro2::TokenStream = attr.into();

    match parser.parse2(attr2) {
        Ok(parsed) => {
            for meta in parsed {
                if let Meta::NameValue(nv) = meta {
                    if nv.path.is_ident("subscribe") {
                        // Parse event type list.
                        if let syn::Expr::Array(ExprArray { elems, .. }) = nv.value {
                            for elem in elems {
                                if let syn::Expr::Path(expr_path) = elem {
                                    event_types.push(expr_path.path);
                                }
                            }
                        }
                    } else if nv.path.is_ident("poll_interval") {
                        // Parse poll_interval as milliseconds.
                        if let syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Int(lit_int),
                            ..
                        }) = nv.value
                        {
                            poll_interval_ms = Some(
                                lit_int
                                    .base10_parse::<u64>()
                                    .expect("poll_interval must be a valid u64"),
                            );
                        } else {
                            panic!("poll_interval must be an integer literal (milliseconds)");
                        }
                    }
                }
            }
        }
        Err(e) => {
            panic!("Failed to parse controller attributes: {}", e);
        }
    }

    ControllerConfig {
        event_types,
        poll_interval_ms,
    }
}

/// Build next_message using select_biased.
fn generate_next_message(event_types: &[Path], enum_name: &syn::Ident) -> proc_macro2::TokenStream {
    let num_events = event_types.len();

    // Subscriber variable names.
    let sub_vars: Vec<_> = (0..num_events).map(|i| format_ident!("sub{}", i)).collect();

    // Subscriber initializations.
    let sub_inits: Vec<_> = event_types
        .iter()
        .zip(&sub_vars)
        .map(|(event_type, sub_var)| {
            quote! {
                let mut #sub_var = <#event_type as ::rmk::event::ControllerEvent>::controller_subscriber();
            }
        })
        .collect();

    // select_biased! arms for each event.
    let select_arms: Vec<_> = sub_vars
        .iter()
        .enumerate()
        .map(|(idx, sub_var)| {
            let variant_name = format_ident!("Event{}", idx);
            quote! {
                event = #sub_var.next_event().fuse() => #enum_name::#variant_name(event),
            }
        })
        .collect();

    quote! {
        async fn next_message(&mut self) -> Self::Event {
            use ::rmk::event::EventSubscriber;
            use ::rmk::futures::FutureExt;
            #(#sub_inits)*

            ::rmk::futures::select_biased! {
                #(#select_arms)*
            }
        }
    }
}
