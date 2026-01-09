use quote::{quote, format_ident};
use syn::{parse_macro_input, DeriveInput, Meta, Path};
use syn::parse::Parser;

/// Implements the #[controller] macro
///
/// This macro generates:
/// 1. An internal enum to wrap all subscribed event types
/// 2. Implementation of Controller trait with event routing
/// 3. Generated `next_message()` using embassy_futures::select
///
/// Attributes:
/// - `subscribe = [EventType1, EventType2, ...]`: List of event types to subscribe to
///
/// Examples:
/// ```ignore
/// #[controller(subscribe = [BatteryEvent, ChargingStateEvent])]
/// pub struct BatteryLedController<P> {
///     pin: OutputController<P>,
///     state: BatteryState,
/// }
///
/// impl<P> BatteryLedController<P> {
///     async fn on_battery_event(&mut self, event: BatteryEvent) { /* ... */ }
///     async fn on_charging_state_event(&mut self, event: ChargingStateEvent) { /* ... */ }
/// }
/// ```
pub fn controller_impl(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    // Parse attributes to extract event types
    let event_types = parse_controller_attributes(attr);

    if event_types.is_empty() {
        return syn::Error::new_spanned(
            input,
            "#[controller] requires `subscribe` attribute with at least one event type. Use `#[controller(subscribe = [EventType1, EventType2])]`"
        )
        .to_compile_error()
        .into();
    }

    // Validate input is a struct
    if !matches!(input.data, syn::Data::Struct(_)) {
        return syn::Error::new_spanned(
            input,
            "#[controller] can only be applied to structs"
        )
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
    let enum_variants: Vec<_> = event_types.iter().enumerate().map(|(idx, event_type)| {
        let variant_name = format_ident!("Event{}", idx);
        quote! { #variant_name(#event_type) }
    }).collect();

    // Generate match arms for process_event
    let process_event_arms: Vec<_> = event_types.iter().enumerate().map(|(idx, event_type)| {
        let variant_name = format_ident!("Event{}", idx);
        let method_name = event_type_to_method_name(event_type);
        quote! {
            #enum_name::#variant_name(event) => self.#method_name(event).await
        }
    }).collect();

    // Generate next_message implementation using embassy_futures::select
    let next_message_impl = generate_next_message(&event_types, &enum_name);

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
    };

    expanded.into()
}

/// Parse #[controller] attributes to extract event types
fn parse_controller_attributes(attr: proc_macro::TokenStream) -> Vec<Path> {
    use syn::{Token, punctuated::Punctuated, ExprArray};

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
            panic!("Failed to parse controller attributes: {}", e);
        }
    }

    event_types
}

/// Convert event type path to method name
/// Example: BatteryEvent -> on_battery_event
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
    let mut prev_is_lower = false;

    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 && prev_is_lower {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
            prev_is_lower = false;
        } else {
            result.push(c);
            prev_is_lower = true;
        }
    }

    result
}

/// Generate next_message implementation using futures::select_biased
fn generate_next_message(event_types: &[Path], enum_name: &syn::Ident) -> proc_macro2::TokenStream {
    let num_events = event_types.len();

    // Create subscriber variable names
    let sub_vars: Vec<_> = (0..num_events)
        .map(|i| format_ident!("sub{}", i))
        .collect();

    // Create subscriber initializations
    let sub_inits: Vec<_> = event_types.iter().zip(&sub_vars).map(|(event_type, sub_var)| {
        quote! {
            let mut #sub_var = <#event_type as ::rmk::event::ControllerEventTrait>::subscriber();
        }
    }).collect();

    // Generate select_biased! arms for each event
    let select_arms: Vec<_> = sub_vars.iter().enumerate().map(|(idx, sub_var)| {
        let variant_name = format_ident!("Event{}", idx);
        quote! {
            event = #sub_var.next_event().fuse() => #enum_name::#variant_name(event),
        }
    }).collect();

    quote! {
        async fn next_message(&mut self) -> Self::Event {
            use ::rmk::event::EventSubscriber;
            use ::futures::FutureExt;
            #(#sub_inits)*

            ::futures::select_biased! {
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
        assert_eq!(to_snake_case("Battery"), "battery");
        assert_eq!(to_snake_case("ChargingState"), "charging_state");
        assert_eq!(to_snake_case("KeyboardIndicator"), "keyboard_indicator");
        assert_eq!(to_snake_case("BLE"), "b_l_e");
    }
}
