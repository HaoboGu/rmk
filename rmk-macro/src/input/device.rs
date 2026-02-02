use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::{DeriveInput, Meta, Path, parse_macro_input};

/// Generates InputDevice and Runnable trait implementations for single-event devices.
///
/// This macro is used to define InputDevice structs that publish a single event type:
/// ```rust
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
    let input = parse_macro_input!(item as DeriveInput);

    // Parse attributes to extract event type
    let config = parse_device_attributes(attr);

    // Validate single event type
    if config.event_type.is_none() {
        return syn::Error::new_spanned(
            &input,
            "#[input_device] requires `publish` attribute with a single event type. Use `#[input_device(publish = EventType)]`",
        )
        .to_compile_error()
        .into();
    }

    let event_type = config.event_type.unwrap();

    // Validate input is a struct
    if !matches!(input.data, syn::Data::Struct(_)) {
        return syn::Error::new_spanned(&input, "#[input_device] can only be applied to structs")
            .to_compile_error()
            .into();
    }

    // Check for mutually exclusive attributes
    let has_input_processor = input.attrs.iter().any(|attr| attr.path().is_ident("input_processor"));
    if has_input_processor {
        return syn::Error::new_spanned(
            &input,
            "#[input_device] and #[input_processor] are mutually exclusive. A struct cannot be both an input device and an input processor.",
        )
        .to_compile_error()
        .into();
    }

    // Check for runnable_generated marker
    let has_runnable_marker = input.attrs.iter().any(|attr| {
        let path = attr.path();
        path.is_ident("runnable_generated")
            || (path.segments.len() == 2
                && path.segments[0].ident == "rmk"
                && path.segments[1].ident == "runnable_generated")
    });

    // Check for controller attribute (for combined Runnable generation)
    let has_controller = input.attrs.iter().any(|attr| attr.path().is_ident("controller"));

    // Parse controller config if present (for combined Runnable)
    let controller_config = if has_controller {
        input
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("controller"))
            .and_then(|attr| {
                if let Meta::List(meta_list) = &attr.meta {
                    Some(parse_controller_config_from_tokens(meta_list.tokens.clone().into()))
                } else {
                    None
                }
            })
    } else {
        None
    };

    let struct_name = &input.ident;
    let vis = &input.vis;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Filter out input_device attribute and runnable_generated marker from output
    let attrs: Vec<_> = input
        .attrs
        .iter()
        .filter(|attr| {
            let path = attr.path();
            !path.is_ident("input_device")
                && !path.is_ident("runnable_generated")
                && !(path.segments.len() == 2
                    && path.segments[0].ident == "rmk"
                    && path.segments[1].ident == "runnable_generated")
        })
        .collect();

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
    let enum_name = format_ident!("{}InputEventEnum", struct_name);

    // Generate method name from event type
    let method_name = event_type_to_read_method_name(&event_type);

    // Generate Runnable implementation
    let runnable_impl = if has_runnable_marker {
        // Skip Runnable generation if marker is present
        quote! {}
    } else if has_controller {
        // Generate combined Runnable for input_device + controller
        generate_combined_runnable_input_device_controller(
            struct_name,
            &impl_generics,
            &ty_generics,
            where_clause,
            &event_type,
            &method_name,
            controller_config.as_ref(),
        )
    } else {
        // Generate standalone Runnable for input_device only
        quote! {
            impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
                async fn run(&mut self) -> ! {
                    use ::rmk::event::publish_input_event_async;
                    loop {
                        let event = self.#method_name().await;
                        publish_input_event_async(event).await;
                    }
                }
            }
        }
    };

    // Add marker attribute if we generated Runnable and there are other macros
    let marker_attr = if !has_runnable_marker && has_controller {
        quote! { #[::rmk::runnable_generated] }
    } else {
        quote! {}
    };

    // Generate the complete output
    let expanded = quote! {
        #(#attrs)*
        #marker_attr
        #vis #struct_def

        // Internal enum for InputDevice trait (single variant)
        #vis enum #enum_name {
            Event0(#event_type),
        }

        impl #impl_generics ::rmk::input_device::InputDevice for #struct_name #ty_generics #where_clause {
            type Event = #enum_name;

            async fn read_event(&mut self) -> Self::Event {
                #enum_name::Event0(self.#method_name().await)
            }
        }

        #runnable_impl
    };

    expanded.into()
}

/// InputDevice attribute configuration
struct DeviceConfig {
    event_type: Option<Path>,
}

/// Parse #[input_device] publish attribute
fn parse_device_attributes(attr: TokenStream) -> DeviceConfig {
    use syn::punctuated::Punctuated;
    use syn::{ExprArray, Token};

    let mut event_type = None;

    // Parse as Meta::List containing name-value pairs
    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let attr2: proc_macro2::TokenStream = attr.into();

    match parser.parse2(attr2) {
        Ok(parsed) => {
            for meta in parsed {
                if let Meta::NameValue(nv) = meta {
                    if nv.path.is_ident("publish") {
                        // Check if it's an array (not allowed for input_device)
                        if let syn::Expr::Array(ExprArray { .. }) = &nv.value {
                            // Will be caught by validation later
                            return DeviceConfig { event_type: None };
                        }
                        // Parse single event type
                        if let syn::Expr::Path(expr_path) = nv.value {
                            event_type = Some(expr_path.path);
                        }
                    }
                }
            }
        }
        Err(e) => {
            panic!("Failed to parse input_device attributes: {}", e);
        }
    }

    DeviceConfig { event_type }
}

/// Convert event type to read method name: BatteryEvent -> read_battery_event
fn event_type_to_read_method_name(path: &Path) -> syn::Ident {
    let type_name = path.segments.last().unwrap().ident.to_string();

    // Remove "Event" suffix if present
    let base_name = type_name.strip_suffix("Event").unwrap_or(&type_name);

    // Convert CamelCase to snake_case
    let snake_case = to_snake_case(base_name);

    // Add "read_" prefix and "_event" suffix
    format_ident!("read_{}_event", snake_case)
}

/// Convert CamelCase to snake_case
fn to_snake_case(s: &str) -> String {
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
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }

    result
}

/// Controller config parsed from attribute
struct ControllerConfigParsed {
    event_types: Vec<Path>,
    poll_interval_ms: Option<u64>,
}

/// Parse controller config from tokens
fn parse_controller_config_from_tokens(tokens: TokenStream) -> ControllerConfigParsed {
    use syn::punctuated::Punctuated;
    use syn::{ExprArray, Token};

    let mut event_types = Vec::new();
    let mut poll_interval_ms = None;

    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let tokens2: proc_macro2::TokenStream = tokens.into();

    if let Ok(parsed) = parser.parse2(tokens2) {
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

    ControllerConfigParsed {
        event_types,
        poll_interval_ms,
    }
}

/// Generate combined Runnable for input_device + controller
fn generate_combined_runnable_input_device_controller(
    struct_name: &syn::Ident,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
    _event_type: &Path,
    read_method: &syn::Ident,
    controller_config: Option<&ControllerConfigParsed>,
) -> proc_macro2::TokenStream {
    let controller_config = match controller_config {
        Some(c) => c,
        None => {
            return quote! {
                impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
                    async fn run(&mut self) -> ! {
                        use ::rmk::event::publish_input_event_async;
                        loop {
                            let event = self.#read_method().await;
                            publish_input_event_async(event).await;
                        }
                    }
                }
            }
        }
    };

    // Generate controller event enum name
    let ctrl_enum_name = format_ident!("{}EventEnum", struct_name);

    // Generate subscriber definitions for controller events
    let ctrl_sub_defs: Vec<_> = controller_config
        .event_types
        .iter()
        .enumerate()
        .map(|(idx, ctrl_event_type)| {
            let sub_name = format_ident!("ctrl_sub{}", idx);
            quote! {
                let mut #sub_name = <#ctrl_event_type as ::rmk::event::ControllerEvent>::controller_subscriber();
            }
        })
        .collect();

    // Generate select arms for controller events
    let ctrl_select_arms: Vec<_> = controller_config
        .event_types
        .iter()
        .enumerate()
        .map(|(idx, _)| {
            let sub_name = format_ident!("ctrl_sub{}", idx);
            let variant_name = format_ident!("Event{}", idx);
            quote! {
                ctrl_event = #sub_name.next_event().fuse() => {
                    <Self as ::rmk::controller::Controller>::process_event(self, #ctrl_enum_name::#variant_name(ctrl_event)).await;
                }
            }
        })
        .collect();

    // Check if polling is enabled
    if let Some(interval_ms) = controller_config.poll_interval_ms {
        // Combined with polling
        quote! {
            impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
                async fn run(&mut self) -> ! {
                    use ::rmk::event::{ControllerEvent, EventSubscriber, publish_input_event_async};
                    use ::rmk::futures::FutureExt;
                    use ::rmk::controller::{Controller, PollingController};

                    #(#ctrl_sub_defs)*
                    let mut last = ::embassy_time::Instant::now();

                    loop {
                        let elapsed = last.elapsed();
                        let interval = ::embassy_time::Duration::from_millis(#interval_ms);
                        let timer = ::embassy_time::Timer::after(
                            interval.checked_sub(elapsed).unwrap_or(::embassy_time::Duration::MIN)
                        );

                        ::rmk::futures::select_biased! {
                            event = self.#read_method().fuse() => {
                                publish_input_event_async(event).await;
                            },
                            #(#ctrl_select_arms)*
                            _ = timer.fuse() => {
                                <Self as PollingController>::update(self).await;
                                last = ::embassy_time::Instant::now();
                            },
                        }
                    }
                }
            }
        }
    } else {
        // Combined without polling
        quote! {
            impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
                async fn run(&mut self) -> ! {
                    use ::rmk::event::{ControllerEvent, EventSubscriber, publish_input_event_async};
                    use ::rmk::futures::FutureExt;
                    use ::rmk::controller::Controller;

                    #(#ctrl_sub_defs)*

                    loop {
                        ::rmk::futures::select_biased! {
                            event = self.#read_method().fuse() => {
                                publish_input_event_async(event).await;
                            },
                            #(#ctrl_select_arms)*
                        }
                    }
                }
            }
        }
    }
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
