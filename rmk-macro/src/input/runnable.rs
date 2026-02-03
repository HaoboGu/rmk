//! Helpers for generating combined Runnable implementations.
//!
//! Provides a unified `Runnable` generator for input_device/input_processor/controller.
//! Keeps combined macro output consistent.

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::{Attribute, ExprArray, GenericParam, Meta, Path};

/// Controller subscription config.
pub struct ControllerConfig {
    pub event_types: Vec<Path>,
    pub poll_interval_ms: Option<u64>,
}

/// Input device publishing config.
pub struct InputDeviceConfig {
    pub event_type: Path,
}

/// Input processor subscription config.
pub struct InputProcessorConfig {
    pub event_types: Vec<Path>,
}

/// Deduplicate generic parameters by name.
/// Handles cfg-conditional generics that repeat the same name.
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
            // First occurrence.
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

/// Convert CamelCase to snake_case.
pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];

        if c.is_uppercase() {
            // Add underscore before uppercase when:
            // 1) not at start
            // 2) previous is lowercase
            // 3) next is lowercase (acronym end)
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

/// Convert CamelCase to UPPER_SNAKE_CASE.
pub fn to_upper_snake_case(s: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];

        if c.is_uppercase() {
            let add_underscore =
                i > 0 && (chars[i - 1].is_lowercase() || (i + 1 < chars.len() && chars[i + 1].is_lowercase()));

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

/// Check if a type derives a trait (e.g., Clone, Copy).
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

/// Reconstruct a struct/enum definition.
/// Returns a TokenStream without attributes.
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

/// Check for the runnable_generated marker.
/// Prevents duplicate Runnable impls when macros combine.
pub fn has_runnable_marker(attrs: &[Attribute]) -> bool {
    attrs.iter().any(is_runnable_generated_attr)
}

/// Check runnable_generated attribute.
/// Accepts `#[runnable_generated]` and `#[rmk::runnable_generated]`.
pub fn is_runnable_generated_attr(attr: &Attribute) -> bool {
    let path = attr.path();
    path.is_ident("runnable_generated")
        || (path.segments.len() == 2
            && path.segments[0].ident == "rmk"
            && path.segments[1].ident == "runnable_generated")
}

/// Convert event type to read method name (BatteryEvent -> read_battery_event).
pub fn event_type_to_read_method_name(path: &Path) -> syn::Ident {
    let type_name = path.segments.last().unwrap().ident.to_string();

    // Strip "Event" suffix if present.
    let base_name = type_name.strip_suffix("Event").unwrap_or(&type_name);

    // CamelCase -> snake_case.
    let snake_case = to_snake_case(base_name);

    // Add "read_" prefix and "_event" suffix.
    format_ident!("read_{}_event", snake_case)
}

/// Convert event type to handler method name (BatteryEvent -> on_battery_event).
pub fn event_type_to_handler_method_name(path: &Path) -> syn::Ident {
    let type_name = path.segments.last().unwrap().ident.to_string();

    // Strip "Event" suffix if present.
    let base_name = type_name.strip_suffix("Event").unwrap_or(&type_name);

    // CamelCase -> snake_case.
    let snake_case = to_snake_case(base_name);

    // Add "on_" prefix and "_event" suffix.
    format_ident!("on_{}_event", snake_case)
}

/// Generate a unified `Runnable` impl for input_device/input_processor/controller.
///
/// Handles:
/// - InputDevice: read_event + publish
/// - InputProcessor: subscribe + process
/// - Controller: subscribe + optional polling
///
/// Uses select_biased! when multiplexing multiple sources.
/// `input_device_config` and `input_processor_config` are mutually exclusive.
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
    let runnable_impl = |body: TokenStream| {
        quote! {
            impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
                async fn run(&mut self) -> ! {
                    #body
                }
            }
        }
    };

    // Enforce mutual exclusivity.
    if input_device_config.is_some() && input_processor_config.is_some() {
        panic!("input_device and input_processor are mutually exclusive");
    }

    // Collect select arms and subscriber definitions.
    let mut sub_defs: Vec<TokenStream> = Vec::new();
    let mut select_arms: Vec<TokenStream> = Vec::new();
    let mut select_match_arms: Vec<TokenStream> = Vec::new();
    let mut use_statements: Vec<TokenStream> = Vec::new();

    let needs_split_select = input_device_config.is_some() && controller_config.is_some();
    let select_enum_name = if needs_split_select {
        Some(format_ident!("__RmkSelectEvent{}", struct_name))
    } else {
        None
    };
    let mut input_event_type: Option<syn::Path> = None;
    let mut ctrl_enum_name: Option<syn::Ident> = None;

    // Handle input_device.
    if let Some(device_config) = input_device_config {
        input_event_type = Some(device_config.event_type.clone());
        use_statements.push(quote! { use ::rmk::event::publish_input_event_async; });
        use_statements.push(quote! { use ::rmk::input_device::InputDevice; });
        if needs_split_select {
            let select_enum_name = select_enum_name.as_ref().unwrap();
            select_arms.push(quote! {
                event = self.read_event().fuse() => #select_enum_name::Input(event)
            });
            select_match_arms.push(quote! {
                #select_enum_name::Input(event) => {
                    publish_input_event_async(event).await;
                }
            });
        } else {
            select_arms.push(quote! {
                event = self.read_event().fuse() => {
                    publish_input_event_async(event).await;
                }
            });
        }
    }

    // Handle input_processor.
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

    // Handle controller.
    let has_polling = controller_config.as_ref().and_then(|c| c.poll_interval_ms).is_some();

    if let Some(ctrl_config) = controller_config {
        let ctrl_enum = format_ident!("{}EventEnum", struct_name);
        ctrl_enum_name = Some(ctrl_enum.clone());
        use_statements.push(quote! { use ::rmk::event::ControllerEvent; });
        use_statements.push(quote! { use ::rmk::controller::Controller; });

        for (idx, ctrl_event_type) in ctrl_config.event_types.iter().enumerate() {
            let sub_name = format_ident!("ctrl_sub{}", idx);
            let variant_name = format_ident!("Event{}", idx);
            sub_defs.push(quote! {
                let mut #sub_name = <#ctrl_event_type as ::rmk::event::ControllerEvent>::controller_subscriber();
            });
            if needs_split_select {
                let select_enum_name = select_enum_name.as_ref().unwrap();
                select_arms.push(quote! {
                    ctrl_event = #sub_name.next_event().fuse() => #select_enum_name::Controller(#ctrl_enum::#variant_name(ctrl_event))
                });
            } else {
                select_arms.push(quote! {
                    ctrl_event = #sub_name.next_event().fuse() => {
                        <Self as ::rmk::controller::Controller>::process_event(self, #ctrl_enum::#variant_name(ctrl_event)).await;
                    }
                });
            }
        }

        if needs_split_select {
            let select_enum_name = select_enum_name.as_ref().unwrap();
            select_match_arms.push(quote! {
                #select_enum_name::Controller(event) => {
                    <Self as ::rmk::controller::Controller>::process_event(self, event).await;
                }
            });
        }
    }

    // Standalone controller (no input_device/input_processor).
    if input_device_config.is_none() && input_processor_config.is_none() && controller_config.is_some() {
        // Use event_loop/polling_loop directly.
        if has_polling {
            return runnable_impl(quote! {
                use ::rmk::controller::PollingController;
                self.polling_loop().await
            });
        } else {
            return runnable_impl(quote! {
                use ::rmk::controller::EventController;
                self.event_loop().await
            });
        }
    }

    // Standalone input_device.
    if input_device_config.is_some() && input_processor_config.is_none() && controller_config.is_none() {
        return runnable_impl(quote! {
            use ::rmk::event::publish_input_event_async;
            use ::rmk::input_device::InputDevice;
            loop {
                let event = self.read_event().await;
                publish_input_event_async(event).await;
            }
        });
    }

    // Standalone input_processor.
    if input_device_config.is_none() && controller_config.is_none() && let Some(processor_config) = input_processor_config {
        let proc_enum_name = format_ident!("{}EventEnum", struct_name);

        // Single event type: avoid select_biased.
        if processor_config.event_types.len() == 1 {
            let event_type = &processor_config.event_types[0];
            return runnable_impl(quote! {
                use ::rmk::event::InputEvent;
                use ::rmk::event::EventSubscriber;
                use ::rmk::input_device::InputProcessor;

                let mut sub = <#event_type as ::rmk::event::InputEvent>::input_subscriber();

                loop {
                    let event = sub.next_event().await;
                    self.process(#proc_enum_name::Event0(event)).await;
                }
            });
        }
    }

    // Common use statements.
    if !sub_defs.is_empty() {
        use_statements.push(quote! { use ::rmk::event::EventSubscriber; });
    }
    use_statements.push(quote! { use ::rmk::futures::FutureExt; });

    // Generate polling-related code if needed.
    if has_polling {
        let interval_ms = controller_config.as_ref().unwrap().poll_interval_ms.unwrap();
        use_statements.push(quote! { use ::rmk::controller::PollingController; });

        let timer_init = quote! {
            let mut last = ::embassy_time::Instant::now();
        };

        if needs_split_select {
            let select_enum_name = select_enum_name.as_ref().unwrap();
            let input_event_type = input_event_type.as_ref().unwrap();
            let ctrl_enum_name = ctrl_enum_name.as_ref().unwrap();
            let select_enum_def = quote! {
                enum #select_enum_name {
                    Input(#input_event_type),
                    Controller(#ctrl_enum_name),
                    Timer,
                }
            };

            let timer_arm = quote! {
                _ = timer.fuse() => #select_enum_name::Timer
            };

            select_match_arms.push(quote! {
                #select_enum_name::Timer => {
                    <Self as PollingController>::update(self).await;
                    last = ::embassy_time::Instant::now();
                }
            });

            quote! {
                impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
                    async fn run(&mut self) -> ! {
                        #(#use_statements)*
                        #select_enum_def

                        #(#sub_defs)*
                        #timer_init

                        loop {
                            let elapsed = last.elapsed();
                            let interval = ::embassy_time::Duration::from_millis(#interval_ms);
                            let timer = ::embassy_time::Timer::after(
                                interval.checked_sub(elapsed).unwrap_or(::embassy_time::Duration::MIN)
                            );

                            let select_result = {
                                ::rmk::futures::select_biased! {
                                    #(#select_arms)*
                                    #timer_arm
                                }
                            };

                            match select_result {
                                #(#select_match_arms)*
                            }
                        }
                    }
                }
            }
        } else {
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
        }
    } else if needs_split_select {
        let select_enum_name = select_enum_name.as_ref().unwrap();
        let input_event_type = input_event_type.as_ref().unwrap();
        let ctrl_enum_name = ctrl_enum_name.as_ref().unwrap();
        let select_enum_def = quote! {
            enum #select_enum_name {
                Input(#input_event_type),
                Controller(#ctrl_enum_name),
            }
        };

        quote! {
            impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
                async fn run(&mut self) -> ! {
                    #(#use_statements)*
                    #select_enum_def

                    #(#sub_defs)*

                    loop {
                        let select_result = {
                            ::rmk::futures::select_biased! {
                                #(#select_arms)*
                            }
                        };

                        match select_result {
                            #(#select_match_arms)*
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
    use syn::Token;
    use syn::punctuated::Punctuated;

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
                } else if nv.path.is_ident("poll_interval")
                    && let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Int(lit_int),
                        ..
                    }) = nv.value
                {
                    poll_interval_ms = lit_int.base10_parse::<u64>().ok();
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
    use syn::Token;
    use syn::punctuated::Punctuated;

    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let tokens: TokenStream = tokens.into();

    if let Ok(parsed) = parser.parse2(tokens) {
        for meta in parsed {
            if let Meta::NameValue(nv) = meta
                && nv.path.is_ident("publish")
                && let syn::Expr::Path(expr_path) = nv.value
            {
                return Some(InputDeviceConfig {
                    event_type: expr_path.path,
                });
            }
        }
    }

    None
}

/// Parse input_processor config from attribute tokens.
/// Extracts `subscribe = [...]`.
pub fn parse_input_processor_config(tokens: impl Into<TokenStream>) -> InputProcessorConfig {
    use syn::Token;
    use syn::punctuated::Punctuated;

    let mut event_types = Vec::new();

    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let tokens: TokenStream = tokens.into();

    if let Ok(parsed) = parser.parse2(tokens) {
        for meta in parsed {
            if let Meta::NameValue(nv) = meta
                && nv.path.is_ident("subscribe")
                && let syn::Expr::Array(ExprArray { elems, .. }) = nv.value
            {
                for elem in elems {
                    if let syn::Expr::Path(expr_path) = elem {
                        event_types.push(expr_path.path);
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
