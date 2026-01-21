use quote::quote;
use syn::parse::Parser;
use syn::{Attribute, DeriveInput, Meta, parse_macro_input};

/// Generates controller event infrastructure.
///
/// See `rmk::event::ControllerEventTrait` for usage.
pub fn controller_event_impl(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    // Parse attributes
    let (channel_size, subs, pubs) = if attr.is_empty() {
        (None, None, None)
    } else {
        parse_attributes(attr)
    };

    // Validate input is a struct or enum
    if !matches!(input.data, syn::Data::Struct(_) | syn::Data::Enum(_)) {
        return syn::Error::new_spanned(input, "#[controller_event] can only be applied to structs or enums")
            .to_compile_error()
            .into();
    }

    // Verify Clone + Copy derives
    if !has_derive(&input.attrs, "Clone") || !has_derive(&input.attrs, "Copy") {
        return syn::Error::new_spanned(
            input,
            "#[controller_event] requires the struct to derive Clone and Copy",
        )
        .to_compile_error()
        .into();
    }

    let type_name = &input.ident;
    let vis = &input.vis;
    let attrs = &input.attrs;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Reconstruct the type definition (struct or enum)
    let type_def = match &input.data {
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
        _ => unreachable!(),
    };

    // Generate channel name: BatteryLevelEvent -> BATTERY_LEVEL_EVENT_CHANNEL
    let channel_name = syn::Ident::new(
        &format!("{}_CHANNEL", to_upper_snake_case(&type_name.to_string())),
        type_name.span(),
    );

    // Generate channel and trait implementations
    let (channel_static, trait_impl) = {
        // PubSubChannel: buffered events with awaitable publish support
        // Wrap in braces to support const expressions in generic parameters
        let cap = channel_size.unwrap_or_else(|| quote::quote! { 1 });
        let subs_val = subs.unwrap_or_else(|| quote::quote! { 4 });
        let pubs_val = pubs.unwrap_or_else(|| quote::quote! { 1 });

        let awaitable_trait_impl = quote! {
            impl #impl_generics ::rmk::event::AwaitableControllerEventTrait for #type_name #ty_generics #where_clause {
                type AsyncPublisher = ::embassy_sync::pubsub::Publisher<
                    'static,
                    ::rmk::RawMutex,
                    #type_name #ty_generics,
                    { #cap },
                    { #subs_val },
                    { #pubs_val }
                >;

                // Awaitable publisher: waits if channel is full
                fn async_publisher() -> Self::AsyncPublisher {
                    #channel_name.publisher().unwrap()
                }
            }
        };

        (
            quote! {
                static #channel_name: ::embassy_sync::pubsub::PubSubChannel<
                    ::rmk::RawMutex,
                    #type_name #ty_generics,
                    { #cap },
                    { #subs_val },
                    { #pubs_val }
                > = ::embassy_sync::pubsub::PubSubChannel::new();
            },
            quote! {
                impl #impl_generics ::rmk::event::ControllerEventTrait for #type_name #ty_generics #where_clause {
                    type Publisher = ::embassy_sync::pubsub::ImmediatePublisher<
                        'static,
                        ::rmk::RawMutex,
                        #type_name #ty_generics,
                        { #cap },
                        { #subs_val },
                        { #pubs_val }
                    >;
                    type Subscriber = ::embassy_sync::pubsub::Subscriber<
                        'static,
                        ::rmk::RawMutex,
                        #type_name #ty_generics,
                        { #cap },
                        { #subs_val },
                        { #pubs_val }
                    >;

                    // Immediate publisher: drops events if buffer is full
                    fn publisher() -> Self::Publisher {
                        #channel_name.immediate_publisher()
                    }

                    fn subscriber() -> Self::Subscriber {
                        #channel_name.subscriber().unwrap()
                    }
                }

                #awaitable_trait_impl
            },
        )
    };

    // Generate the complete output
    let expanded = quote! {
        #(#attrs)*
        #vis #type_def

        #channel_static

        #trait_impl
    };

    expanded.into()
}

/// Parse macro attributes: (channel_size, subs, pubs)
fn parse_attributes(attr: proc_macro::TokenStream) -> (Option<proc_macro2::TokenStream>, Option<proc_macro2::TokenStream>, Option<proc_macro2::TokenStream>) {
    use syn::Token;
    use syn::punctuated::Punctuated;

    let mut channel_size = None;
    let mut subs = None;
    let mut pubs = None;

    // Parse as Meta::List containing name-value pairs
    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;

    // Convert to proc_macro2::TokenStream for syn parsing
    let attr2: proc_macro2::TokenStream = attr.into();

    match parser.parse2(attr2) {
        Ok(parsed) => {
            for meta in parsed {
                if let Meta::NameValue(nv) = meta {
                    if nv.path.is_ident("channel_size") {
                        // Support both literals and path expressions
                        let expr = &nv.value;
                        channel_size = Some(quote::quote! { #expr });
                    } else if nv.path.is_ident("subs") {
                        let expr = &nv.value;
                        subs = Some(quote::quote! { #expr });
                    } else if nv.path.is_ident("pubs") {
                        let expr = &nv.value;
                        pubs = Some(quote::quote! { #expr });
                    }
                }
            }
        }
        Err(e) => {
            panic!("Failed to parse controller_event attributes: {}", e);
        }
    }

    (channel_size, subs, pubs)
}

/// Check if a type has a specific derive attribute
fn has_derive(attrs: &[Attribute], derive_name: &str) -> bool {
    attrs.iter().any(|attr| {
        if attr.path().is_ident("derive")
            && let Meta::List(meta_list) = &attr.meta
        {
            return meta_list.tokens.to_string().contains(derive_name);
        }
        false
    })
}

/// Convert CamelCase to UPPER_SNAKE_CASE for channel names
fn to_upper_snake_case(s: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];

        if c.is_uppercase() {
            // Add underscore before uppercase letter if:
            // 1. Not at start (i > 0)
            // 2. Previous char is lowercase OR
            // 3. Next char exists and is lowercase (end of acronym: "HTMLParser" -> "HTML_Parser")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_upper_snake_case() {
        // Basic cases
        assert_eq!(to_upper_snake_case("BatteryEvent"), "BATTERY_EVENT");
        assert_eq!(to_upper_snake_case("KeyEvent"), "KEY_EVENT");
        assert_eq!(
            to_upper_snake_case("SplitPeripheralBatteryEvent"),
            "SPLIT_PERIPHERAL_BATTERY_EVENT"
        );

        // Acronyms should stay together
        assert_eq!(to_upper_snake_case("WPMEvent"), "WPM_EVENT");
        assert_eq!(to_upper_snake_case("BLEState"), "BLE_STATE");
        assert_eq!(to_upper_snake_case("USBConnection"), "USB_CONNECTION");

        // Mixed acronyms and words
        assert_eq!(to_upper_snake_case("HTMLParser"), "HTML_PARSER");
        assert_eq!(to_upper_snake_case("parseHTMLString"), "PARSE_HTML_STRING");
    }
}
