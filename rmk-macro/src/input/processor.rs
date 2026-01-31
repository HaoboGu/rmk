use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::{DeriveInput, Meta, Path, parse_macro_input};

/// Generates InputProcessor trait implementation with automatic event routing.
///
/// See `rmk::input_device::InputProcessor` trait documentation for usage.
///
/// This macro is used to define InputProcessor structs:
/// ```rust
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
    let enum_variants: Vec<_> = config
        .event_types
        .iter()
        .enumerate()
        .map(|(idx, event_type)| {
            let variant_name = format_ident!("Event{}", idx);
            quote! { #variant_name(#event_type) }
        })
        .collect();

    let enum_subs_defs: Vec<_> = config
        .event_types
        .iter()
        .enumerate()
        .map(|(idx, event_type)| {
            let sub_name = format_ident!("sub{}", idx);
            quote! { let mut #sub_name = <#event_type as ::rmk::event::InputEvent>::input_subscriber(); }
        })
        .collect();

    let enum_subs_arms: Vec<_> = config
        .event_types
        .iter()
        .enumerate()
        .map(|(idx, _event_type)| {
            let sub_name = format_ident!("sub{}", idx);
            let variant_name = format_ident!("Event{}", idx);
            quote! { e = #sub_name.next_event().fuse() => <Self as ::rmk::input_device::InputProcessor<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>>::Event::#variant_name(e) }
        })
        .collect();

    // Generate match arms for process method
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

    // Generate the complete output
    let expanded = quote! {
        #(#attrs)*
        #vis #struct_def

        // Internal enum for event routing
        #vis enum #enum_name {
            #(#enum_variants),*
        }

        // FIXME: allow users to override `run()` function
        impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
            async fn run(&mut self) -> ! {
                use ::rmk::input_device::InputProcessor;
                use ::rmk::event::InputEvent;
                use ::rmk::event::EventSubscriber;
                use ::futures::FutureExt;
                #(#enum_subs_defs)*
                loop {
                    let e = ::futures::select_biased! {
                        #(#enum_subs_arms),*
                    };
                    self.process(e).await;
                }
            }
        }

        impl #impl_generics ::rmk::input_device::InputProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER> for #struct_name #ty_generics #where_clause {
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

/// Convert event type to handler method name: BatteryEvent -> on_battery_event
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_snake_case() {
        // Basic cases
        assert_eq!(to_snake_case("Battery"), "battery");
        assert_eq!(to_snake_case("ChargingState"), "charging_state");
        assert_eq!(to_snake_case("InputChargingState"), "input_charging_state");

        // Acronyms should stay together
        assert_eq!(to_snake_case("USB"), "usb");
        assert_eq!(to_snake_case("HID"), "hid");

        // Mixed acronyms and words
        assert_eq!(to_snake_case("USBKey"), "usb_key");
        assert_eq!(to_snake_case("HIDDevice"), "hid_device");
    }
}
