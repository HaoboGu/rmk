use quote::quote;
use syn::parse::Parser;
use syn::{Attribute, DeriveInput, Lit, Meta, parse_macro_input};

/// Implements the #[controller_event] macro
///
/// This macro generates:
/// 1. A static channel (Watch or PubSubChannel based on attributes)
/// 2. Implementation of ControllerEventTrait for the event type
/// 3. Implementation of AwaitableControllerEventTrait (when channel_size is specified)
///
/// Attributes:
/// - `channel_size = N`: Use PubSubChannel instead of Watch (for buffering)
/// - `subs = N`: Number of subscribers (required for Watch, default 4 for PubSubChannel)
/// - `pubs = N`: Number of publishers for PubSubChannel (default 1, only valid with channel_size)
///
/// Examples:
/// ```ignore
/// // Watch channel (default)
/// #[controller_event(subs = 1)]
/// pub struct BatteryEvent(pub u8);
///
/// // PubSubChannel with buffering and awaitable publish
/// #[controller_event(channel_size = 8, subs = 4, pubs = 2)]
/// pub struct KeyEvent { /* ... */ }
///
/// // Also works with enums
/// #[controller_event(subs = 1)]
/// pub enum ConnectionState {
///     Connected,
///     Disconnected,
/// }
/// ```
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

    // Generate channel name: TYPE_NAME_UPPER_SNAKE_CHANNEL
    let channel_name = syn::Ident::new(
        &format!("{}_CHANNEL", to_upper_snake_case(&type_name.to_string())),
        type_name.span(),
    );

    // Generate channel and trait implementation based on channel_size
    let (channel_static, trait_impl) = if let Some(cap) = channel_size {
        // PubSubChannel with ImmediatePublisher for ControllerEventTrait
        // and Publisher for AwaitableControllerEventTrait
        let subs_val = subs.unwrap_or(4);
        let pubs_val = pubs.unwrap_or(1);

        let awaitable_trait_impl = quote! {
            impl #impl_generics ::rmk::event::AwaitableControllerEventTrait for #type_name #ty_generics #where_clause {
                type AsyncPublisher = ::embassy_sync::pubsub::Publisher<
                    'static,
                    ::rmk::RawMutex,
                    #type_name #ty_generics,
                    #cap,
                    #subs_val,
                    #pubs_val
                >;

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
                    #cap,
                    #subs_val,
                    #pubs_val
                > = ::embassy_sync::pubsub::PubSubChannel::new();
            },
            quote! {
                impl #impl_generics ::rmk::event::ControllerEventTrait for #type_name #ty_generics #where_clause {
                    type Publisher = ::embassy_sync::pubsub::ImmediatePublisher<
                        'static,
                        ::rmk::RawMutex,
                        #type_name #ty_generics,
                        #cap,
                        #subs_val,
                        #pubs_val
                    >;
                    type Subscriber = ::embassy_sync::pubsub::Subscriber<
                        'static,
                        ::rmk::RawMutex,
                        #type_name #ty_generics,
                        #cap,
                        #subs_val,
                        #pubs_val
                    >;

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
    } else {
        // Watch channel (default) - no AwaitableControllerEventTrait
        let subs_val = subs.unwrap_or(4);
        (
            quote! {
                static #channel_name: ::embassy_sync::watch::Watch<
                    ::rmk::RawMutex,
                    #type_name #ty_generics,
                    #subs_val
                > = ::embassy_sync::watch::Watch::new();
            },
            quote! {
                impl #impl_generics ::rmk::event::ControllerEventTrait for #type_name #ty_generics #where_clause {
                    type Publisher = ::embassy_sync::watch::Sender<
                        'static,
                        ::rmk::RawMutex,
                        #type_name #ty_generics,
                        #subs_val
                    >;
                    type Subscriber = ::embassy_sync::watch::Receiver<
                        'static,
                        ::rmk::RawMutex,
                        #type_name #ty_generics,
                        #subs_val
                    >;

                    fn publisher() -> Self::Publisher {
                        #channel_name.sender()
                    }

                    fn subscriber() -> Self::Subscriber {
                        #channel_name.receiver().unwrap()
                    }
                }
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

/// Parse macro attributes to extract channel_size, subs, and pubs
fn parse_attributes(attr: proc_macro::TokenStream) -> (Option<usize>, Option<usize>, Option<usize>) {
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
                        if let syn::Expr::Lit(expr_lit) = nv.value
                            && let Lit::Int(lit) = expr_lit.lit {
                                channel_size =
                                    Some(lit.base10_parse().expect("channel_size must be a valid usize"));
                            }
                    } else if nv.path.is_ident("subs") {
                        if let syn::Expr::Lit(expr_lit) = nv.value
                            && let Lit::Int(lit) = expr_lit.lit {
                                subs = Some(lit.base10_parse().expect("subs must be a valid usize"));
                            }
                    } else if nv.path.is_ident("pubs") {
                        if let syn::Expr::Lit(expr_lit) = nv.value
                            && let Lit::Int(lit) = expr_lit.lit {
                                pubs = Some(lit.base10_parse().expect("pubs must be a valid usize"));
                            }
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

/// Check if a struct has a specific derive
fn has_derive(attrs: &[Attribute], derive_name: &str) -> bool {
    attrs.iter().any(|attr| {
        if attr.path().is_ident("derive")
            && let Meta::List(meta_list) = &attr.meta {
                return meta_list.tokens.to_string().contains(derive_name);
            }
        false
    })
}

/// Convert CamelCase to UPPER_SNAKE_CASE
fn to_upper_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut prev_is_upper = false;

    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 && !prev_is_upper {
                result.push('_');
            }
            result.push(c);
            prev_is_upper = true;
        } else {
            result.push(c.to_ascii_uppercase());
            prev_is_upper = false;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_upper_snake_case() {
        assert_eq!(to_upper_snake_case("BatteryEvent"), "BATTERY_EVENT");
        assert_eq!(to_upper_snake_case("KeyEvent"), "KEY_EVENT");
        assert_eq!(to_upper_snake_case("WPMEvent"), "W_P_M_EVENT");
        assert_eq!(
            to_upper_snake_case("SplitPeripheralBatteryEvent"),
            "SPLIT_PERIPHERAL_BATTERY_EVENT"
        );
    }
}
