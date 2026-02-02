pub(crate) mod event;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use rmk_config::{ChipModel, KeyboardTomlConfig, PinConfig};
use syn::parse::Parser;
use syn::{Attribute, DeriveInput, ItemMod, Meta, Path, parse_macro_input};

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
    let config = match parse_controller_attributes(attr) {
        Ok(config) => config,
        Err(err) => return err.to_compile_error().into(),
    };

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

    let has_marker = has_runnable_marker(attrs);
    let input_device_attr = if has_marker { None } else { find_attr(attrs, "input_device") };
    let input_processor_attr = if has_marker { None } else { find_attr(attrs, "input_processor") };

    if input_device_attr.is_some() && input_processor_attr.is_some() {
        return syn::Error::new_spanned(
            input,
            "#[controller] cannot be combined with both #[input_device] and #[input_processor]",
        )
        .to_compile_error()
        .into();
    }

    let input_device_config = if let Some(attr) = input_device_attr {
        match parse_input_device_attribute(attr) {
            Ok(config) => Some(config),
            Err(err) => return err.to_compile_error().into(),
        }
    } else {
        None
    };

    let input_processor_config = if let Some(attr) = input_processor_attr {
        match parse_input_processor_attribute(attr) {
            Ok(config) => Some(config),
            Err(err) => return err.to_compile_error().into(),
        }
    } else {
        None
    };

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
    let enum_variants: Vec<_> = config
        .event_types
        .iter()
        .enumerate()
        .map(|(idx, event_type)| {
            let variant_name = format_ident!("Event{}", idx);
            quote! { #variant_name(#event_type) }
        })
        .collect();

    // Generate match arms for process_event
    let process_event_arms: Vec<_> = config
        .event_types
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

    let from_impls: Vec<_> = config
        .event_types
        .iter()
        .enumerate()
        .map(|(idx, event_type)| {
            let variant_name = format_ident!("Event{}", idx);
            quote! {
                impl #impl_generics From<#event_type> for #enum_name #ty_generics #where_clause {
                    fn from(e: #event_type) -> Self {
                        #enum_name::#variant_name(e)
                    }
                }
            }
        })
        .collect();

    let runnable_impl = if has_marker {
        quote! {}
    } else if let Some(input_device_config) = input_device_config {
        let read_method = event_type_to_read_method_name(&input_device_config.event_type);
        let ctrl_subs_defs: Vec<_> = config
            .event_types
            .iter()
            .enumerate()
            .map(|(idx, event_type)| {
                let sub_name = format_ident!("ctrl_sub{}", idx);
                quote! { let mut #sub_name = <#event_type as ::rmk::event::ControllerEvent>::controller_subscriber(); }
            })
            .collect();

        let ctrl_subs_arms: Vec<_> = config
            .event_types
            .iter()
            .enumerate()
            .map(|(idx, _event_type)| {
                let sub_name = format_ident!("ctrl_sub{}", idx);
                quote! {
                    ctrl_event = #sub_name.next_event().fuse() => {
                        <Self as ::rmk::controller::Controller>::process_event(self, ctrl_event.into()).await;
                    },
                }
            })
            .collect();

        let polling_setup = if config.poll_interval_ms.is_some() {
            quote! { let mut last = ::embassy_time::Instant::now(); }
        } else {
            quote! {}
        };

        let timer_setup = if config.poll_interval_ms.is_some() {
            quote! {
                let elapsed = last.elapsed();
                let interval = <Self as ::rmk::controller::PollingController>::interval(self);
                let timer = ::embassy_time::Timer::after(
                    interval
                        .checked_sub(elapsed)
                        .unwrap_or(::embassy_time::Duration::MIN)
                );
            }
        } else {
            quote! {}
        };

        let timer_arm = if config.poll_interval_ms.is_some() {
            quote! {
                _ = timer.fuse() => {
                    <Self as ::rmk::controller::PollingController>::update(self).await;
                    last = ::embassy_time::Instant::now();
                },
            }
        } else {
            quote! {}
        };

        quote! {
            impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
                async fn run(&mut self) -> ! {
                    use ::futures::FutureExt;
                    use ::rmk::event::{ControllerEvent, EventSubscriber, publish_input_event_async};
                    #polling_setup
                    #(#ctrl_subs_defs)*

                    loop {
                        #timer_setup
                        ::futures::select_biased! {
                            event = self.#read_method().fuse() => {
                                publish_input_event_async(event).await;
                            },
                            #(#ctrl_subs_arms)*
                            #timer_arm
                        }
                    }
                }
            }
        }
    } else if let Some(input_processor_config) = input_processor_config {
        let processor_enum = format_ident!("{}InputEventEnum", struct_name);
        let input_subs_defs: Vec<_> = input_processor_config
            .event_types
            .iter()
            .enumerate()
            .map(|(idx, event_type)| {
                let sub_name = format_ident!("sub{}", idx);
                quote! { let mut #sub_name = <#event_type as ::rmk::event::InputEvent>::input_subscriber(); }
            })
            .collect();

        let input_subs_arms: Vec<_> = input_processor_config
            .event_types
            .iter()
            .enumerate()
            .map(|(idx, _event_type)| {
                let sub_name = format_ident!("sub{}", idx);
                let variant_name = format_ident!("Event{}", idx);
                quote! {
                    event = #sub_name.next_event().fuse() => {
                        self.process(#processor_enum::#variant_name(event)).await;
                    },
                }
            })
            .collect();

        let ctrl_subs_defs: Vec<_> = config
            .event_types
            .iter()
            .enumerate()
            .map(|(idx, event_type)| {
                let sub_name = format_ident!("ctrl_sub{}", idx);
                quote! { let mut #sub_name = <#event_type as ::rmk::event::ControllerEvent>::controller_subscriber(); }
            })
            .collect();

        let ctrl_subs_arms: Vec<_> = config
            .event_types
            .iter()
            .enumerate()
            .map(|(idx, _event_type)| {
                let sub_name = format_ident!("ctrl_sub{}", idx);
                quote! {
                    ctrl_event = #sub_name.next_event().fuse() => {
                        <Self as ::rmk::controller::Controller>::process_event(self, ctrl_event.into()).await;
                    },
                }
            })
            .collect();

        let polling_setup = if config.poll_interval_ms.is_some() {
            quote! { let mut last = ::embassy_time::Instant::now(); }
        } else {
            quote! {}
        };

        let timer_setup = if config.poll_interval_ms.is_some() {
            quote! {
                let elapsed = last.elapsed();
                let interval = <Self as ::rmk::controller::PollingController>::interval(self);
                let timer = ::embassy_time::Timer::after(
                    interval
                        .checked_sub(elapsed)
                        .unwrap_or(::embassy_time::Duration::MIN)
                );
            }
        } else {
            quote! {}
        };

        let timer_arm = if config.poll_interval_ms.is_some() {
            quote! {
                _ = timer.fuse() => {
                    <Self as ::rmk::controller::PollingController>::update(self).await;
                    last = ::embassy_time::Instant::now();
                },
            }
        } else {
            quote! {}
        };

        quote! {
            impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
                async fn run(&mut self) -> ! {
                    use ::futures::FutureExt;
                    use ::rmk::event::{ControllerEvent, EventSubscriber, InputEvent};
                    use ::rmk::input_device::InputProcessor;
                    #polling_setup
                    #(#input_subs_defs)*
                    #(#ctrl_subs_defs)*

                    loop {
                        #timer_setup
                        ::futures::select_biased! {
                            #(#input_subs_arms)*
                            #(#ctrl_subs_arms)*
                            #timer_arm
                        }
                    }
                }
            }
        }
    } else {
        let runnable_body = if config.poll_interval_ms.is_some() {
            quote! {
                use ::rmk::controller::PollingController;
                self.polling_loop().await
            }
        } else {
            quote! {
                use ::rmk::controller::EventController;
                self.event_loop().await
            }
        };

        quote! {
            impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
                async fn run(&mut self) -> ! {
                    #runnable_body
                }
            }
        }
    };

    let marker_attr = if has_marker { quote! {} } else { quote! { #[::rmk::runnable_generated] } };

    // Generate the complete output
    let expanded = quote! {
        #(#attrs)*
        #marker_attr
        #vis #struct_def

        // Internal enum for event routing (needs same visibility as struct for public trait implementation)
        #vis enum #enum_name {
            #(#enum_variants),*
        }

        #(#from_impls)*

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

        #runnable_impl
    };

    expanded.into()
}

/// Controller attribute configuration
struct ControllerConfig {
    event_types: Vec<Path>,
    poll_interval_ms: Option<u64>,
}

struct InputDeviceConfig {
    event_type: Path,
}

struct InputProcessorConfig {
    event_types: Vec<Path>,
}

/// Parse #[controller] subscribe attribute
fn parse_controller_attributes(attr: proc_macro::TokenStream) -> Result<ControllerConfig, syn::Error> {
    use syn::punctuated::Punctuated;
    use syn::{Expr, ExprArray, ExprLit, Lit, Token};

    let mut event_types = Vec::new();
    let mut poll_interval_ms = None;

    // Parse as Meta::List containing name-value pairs
    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let attr2: proc_macro2::TokenStream = attr.into();

    let parsed = parser
        .parse2(attr2)
        .map_err(|e| syn::Error::new(proc_macro2::Span::call_site(), format!("Failed to parse controller attributes: {e}")))?;

    for meta in parsed {
        if let Meta::NameValue(nv) = meta {
            if nv.path.is_ident("subscribe") {
                match nv.value {
                    Expr::Array(ExprArray { elems, .. }) => {
                        for elem in elems {
                            if let Expr::Path(expr_path) = elem {
                                event_types.push(expr_path.path);
                            } else {
                                return Err(syn::Error::new_spanned(
                                    elem,
                                    "#[controller] subscribe must contain event types",
                                ));
                            }
                        }
                    }
                    _ => {
                        return Err(syn::Error::new_spanned(
                            nv,
                            "#[controller] subscribe must be an array: subscribe = [EventType1, EventType2]",
                        ));
                    }
                }
            } else if nv.path.is_ident("poll_interval") {
                match nv.value {
                    Expr::Lit(ExprLit { lit: Lit::Int(lit_int), .. }) => {
                        poll_interval_ms = Some(lit_int.base10_parse::<u64>().map_err(|_| {
                            syn::Error::new_spanned(lit_int, "poll_interval must be a valid u64")
                        })?);
                    }
                    _ => {
                        return Err(syn::Error::new_spanned(
                            nv,
                            "poll_interval must be an integer literal (milliseconds)",
                        ));
                    }
                }
            }
        }
    }

    Ok(ControllerConfig {
        event_types,
        poll_interval_ms,
    })
}

fn parse_input_device_attribute(attr: &Attribute) -> Result<InputDeviceConfig, syn::Error> {
    use syn::punctuated::Punctuated;
    use syn::{Expr, Token};

    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let parsed = attr
        .parse_args_with(parser)
        .map_err(|e| syn::Error::new_spanned(attr, format!("Failed to parse input_device attributes: {e}")))?;

    let mut event_type: Option<Path> = None;

    for meta in parsed {
        if let Meta::NameValue(nv) = meta {
            if nv.path.is_ident("publish") {
                if event_type.is_some() {
                    return Err(syn::Error::new_spanned(
                        nv,
                        "#[input_device] supports only one publish event type",
                    ));
                }
                match nv.value {
                    Expr::Path(expr_path) => {
                        event_type = Some(expr_path.path);
                    }
                    Expr::Array(_) => {
                        return Err(syn::Error::new_spanned(
                            nv,
                            "#[input_device] supports a single event type. For multi-event devices, use #[derive(InputEvent)]",
                        ));
                    }
                    _ => {
                        return Err(syn::Error::new_spanned(
                            nv,
                            "#[input_device] expects `publish = EventType`",
                        ));
                    }
                }
            }
        }
    }

    let event_type = event_type.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[input_device] requires `publish = EventType`",
        )
    })?;

    Ok(InputDeviceConfig { event_type })
}

fn parse_input_processor_attribute(attr: &Attribute) -> Result<InputProcessorConfig, syn::Error> {
    use syn::punctuated::Punctuated;
    use syn::{Expr, ExprArray, Token};

    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let parsed = attr
        .parse_args_with(parser)
        .map_err(|e| syn::Error::new_spanned(attr, format!("Failed to parse input_processor attributes: {e}")))?;

    let mut event_types = Vec::new();

    for meta in parsed {
        if let Meta::NameValue(nv) = meta {
            if nv.path.is_ident("subscribe") {
                match nv.value {
                    Expr::Array(ExprArray { elems, .. }) => {
                        for elem in elems {
                            if let Expr::Path(expr_path) = elem {
                                event_types.push(expr_path.path);
                            } else {
                                return Err(syn::Error::new_spanned(
                                    elem,
                                    "#[input_processor] subscribe must contain event types",
                                ));
                            }
                        }
                    }
                    _ => {
                        return Err(syn::Error::new_spanned(
                            nv,
                            "#[input_processor] subscribe must be an array: subscribe = [EventType1, EventType2]",
                        ));
                    }
                }
            }
        }
    }

    Ok(InputProcessorConfig { event_types })
}

fn has_runnable_marker(attrs: &[Attribute]) -> bool {
    attrs.iter().any(is_runnable_marker)
}

fn is_runnable_marker(attr: &Attribute) -> bool {
    let path = attr.path();
    if path.is_ident("runnable_generated") {
        return true;
    }
    if path.segments.len() == 2 {
        let first = &path.segments[0].ident;
        let second = &path.segments[1].ident;
        return first == "rmk" && second == "runnable_generated";
    }
    false
}

fn find_attr<'a>(attrs: &'a [Attribute], name: &str) -> Option<&'a Attribute> {
    attrs.iter().find(|attr| attr.path().is_ident(name))
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

/// Convert event type to read method name: BatteryEvent -> read_battery_event
fn event_type_to_read_method_name(path: &Path) -> syn::Ident {
    let type_name = path.segments.last().unwrap().ident.to_string();
    let base_name = type_name.strip_suffix("Event").unwrap_or(&type_name);
    let snake_case = to_snake_case(base_name);
    format_ident!("read_{}_event", snake_case)
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
                let mut #sub_var = <#event_type as ::rmk::event::ControllerEvent>::controller_subscriber();
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
