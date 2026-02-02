//! Shared logic for generating combined Runnable implementations.
//!
//! This module provides a unified function for generating `Runnable` trait implementations
//! that combine multiple macro functionalities (input_device, input_processor, controller).

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashSet;
use syn::parse::Parser;
use syn::{Attribute, ExprArray, GenericParam, Meta, Path};

/// Configuration for controller subscription
pub struct ControllerConfig {
    pub event_types: Vec<Path>,
    pub poll_interval_ms: Option<u64>,
}

/// Configuration for input device publishing
pub struct InputDeviceConfig {
    pub event_type: Path,
}

/// Configuration for input processor subscription
pub struct InputProcessorConfig {
    pub event_types: Vec<Path>,
}

/// Deduplicate generic parameters by name.
///
/// When a struct has cfg-conditional generic bounds like:
/// ```rust,ignore
/// struct Foo<
///     #[cfg(feature = "a")] T: Trait1,
///     #[cfg(not(feature = "a"))] T: Trait2,
/// >
/// ```
///
/// syn parses this as two separate parameters with the same name.
/// This function extracts unique parameter names for use in type position (e.g., `Foo<T>`).
pub fn deduplicate_type_generics(generics: &syn::Generics) -> TokenStream {
    let mut seen = HashSet::new();
    let mut unique_params = Vec::new();

    for param in &generics.params {
        let name = match param {
            GenericParam::Type(t) => t.ident.to_string(),
            GenericParam::Lifetime(l) => l.lifetime.to_string(),
            GenericParam::Const(c) => c.ident.to_string(),
        };

        if seen.insert(name) {
            // First occurrence - add to unique params
            match param {
                GenericParam::Type(t) => {
                    let ident = &t.ident;
                    unique_params.push(quote! { #ident });
                }
                GenericParam::Lifetime(l) => {
                    let lifetime = &l.lifetime;
                    unique_params.push(quote! { #lifetime });
                }
                GenericParam::Const(c) => {
                    let ident = &c.ident;
                    unique_params.push(quote! { #ident });
                }
            }
        }
    }

    if unique_params.is_empty() {
        quote! {}
    } else {
        quote! { < #(#unique_params),* > }
    }
}

/// Convert CamelCase to snake_case
pub fn to_snake_case(s: &str) -> String {
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

/// Convert CamelCase to UPPER_SNAKE_CASE for channel names
pub fn to_upper_snake_case(s: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];

        if c.is_uppercase() {
            let add_underscore = i > 0
                && (chars[i - 1].is_lowercase()
                    || (i + 1 < chars.len() && chars[i + 1].is_lowercase()));

            if add_underscore {
                result.push('_');
            }
            result.push(c);
        } else {
            result.push(c.to_ascii_uppercase());
        }
    }

    result
}

/// Check if a type has a specific derive attribute (e.g., Clone, Copy)
pub fn has_derive(attrs: &[Attribute], derive_name: &str) -> bool {
    attrs.iter().any(|attr| {
        if attr.path().is_ident("derive")
            && let Meta::List(meta_list) = &attr.meta
        {
            return meta_list.tokens.to_string().contains(derive_name);
        }
        false
    })
}

/// Reconstruct type definition (struct or enum) from DeriveInput.
/// Returns TokenStream for the type definition without attributes.
pub fn reconstruct_type_def(input: &syn::DeriveInput) -> TokenStream {
    let type_name = &input.ident;
    let generics = &input.generics;
    let (_, _, where_clause) = generics.split_for_impl();

    match &input.data {
        syn::Data::Struct(data_struct) => match &data_struct.fields {
            syn::Fields::Named(fields) => {
                quote! { struct #type_name #generics #fields #where_clause }
            }
            syn::Fields::Unnamed(fields) => {
                quote! { struct #type_name #generics #fields #where_clause ; }
            }
            syn::Fields::Unit => {
                quote! { struct #type_name #generics #where_clause ; }
            }
        },
        syn::Data::Enum(data_enum) => {
            let variants = &data_enum.variants;
            quote! { enum #type_name #generics #where_clause { #variants } }
        }
        syn::Data::Union(_) => {
            panic!("Unions are not supported")
        }
    }
}

/// Check if a struct has the runnable_generated marker attribute.
/// This marker is used to prevent multiple Runnable implementations when
/// multiple macros (input_device, input_processor, controller) are combined.
pub fn has_runnable_marker(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        let path = attr.path();
        path.is_ident("runnable_generated")
            || (path.segments.len() == 2
                && path.segments[0].ident == "rmk"
                && path.segments[1].ident == "runnable_generated")
    })
}

/// Convert event type to read method name: BatteryEvent -> read_battery_event
pub fn event_type_to_read_method_name(path: &Path) -> syn::Ident {
    let type_name = path.segments.last().unwrap().ident.to_string();

    // Remove "Event" suffix if present
    let base_name = type_name.strip_suffix("Event").unwrap_or(&type_name);

    // Convert CamelCase to snake_case
    let snake_case = to_snake_case(base_name);

    // Add "read_" prefix and "_event" suffix
    format_ident!("read_{}_event", snake_case)
}

/// Generate unified Runnable implementation for any combination of input_device, input_processor, and controller.
///
/// This function generates a `Runnable::run()` implementation that handles:
/// - InputDevice: reads events via `read_xxx_event()` method and publishes them
/// - InputProcessor: subscribes to input events and processes them
/// - Controller: subscribes to controller events and optionally handles polling
///
/// The generated code uses `select_biased!` to concurrently handle all event sources.
///
/// Note: `input_device_config` and `input_processor_config` are mutually exclusive.
pub fn generate_runnable(
    struct_name: &syn::Ident,
    generics: &syn::Generics,
    where_clause: Option<&syn::WhereClause>,
    input_device_config: Option<&InputDeviceConfig>,
    input_processor_config: Option<&InputProcessorConfig>,
    controller_config: Option<&ControllerConfig>,
) -> TokenStream {
    let (impl_generics, _, _) = generics.split_for_impl();
    let ty_generics = deduplicate_type_generics(generics);

    // Validate mutual exclusivity
    if input_device_config.is_some() && input_processor_config.is_some() {
        panic!("input_device and input_processor are mutually exclusive");
    }

    // Collect all select arms and subscriber definitions
    let mut sub_defs: Vec<TokenStream> = Vec::new();
    let mut select_arms: Vec<TokenStream> = Vec::new();
    let mut use_statements: Vec<TokenStream> = Vec::new();

    // Handle input_device
    if let Some(device_config) = input_device_config {
        let read_method = event_type_to_read_method_name(&device_config.event_type);
        use_statements.push(quote! { use ::rmk::event::publish_input_event_async; });
        select_arms.push(quote! {
            event = self.#read_method().fuse() => {
                publish_input_event_async(event).await;
            }
        });
    }

    // Handle input_processor
    if let Some(processor_config) = input_processor_config {
        let proc_enum_name = format_ident!("{}EventEnum", struct_name);
        use_statements.push(quote! { use ::rmk::event::InputEvent; });
        use_statements.push(quote! { use ::rmk::input_device::InputProcessor; });

        for (idx, event_type) in processor_config.event_types.iter().enumerate() {
            let sub_name = format_ident!("proc_sub{}", idx);
            let variant_name = format_ident!("Event{}", idx);
            sub_defs.push(quote! {
                let mut #sub_name = <#event_type as ::rmk::event::InputEvent>::input_subscriber();
            });
            select_arms.push(quote! {
                proc_event = #sub_name.next_event().fuse() => {
                    self.process(#proc_enum_name::#variant_name(proc_event)).await;
                }
            });
        }
    }

    // Handle controller
    let has_polling = controller_config
        .as_ref()
        .and_then(|c| c.poll_interval_ms)
        .is_some();

    if let Some(ctrl_config) = controller_config {
        let ctrl_enum_name = format_ident!("{}EventEnum", struct_name);
        use_statements.push(quote! { use ::rmk::event::ControllerEvent; });
        use_statements.push(quote! { use ::rmk::controller::Controller; });

        for (idx, ctrl_event_type) in ctrl_config.event_types.iter().enumerate() {
            let sub_name = format_ident!("ctrl_sub{}", idx);
            let variant_name = format_ident!("Event{}", idx);
            sub_defs.push(quote! {
                let mut #sub_name = <#ctrl_event_type as ::rmk::event::ControllerEvent>::controller_subscriber();
            });
            select_arms.push(quote! {
                ctrl_event = #sub_name.next_event().fuse() => {
                    <Self as ::rmk::controller::Controller>::process_event(self, #ctrl_enum_name::#variant_name(ctrl_event)).await;
                }
            });
        }
    }

    // Handle standalone controller case (no input_device or input_processor)
    if input_device_config.is_none() && input_processor_config.is_none() && controller_config.is_some() {
        // Use the simpler event_loop/polling_loop approach for standalone controller
        if has_polling {
            return quote! {
                impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
                    async fn run(&mut self) -> ! {
                        use ::rmk::controller::PollingController;
                        self.polling_loop().await
                    }
                }
            };
        } else {
            return quote! {
                impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
                    async fn run(&mut self) -> ! {
                        use ::rmk::controller::EventController;
                        self.event_loop().await
                    }
                }
            };
        }
    }

    // Common use statements
    use_statements.push(quote! { use ::rmk::event::EventSubscriber; });
    use_statements.push(quote! { use ::rmk::futures::FutureExt; });

    // Generate polling-related code if needed
    if has_polling {
        let interval_ms = controller_config.as_ref().unwrap().poll_interval_ms.unwrap();
        use_statements.push(quote! { use ::rmk::controller::PollingController; });

        let timer_init = quote! {
            let mut last = ::embassy_time::Instant::now();
        };

        let timer_arm = quote! {
            _ = timer.fuse() => {
                <Self as PollingController>::update(self).await;
                last = ::embassy_time::Instant::now();
            }
        };

        quote! {
            impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
                async fn run(&mut self) -> ! {
                    #(#use_statements)*

                    #(#sub_defs)*
                    #timer_init

                    loop {
                        let elapsed = last.elapsed();
                        let interval = ::embassy_time::Duration::from_millis(#interval_ms);
                        let timer = ::embassy_time::Timer::after(
                            interval.checked_sub(elapsed).unwrap_or(::embassy_time::Duration::MIN)
                        );

                        ::rmk::futures::select_biased! {
                            #(#select_arms)*
                            #timer_arm
                        }
                    }
                }
            }
        }
    } else {
        quote! {
            impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
                async fn run(&mut self) -> ! {
                    #(#use_statements)*

                    #(#sub_defs)*

                    loop {
                        ::rmk::futures::select_biased! {
                            #(#select_arms)*
                        }
                    }
                }
            }
        }
    }
}

/// Parse controller config from attribute tokens.
/// Extracts `subscribe = [...]` and optional `poll_interval = N`.
pub fn parse_controller_config(tokens: impl Into<TokenStream>) -> ControllerConfig {
    use syn::punctuated::Punctuated;
    use syn::Token;

    let mut event_types = Vec::new();
    let mut poll_interval_ms = None;

    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let tokens: TokenStream = tokens.into();

    if let Ok(parsed) = parser.parse2(tokens) {
        for meta in parsed {
            if let Meta::NameValue(nv) = meta {
                if nv.path.is_ident("subscribe") {
                    if let syn::Expr::Array(ExprArray { elems, .. }) = nv.value {
                        for elem in elems {
                            if let syn::Expr::Path(expr_path) = elem {
                                event_types.push(expr_path.path);
                            }
                        }
                    }
                } else if nv.path.is_ident("poll_interval") {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Int(lit_int),
                        ..
                    }) = nv.value
                    {
                        poll_interval_ms = lit_int.base10_parse::<u64>().ok();
                    }
                }
            }
        }
    }

    ControllerConfig {
        event_types,
        poll_interval_ms,
    }
}

/// Parse input_device config from attribute tokens.
/// Extracts `publish = EventType`.
pub fn parse_input_device_config(tokens: impl Into<TokenStream>) -> Option<InputDeviceConfig> {
    use syn::punctuated::Punctuated;
    use syn::Token;

    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let tokens: TokenStream = tokens.into();

    if let Ok(parsed) = parser.parse2(tokens) {
        for meta in parsed {
            if let Meta::NameValue(nv) = meta {
                if nv.path.is_ident("publish") {
                    if let syn::Expr::Path(expr_path) = nv.value {
                        return Some(InputDeviceConfig {
                            event_type: expr_path.path,
                        });
                    }
                }
            }
        }
    }

    None
}

/// Parse input_processor config from attribute tokens.
/// Extracts `subscribe = [...]`.
pub fn parse_input_processor_config(tokens: impl Into<TokenStream>) -> InputProcessorConfig {
    use syn::punctuated::Punctuated;
    use syn::Token;

    let mut event_types = Vec::new();

    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let tokens: TokenStream = tokens.into();

    if let Ok(parsed) = parser.parse2(tokens) {
        for meta in parsed {
            if let Meta::NameValue(nv) = meta {
                if nv.path.is_ident("subscribe") {
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

    InputProcessorConfig { event_types }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("Battery"), "battery");
        assert_eq!(to_snake_case("ChargingState"), "charging_state");
        assert_eq!(to_snake_case("USB"), "usb");
        assert_eq!(to_snake_case("USBKey"), "usb_key");
        assert_eq!(to_snake_case("HTMLParser"), "html_parser");
    }

    #[test]
    fn test_to_upper_snake_case() {
        assert_eq!(to_upper_snake_case("KeyEvent"), "KEY_EVENT");
        assert_eq!(to_upper_snake_case("ModifierEvent"), "MODIFIER_EVENT");
        assert_eq!(to_upper_snake_case("TouchpadEvent"), "TOUCHPAD_EVENT");
        assert_eq!(to_upper_snake_case("USBEvent"), "USB_EVENT");
        assert_eq!(to_upper_snake_case("HIDDevice"), "HID_DEVICE");
    }

    #[test]
    fn test_event_type_to_read_method_name() {
        use syn::parse_quote;

        let path: Path = parse_quote!(BatteryEvent);
        assert_eq!(event_type_to_read_method_name(&path).to_string(), "read_battery_event");

        let path: Path = parse_quote!(ChargingStateEvent);
        assert_eq!(
            event_type_to_read_method_name(&path).to_string(),
            "read_charging_state_event"
        );
    }
}
