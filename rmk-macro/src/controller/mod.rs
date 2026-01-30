pub(crate) mod event;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use rmk_config::{ChipModel, KeyboardTomlConfig, PinConfig};
use syn::parse::Parser;
use syn::{DeriveInput, ItemMod, Meta, Path, parse_macro_input};

use crate::feature::{get_rmk_features, is_feature_enabled};
use crate::gpio_config::convert_gpio_str_to_output_pin;

/// Expands the controller initialization code based on the keyboard configuration.
/// Returns a tuple containing: (controller_initialization, controller_execution)
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

    // external controller
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
    let chip = keyboard_config.chip().unwrap();
    let light_config = keyboard_config.light();

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

/// Generates Controller trait implementation with automatic event routing.
///
/// See `rmk::controller::Controller` trait documentation for usage.
///
/// This macro is used to define Controller structs:
/// ```rust
/// #[controller(subscribe = [BatteryEvent, ChargingStateEvent])]
/// pub struct BatteryLedController { ... }
/// ```
pub fn controller_impl(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    // Parse attributes to extract event types and polling configuration
    let config = parse_controller_attributes(attr);

    if config.event_types.is_empty() {
        return syn::Error::new_spanned(
            input,
            "#[controller] requires `subscribe` attribute with at least one event type. Use `#[controller(subscribe = [EventType1, EventType2])]`"
        )
        .to_compile_error()
        .into();
    }

    // Validate input is a struct
    if !matches!(input.data, syn::Data::Struct(_)) {
        return syn::Error::new_spanned(input, "#[controller] can only be applied to structs")
            .to_compile_error()
            .into();
    }

    let struct_name = &input.ident;
    let vis = &input.vis;
    let attrs = &input.attrs;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Reconstruct the struct definition
    let struct_def = match &input.data {
        syn::Data::Struct(data_struct) => match &data_struct.fields {
            syn::Fields::Named(fields) => {
                quote! { struct #struct_name #generics #fields #where_clause }
            }
            syn::Fields::Unnamed(fields) => {
                quote! { struct #struct_name #generics #fields #where_clause ; }
            }
            syn::Fields::Unit => {
                quote! { struct #struct_name #generics #where_clause ; }
            }
        },
        _ => unreachable!(),
    };

    // Generate internal enum name
    let enum_name = format_ident!("{}EventEnum", struct_name);

    // Generate enum variants and related code
    let enum_variants: Vec<_> = config.event_types
        .iter()
        .enumerate()
        .map(|(idx, event_type)| {
            let variant_name = format_ident!("Event{}", idx);
            quote! { #variant_name(#event_type) }
        })
        .collect();

    // Generate match arms for process_event
    let process_event_arms: Vec<_> = config.event_types
        .iter()
        .enumerate()
        .map(|(idx, event_type)| {
            let variant_name = format_ident!("Event{}", idx);
            let method_name = event_type_to_method_name(event_type);
            quote! {
                #enum_name::#variant_name(event) => self.#method_name(event).await
            }
        })
        .collect();

    // Generate next_message implementation using embassy_futures::select
    let next_message_impl = generate_next_message(&config.event_types, &enum_name);

    // Generate PollingController implementation if poll_interval is specified
    let polling_controller_impl = if let Some(interval_ms) = config.poll_interval_ms {
        quote! {
            impl #impl_generics ::rmk::controller::PollingController for #struct_name #ty_generics #where_clause {
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

    // Generate the complete output
    let expanded = quote! {
        #(#attrs)*
        #vis #struct_def

        // Internal enum for event routing (needs same visibility as struct for public trait implementation)
        #vis enum #enum_name {
            #(#enum_variants),*
        }

        impl #impl_generics ::rmk::controller::Controller for #struct_name #ty_generics #where_clause {
            type Event = #enum_name;

            async fn process_event(&mut self, event: Self::Event) {
                match event {
                    #(#process_event_arms),*
                }
            }

            #next_message_impl
        }

        #polling_controller_impl
    };

    expanded.into()
}

/// Controller attribute configuration
struct ControllerConfig {
    event_types: Vec<Path>,
    poll_interval_ms: Option<u64>,
}

/// Parse #[controller] subscribe attribute
fn parse_controller_attributes(attr: proc_macro::TokenStream) -> ControllerConfig {
    use syn::punctuated::Punctuated;
    use syn::{ExprArray, Token};

    let mut event_types = Vec::new();
    let mut poll_interval_ms = None;

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
                    } else if nv.path.is_ident("poll_interval") {
                        // Parse poll_interval as integer (milliseconds)
                        if let syn::Expr::Lit(syn::ExprLit { 
                            lit: syn::Lit::Int(lit_int), .. 
                        }) = nv.value {
                            poll_interval_ms = Some(lit_int.base10_parse::<u64>().expect("poll_interval must be a valid u64"));
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

/// Convert event type to handler method name: BatteryLevelEvent -> on_battery_level_event
fn event_type_to_method_name(path: &Path) -> syn::Ident {
    let type_name = path.segments.last().unwrap().ident.to_string();

    // Remove "Event" suffix if present
    let base_name = type_name.strip_suffix("Event").unwrap_or(&type_name);

    // Convert CamelCase to snake_case
    let snake_case = to_snake_case(base_name);

    // Add "on_" prefix and "_event" suffix
    format_ident!("on_{}_event", snake_case)
}

/// Convert CamelCase to snake_case
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];

        if c.is_uppercase() {
            // Add underscore before uppercase letter if:
            // 1. Not at start (i > 0)
            // 2. Previous char is lowercase OR
            // 3. Next char exists and is lowercase (end of acronym: "HTMLParser" -> "html_parser")
            let add_underscore =
                i > 0 && (chars[i - 1].is_lowercase() || (i + 1 < chars.len() && chars[i + 1].is_lowercase()));

            if add_underscore {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }

    result
}

/// Generate next_message using select_biased for concurrent event polling
fn generate_next_message(event_types: &[Path], enum_name: &syn::Ident) -> proc_macro2::TokenStream {
    let num_events = event_types.len();

    // Create subscriber variable names
    let sub_vars: Vec<_> = (0..num_events).map(|i| format_ident!("sub{}", i)).collect();

    // Create subscriber initializations
    let sub_inits: Vec<_> = event_types
        .iter()
        .zip(&sub_vars)
        .map(|(event_type, sub_var)| {
            quote! {
                let mut #sub_var = <#event_type as ::rmk::event::ControllerEventTrait>::subscriber();
            }
        })
        .collect();

    // Generate select_biased! arms for each event
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_snake_case() {
        // Basic cases
        assert_eq!(to_snake_case("Battery"), "battery");
        assert_eq!(to_snake_case("ChargingState"), "charging_state");
        assert_eq!(to_snake_case("KeyboardIndicator"), "keyboard_indicator");

        // Acronyms should stay together
        assert_eq!(to_snake_case("BLE"), "ble");
        assert_eq!(to_snake_case("WPM"), "wpm");
        assert_eq!(to_snake_case("USB"), "usb");

        // Mixed acronyms and words
        assert_eq!(to_snake_case("HTMLParser"), "html_parser");
        assert_eq!(to_snake_case("BLEConnection"), "ble_connection");
        assert_eq!(to_snake_case("parseHTMLString"), "parse_html_string");
    }
}
