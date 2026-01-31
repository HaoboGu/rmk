use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::parse::Parser;
use syn::{Attribute, DeriveInput, Meta, parse_macro_input};

/// Generates input event infrastructure using embassy_sync::channel::Channel.
///
/// This macro can be combined with `#[controller_event]` on the same struct to create
/// a dual-channel event type that supports both input and controller event patterns.
/// The order of the two macros does not matter.
///
/// See `rmk::event::InputEvent` for usage.
pub fn input_event_impl(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    // Parse attributes - only channel_size is used for Channel
    let channel_size = if attr.is_empty() {
        None
    } else {
        parse_input_event_attributes(attr)
    };

    // Validate input is a struct or enum
    if !matches!(input.data, syn::Data::Struct(_) | syn::Data::Enum(_)) {
        return syn::Error::new_spanned(
            input,
            "#[input_event] can only be applied to structs or enums",
        )
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

    // Check if controller_event macro is also present and extract its parameters
    let controller_event_attr = input
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("controller_event"));

    let type_name = &input.ident;
    let vis = &input.vis;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Generate InputEvent channel and trait implementations
    let input_channel_name = syn::Ident::new(
        &format!(
            "{}_INPUT_CHANNEL",
            to_upper_snake_case(&type_name.to_string())
        ),
        type_name.span(),
    );

    let cap = channel_size.unwrap_or_else(|| quote::quote! { 8 });

    let input_channel_static = quote! {
        static #input_channel_name: ::embassy_sync::channel::Channel<
            ::rmk::RawMutex,
            #type_name #ty_generics,
            { #cap }
        > = ::embassy_sync::channel::Channel::new();
    };

    let input_event_impl = quote! {
        impl #impl_generics ::rmk::event::InputEvent for #type_name #ty_generics #where_clause {
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

            fn input_publisher() -> Self::Publisher {
                #input_channel_name.sender()
            }

            fn input_subscriber() -> Self::Subscriber {
                #input_channel_name.receiver()
            }
        }
    };

    let async_input_event_impl = quote! {
        impl #impl_generics ::rmk::event::AsyncInputEvent for #type_name #ty_generics #where_clause {
            type AsyncPublisher = ::embassy_sync::channel::Sender<
                'static,
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #cap }
            >;

            fn input_publisher_async() -> Self::AsyncPublisher {
                #input_channel_name.sender()
            }
        }
    };

    // Filter out both macros from attributes for the final struct definition
    let filtered_attrs: Vec<TokenStream> = input
        .attrs
        .iter()
        .filter(|attr| {
            !attr.path().is_ident("input_event") && !attr.path().is_ident("controller_event")
        })
        .map(|attr| attr.to_token_stream())
        .collect();

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

    let expanded = if let Some(ctrl_attr) = controller_event_attr {
        // controller_event is also present, generate both sets of implementations
        let (ctrl_channel_size, ctrl_subs, ctrl_pubs) =
            parse_controller_event_attr_from_attribute(ctrl_attr);

        let controller_channel_name = syn::Ident::new(
            &format!(
                "{}_CONTROLLER_CHANNEL",
                to_upper_snake_case(&type_name.to_string())
            ),
            type_name.span(),
        );

        let ctrl_cap = ctrl_channel_size.unwrap_or_else(|| quote::quote! { 1 });
        let ctrl_subs_val = ctrl_subs.unwrap_or_else(|| quote::quote! { 4 });
        let ctrl_pubs_val = ctrl_pubs.unwrap_or_else(|| quote::quote! { 1 });

        let controller_channel_static = quote! {
            static #controller_channel_name: ::embassy_sync::pubsub::PubSubChannel<
                ::rmk::RawMutex,
                #type_name #ty_generics,
                { #ctrl_cap },
                { #ctrl_subs_val },
                { #ctrl_pubs_val }
            > = ::embassy_sync::pubsub::PubSubChannel::new();
        };

        let controller_event_impl = quote! {
            impl #impl_generics ::rmk::event::ControllerEvent for #type_name #ty_generics #where_clause {
                type Publisher = ::embassy_sync::pubsub::ImmediatePublisher<
                    'static,
                    ::rmk::RawMutex,
                    #type_name #ty_generics,
                    { #ctrl_cap },
                    { #ctrl_subs_val },
                    { #ctrl_pubs_val }
                >;
                type Subscriber = ::embassy_sync::pubsub::Subscriber<
                    'static,
                    ::rmk::RawMutex,
                    #type_name #ty_generics,
                    { #ctrl_cap },
                    { #ctrl_subs_val },
                    { #ctrl_pubs_val }
                >;

                fn controller_publisher() -> Self::Publisher {
                    #controller_channel_name.immediate_publisher()
                }

                fn controller_subscriber() -> Self::Subscriber {
                    #controller_channel_name.subscriber().unwrap()
                }
            }
        };

        let async_controller_event_impl = quote! {
            impl #impl_generics ::rmk::event::AsyncControllerEvent for #type_name #ty_generics #where_clause {
                type AsyncPublisher = ::embassy_sync::pubsub::Publisher<
                    'static,
                    ::rmk::RawMutex,
                    #type_name #ty_generics,
                    { #ctrl_cap },
                    { #ctrl_subs_val },
                    { #ctrl_pubs_val }
                >;

                fn controller_publisher_async() -> Self::AsyncPublisher {
                    #controller_channel_name.publisher().unwrap()
                }
            }
        };

        quote! {
            #(#filtered_attrs)*
            #vis #type_def

            #input_channel_static

            #input_event_impl

            #async_input_event_impl

            #controller_channel_static

            #controller_event_impl

            #async_controller_event_impl
        }
    } else {
        // Only input_event
        quote! {
            #(#filtered_attrs)*
            #vis #type_def

            #input_channel_static

            #input_event_impl

            #async_input_event_impl
        }
    };

    expanded.into()
}

/// Parse input_event macro attributes: only channel_size is needed for Channel
fn parse_input_event_attributes(attr: proc_macro::TokenStream) -> Option<proc_macro2::TokenStream> {
    use syn::Token;
    use syn::punctuated::Punctuated;

    let mut channel_size = None;

    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let attr2: proc_macro2::TokenStream = attr.into();

    match parser.parse2(attr2) {
        Ok(parsed) => {
            for meta in parsed {
                if let Meta::NameValue(nv) = meta {
                    if nv.path.is_ident("channel_size") {
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

/// Parse controller_event parameters from an Attribute
fn parse_controller_event_attr_from_attribute(
    attr: &Attribute,
) -> (
    Option<proc_macro2::TokenStream>,
    Option<proc_macro2::TokenStream>,
    Option<proc_macro2::TokenStream>,
) {
    use syn::Token;
    use syn::punctuated::Punctuated;

    let mut channel_size = None;
    let mut subs = None;
    let mut pubs = None;

    if let Meta::List(meta_list) = &attr.meta {
        let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
        if let Ok(parsed) = parser.parse2(meta_list.tokens.clone()) {
            for meta in parsed {
                if let Meta::NameValue(nv) = meta {
                    if nv.path.is_ident("channel_size") {
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
