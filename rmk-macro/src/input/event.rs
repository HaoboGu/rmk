use quote::quote;
use syn::parse::Parser;
use syn::{Attribute, DeriveInput, Meta, parse_macro_input};

/// Generates input event infrastructure using embassy_sync::channel::Channel.
///
/// See `rmk::event::Event` for usage.
pub fn input_event_impl(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    // Parse attributes - only channel_size is used for Channel
    let channel_size = if attr.is_empty() {
        None
    } else {
        parse_attributes(attr)
    };

    // Validate input is a struct or enum
    if !matches!(input.data, syn::Data::Struct(_) | syn::Data::Enum(_)) {
        return syn::Error::new_spanned(input, "#[input_event] can only be applied to structs or enums")
            .to_compile_error()
            .into();
    }

    // Verify Clone + Copy derives
    if !has_derive(&input.attrs, "Clone") || !has_derive(&input.attrs, "Copy") {
        return syn::Error::new_spanned(
            input,
            "#[input_event] requires the struct to derive Clone and Copy",
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

    // Generate channel name: KeyEvent -> KEY_EVENT_CHANNEL
    let channel_name = syn::Ident::new(
        &format!("{}_CHANNEL", to_upper_snake_case(&type_name.to_string())),
        type_name.span(),
    );

    // Generate channel and trait implementations using Channel
    let (channel_static, trait_impl) = {
        // Channel: simple MPMC channel with a buffer
        let cap = channel_size.unwrap_or_else(|| quote::quote! { 8 });

        let awaitable_trait_impl = quote! {
            impl #impl_generics ::rmk::event::AsyncEvent for #type_name #ty_generics #where_clause {
                type AsyncPublisher = ::embassy_sync::channel::Sender<
                    'static,
                    ::rmk::RawMutex,
                    #type_name #ty_generics,
                    { #cap }
                >;

                // Awaitable publisher: waits if channel is full
                fn publisher_async() -> Self::AsyncPublisher {
                    #channel_name.sender()
                }
            }
        };

        (
            quote! {
                static #channel_name: ::embassy_sync::channel::Channel<
                    ::rmk::RawMutex,
                    #type_name #ty_generics,
                    { #cap }
                > = ::embassy_sync::channel::Channel::new();
            },
            quote! {
                impl #impl_generics ::rmk::event::Event for #type_name #ty_generics #where_clause {
                    type Publisher = ::embassy_sync::channel::Sender<
                        'static,
                        ::rmk::RawMutex,
                        #type_name #ty_generics,
                        { #cap }
                    >;
                    type Subscriber = ::embassy_sync::channel::Receiver<
                        'static,
                        ::rmk::RawMutex,
                        #type_name #ty_generics,
                        { #cap }
                    >;

                    // Publisher: may wait or drop events if buffer is full (based on EventPublisher impl)
                    fn publisher() -> Self::Publisher {
                        #channel_name.sender()
                    }

                    fn subscriber() -> Self::Subscriber {
                        #channel_name.receiver()
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

/// Parse macro attributes: only channel_size is needed for Channel
fn parse_attributes(attr: proc_macro::TokenStream) -> Option<proc_macro2::TokenStream> {
    use syn::Token;
    use syn::punctuated::Punctuated;

    let mut channel_size = None;

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
                    }
                }
            }
        }
        Err(e) => {
            panic!("Failed to parse input_event attributes: {}", e);
        }
    }

    channel_size
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
        assert_eq!(to_upper_snake_case("KeyEvent"), "KEY_EVENT");
        assert_eq!(to_upper_snake_case("ModifierEvent"), "MODIFIER_EVENT");
        assert_eq!(
            to_upper_snake_case("TouchpadEvent"),
            "TOUCHPAD_EVENT"
        );

        // Acronyms should stay together
        assert_eq!(to_upper_snake_case("USBEvent"), "USB_EVENT");
        assert_eq!(to_upper_snake_case("HIDDevice"), "HID_DEVICE");
    }
}
