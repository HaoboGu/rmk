use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::{parse_macro_input, Attribute, DeriveInput, Meta, Path};

pub fn input_device_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    let event_type = match parse_input_device_attributes(attr) {
        Ok(event_type) => event_type,
        Err(err) => return err.to_compile_error().into(),
    };

    if !matches!(input.data, syn::Data::Struct(_)) {
        return syn::Error::new_spanned(input, "#[input_device] can only be applied to structs")
            .to_compile_error()
            .into();
    }

    if let Some(attr) = find_attr(&input.attrs, "input_processor") {
        return syn::Error::new_spanned(attr, "#[input_device] cannot be combined with #[input_processor]")
            .to_compile_error()
            .into();
    }

    let has_marker = has_runnable_marker(&input.attrs);
    let controller_attr = if has_marker {
        None
    } else {
        find_attr(&input.attrs, "controller")
    };

    let controller_config = if let Some(attr) = controller_attr {
        match parse_controller_attribute(attr) {
            Ok(config) => Some(config),
            Err(err) => return err.to_compile_error().into(),
        }
    } else {
        None
    };

    let struct_name = &input.ident;
    let vis = &input.vis;
    let attrs = &input.attrs;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

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

    let enum_name = format_ident!("{}InputEventEnum", struct_name);
    let read_method = event_type_to_read_method_name(&event_type);

    let runnable_impl = if has_marker {
        quote! {}
    } else if let Some(config) = controller_config {
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

        let polling_setup = if config.poll_interval_ms.is_some() {
            quote! { let mut last = ::embassy_time::Instant::now(); }
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
    } else {
        quote! {
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
    };

    let marker_attr = if has_marker { quote! {} } else { quote! { #[::rmk::runnable_generated] } };

    let expanded = quote! {
        #(#attrs)*
        #marker_attr
        #vis #struct_def

        enum #enum_name {
            Event0(#event_type),
        }

        impl #impl_generics ::rmk::input_device::InputDevice for #struct_name #ty_generics #where_clause {
            type Event = #enum_name;

            async fn read_event(&mut self) -> Self::Event {
                #enum_name::Event0(self.#read_method().await)
            }
        }

        #runnable_impl
    };

    expanded.into()
}

struct ControllerConfig {
    event_types: Vec<Path>,
    poll_interval_ms: Option<u64>,
}

fn parse_input_device_attributes(attr: TokenStream) -> Result<Path, syn::Error> {
    use syn::punctuated::Punctuated;
    use syn::Expr;
    use syn::Token;

    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let attr2: proc_macro2::TokenStream = attr.into();
    let parsed = parser
        .parse2(attr2)
        .map_err(|e| syn::Error::new(proc_macro2::Span::call_site(), format!("Failed to parse input_device attributes: {e}")))?;

    let mut event_type: Option<Path> = None;

    for meta in parsed {
        if let Meta::NameValue(nv) = meta {
            if nv.path.is_ident("publish") {
                if event_type.is_some() {
                    return Err(syn::Error::new_spanned(nv, "#[input_device] supports only one publish event type"));
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

    event_type.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[input_device] requires `publish = EventType`",
        )
    })
}

fn parse_controller_attribute(attr: &Attribute) -> Result<ControllerConfig, syn::Error> {
    use syn::punctuated::Punctuated;
    use syn::{Expr, ExprArray, ExprLit, Lit, Token};

    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let parsed = attr
        .parse_args_with(parser)
        .map_err(|e| syn::Error::new_spanned(attr, format!("Failed to parse controller attributes: {e}")))?;

    let mut event_types = Vec::new();
    let mut poll_interval_ms = None;

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

fn event_type_to_read_method_name(path: &Path) -> syn::Ident {
    let type_name = path.segments.last().unwrap().ident.to_string();
    let base_name = type_name.strip_suffix("Event").unwrap_or(&type_name);
    let snake_case = to_snake_case(base_name);
    format_ident!("read_{}_event", snake_case)
}

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
