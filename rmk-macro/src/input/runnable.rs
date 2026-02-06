//! Helpers for generating combined Runnable implementations.
//!
//! Provides a unified `Runnable` generator for input_device/input_processor/controller.
//! Keeps combined macro output consistent.

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::{Attribute, GenericParam, Meta, Path};

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

/// Generic attribute parser for extracting values from macro attributes.
///
/// Parses attribute tokens in the form of `name = value` or `name = [value1, value2]`.
pub struct AttributeParser {
    metas: Vec<Meta>,
}

impl AttributeParser {
    /// Create a new parser from attribute tokens.
    pub fn new(tokens: impl Into<TokenStream>) -> Result<Self, syn::Error> {
        use syn::punctuated::Punctuated;
        use syn::Token;

        let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
        let tokens: TokenStream = tokens.into();
        let metas = parser.parse2(tokens)?;
        Ok(Self {
            metas: metas.into_iter().collect(),
        })
    }

    /// Create an empty parser (for error fallback).
    pub fn empty() -> Self {
        Self { metas: vec![] }
    }

    /// Get an integer value for `name = N`.
    pub fn get_int<T>(&self, name: &str) -> Option<T>
    where
        T: std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        self.metas.iter().find_map(|meta| {
            if let Meta::NameValue(nv) = meta
                && nv.path.is_ident(name)
                && let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Int(lit),
                    ..
                }) = &nv.value
            {
                lit.base10_parse().ok()
            } else {
                None
            }
        })
    }

    /// Get an array of paths for `name = [Type1, Type2]`.
    pub fn get_path_array(&self, name: &str) -> Vec<Path> {
        self.metas
            .iter()
            .find_map(|meta| {
                if let Meta::NameValue(nv) = meta
                    && nv.path.is_ident(name)
                    && let syn::Expr::Array(arr) = &nv.value
                {
                    Some(
                        arr.elems
                            .iter()
                            .filter_map(|e| {
                                if let syn::Expr::Path(p) = e {
                                    Some(p.path.clone())
                                } else {
                                    None
                                }
                            })
                            .collect(),
                    )
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }

    /// Get a single path for `name = Type`.
    pub fn get_path(&self, name: &str) -> Option<Path> {
        self.metas.iter().find_map(|meta| {
            if let Meta::NameValue(nv) = meta
                && nv.path.is_ident(name)
                && let syn::Expr::Path(p) = &nv.value
            {
                Some(p.path.clone())
            } else {
                None
            }
        })
    }

    /// Get an expression as TokenStream for `name = expr`.
    /// Useful for values that need to be embedded as-is (like channel_size).
    pub fn get_expr_tokens(&self, name: &str) -> Option<TokenStream> {
        self.metas.iter().find_map(|meta| {
            if let Meta::NameValue(nv) = meta
                && nv.path.is_ident(name)
            {
                let expr = &nv.value;
                Some(quote! { #expr })
            } else {
                None
            }
        })
    }
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

/// Check if a type derives a trait (e.g., Clone).
///
/// This function parses the derive attribute properly to avoid false positives.
/// For example, searching for "Clone" won't match "CloneInto" or "DeepClone".
pub fn has_derive(attrs: &[Attribute], derive_name: &str) -> bool {
    use syn::punctuated::Punctuated;
    use syn::{Path, Token};

    attrs.iter().any(|attr| {
        if !attr.path().is_ident("derive") {
            return false;
        }

        let Meta::List(meta_list) = &attr.meta else {
            return false;
        };

        // Parse the derive macro's token list as comma-separated paths
        let parser = Punctuated::<Path, Token![,]>::parse_terminated;
        let Ok(paths) = parser.parse2(meta_list.tokens.clone()) else {
            return false;
        };

        // Check if any path's last segment matches the derive name exactly
        paths.iter().any(|path| {
            path.segments
                .last()
                .map(|seg| seg.ident == derive_name)
                .unwrap_or(false)
        })
    })
}

/// Attributes that should be preserved when reconstructing type definitions.
const PRESERVED_ATTR_NAMES: &[&str] = &[
    "repr",
    "cfg",
    "cfg_attr",
    "allow",
    "warn",
    "deny",
    "forbid",
    "must_use",
    "non_exhaustive",
];

/// Check if an attribute should be preserved.
fn should_preserve_attr(attr: &Attribute) -> bool {
    let path = attr.path();
    PRESERVED_ATTR_NAMES.iter().any(|name| path.is_ident(name))
}

/// Reconstruct a struct/enum definition.
/// Returns a TokenStream without most attributes, but preserves important
/// attributes like `#[repr]`, `#[cfg]`, etc.
///
/// Note: `#generics` from `syn::Generics` includes the where clause when used directly,
/// so we use `impl_generics` (which excludes where clause) and add `where_clause` separately
/// to avoid duplicating the where clause.
pub fn reconstruct_type_def(input: &syn::DeriveInput) -> TokenStream {
    let type_name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, _, where_clause) = generics.split_for_impl();

    // Preserve important attributes
    let preserved_attrs: Vec<_> = input.attrs.iter().filter(|a| should_preserve_attr(a)).collect();

    match &input.data {
        syn::Data::Struct(data_struct) => match &data_struct.fields {
            syn::Fields::Named(fields) => {
                quote! { #(#preserved_attrs)* struct #type_name #impl_generics #where_clause #fields }
            }
            syn::Fields::Unnamed(fields) => {
                quote! { #(#preserved_attrs)* struct #type_name #impl_generics #fields #where_clause ; }
            }
            syn::Fields::Unit => {
                quote! { #(#preserved_attrs)* struct #type_name #impl_generics #where_clause ; }
            }
        },
        syn::Data::Enum(data_enum) => {
            let variants = &data_enum.variants;
            quote! { #(#preserved_attrs)* enum #type_name #impl_generics #where_clause { #variants } }
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
/// Accepts `#[runnable_generated]`, `#[rmk::runnable_generated]`, and `#[rmk::macros::runnable_generated]`.
pub fn is_runnable_generated_attr(attr: &Attribute) -> bool {
    let path = attr.path();
    path.is_ident("runnable_generated")
        || (path.segments.len() == 2
            && path.segments[0].ident == "rmk"
            && path.segments[1].ident == "runnable_generated")
        || (path.segments.len() == 3
            && path.segments[0].ident == "rmk"
            && path.segments[1].ident == "macros"
            && path.segments[2].ident == "runnable_generated")
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

/// Convert event type to enum variant name (BatteryEvent -> Battery).
///
/// Strips "Event" suffix and returns the base name as the variant.
pub fn event_type_to_variant_name(path: &Path) -> syn::Ident {
    let type_name = path.segments.last().unwrap().ident.to_string();

    // Strip "Event" suffix if present.
    let base_name = type_name.strip_suffix("Event").unwrap_or(&type_name);

    format_ident!("{}", base_name)
}

/// Generate unique variant names from event types, handling collisions.
///
/// If two event types would produce the same variant name (e.g., FooEvent and FooData
/// both mapping to "Foo"), append numeric suffixes to disambiguate.
pub fn generate_unique_variant_names(event_types: &[Path]) -> Vec<syn::Ident> {
    use std::collections::HashMap;

    let base_names: Vec<syn::Ident> = event_types.iter().map(event_type_to_variant_name).collect();

    // Count occurrences of each name
    let mut counts: HashMap<String, usize> = HashMap::new();
    for name in &base_names {
        *counts.entry(name.to_string()).or_insert(0) += 1;
    }

    // Generate unique names with suffixes for duplicates
    let mut seen: HashMap<String, usize> = HashMap::new();
    base_names
        .iter()
        .map(|name| {
            let name_str = name.to_string();
            if counts[&name_str] > 1 {
                let idx = seen.entry(name_str.clone()).or_insert(0);
                *idx += 1;
                format_ident!("{}{}", name_str, idx)
            } else {
                name.clone()
            }
        })
        .collect()
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
        let proc_variant_names = generate_unique_variant_names(&processor_config.event_types);
        use_statements.push(quote! { use ::rmk::event::InputSubscribeEvent; });
        use_statements.push(quote! { use ::rmk::input_device::InputProcessor; });

        for (idx, (event_type, variant_name)) in processor_config
            .event_types
            .iter()
            .zip(&proc_variant_names)
            .enumerate()
        {
            let sub_name = format_ident!("proc_sub{}", idx);
            sub_defs.push(quote! {
                let mut #sub_name = <#event_type as ::rmk::event::InputSubscribeEvent>::input_subscriber();
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
        let ctrl_variant_names = generate_unique_variant_names(&ctrl_config.event_types);
        ctrl_enum_name = Some(ctrl_enum.clone());
        use_statements.push(quote! { use ::rmk::event::ControllerSubscribeEvent; });
        use_statements.push(quote! { use ::rmk::controller::Controller; });

        for (idx, (ctrl_event_type, variant_name)) in ctrl_config
            .event_types
            .iter()
            .zip(&ctrl_variant_names)
            .enumerate()
        {
            let sub_name = format_ident!("ctrl_sub{}", idx);
            sub_defs.push(quote! {
                let mut #sub_name = <#ctrl_event_type as ::rmk::event::ControllerSubscribeEvent>::controller_subscriber();
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
    // Now we can simply call event_loop() or polling_loop() since EventSubscriber handles the select.
    if input_device_config.is_none() && input_processor_config.is_none() && controller_config.is_some() {
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
    // Now we can simply call process_loop() since EventSubscriber handles the select.
    if input_device_config.is_none() && controller_config.is_none() && input_processor_config.is_some() {
        return runnable_impl(quote! {
            use ::rmk::input_device::InputProcessor;
            self.process_loop().await
        });
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
                                    #timer_arm,
                                    #(#select_arms),*
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
                                #timer_arm,
                                #(#select_arms),*
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
                                #(#select_arms),*
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
                            #(#select_arms),*
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
    let parser = AttributeParser::new(tokens).unwrap_or_else(|_| AttributeParser::empty());

    ControllerConfig {
        event_types: parser.get_path_array("subscribe"),
        poll_interval_ms: parser.get_int("poll_interval"),
    }
}

/// Parse input_device config from attribute tokens.
/// Extracts `publish = EventType`.
pub fn parse_input_device_config(tokens: impl Into<TokenStream>) -> Option<InputDeviceConfig> {
    let parser = AttributeParser::new(tokens).ok()?;
    parser.get_path("publish").map(|event_type| InputDeviceConfig { event_type })
}

/// Parse input_processor config from attribute tokens.
/// Extracts `subscribe = [...]`.
pub fn parse_input_processor_config(tokens: impl Into<TokenStream>) -> InputProcessorConfig {
    let parser = AttributeParser::new(tokens).unwrap_or_else(|_| AttributeParser::empty());

    InputProcessorConfig {
        event_types: parser.get_path_array("subscribe"),
    }
}

/// Controller event channel config (channel_size, subs, pubs).
pub struct ControllerEventChannelConfig {
    pub channel_size: Option<TokenStream>,
    pub subs: Option<TokenStream>,
    pub pubs: Option<TokenStream>,
}

/// Parse controller_event parameters from a TokenStream.
/// Extracts `channel_size`, `subs`, `pubs`.
pub fn parse_controller_event_channel_config(tokens: impl Into<TokenStream>) -> ControllerEventChannelConfig {
    let parser = AttributeParser::new(tokens).unwrap_or_else(|_| AttributeParser::empty());

    ControllerEventChannelConfig {
        channel_size: parser.get_expr_tokens("channel_size"),
        subs: parser.get_expr_tokens("subs"),
        pubs: parser.get_expr_tokens("pubs"),
    }
}

/// Parse controller_event parameters from an Attribute.
pub fn parse_controller_event_channel_config_from_attr(attr: &Attribute) -> ControllerEventChannelConfig {
    if let Meta::List(meta_list) = &attr.meta {
        parse_controller_event_channel_config(meta_list.tokens.clone())
    } else {
        ControllerEventChannelConfig {
            channel_size: None,
            subs: None,
            pubs: None,
        }
    }
}

/// Parse input_event channel_size from a TokenStream.
pub fn parse_input_event_channel_size(tokens: impl Into<TokenStream>) -> Option<TokenStream> {
    let parser = AttributeParser::new(tokens).ok()?;
    parser.get_expr_tokens("channel_size")
}

/// Parse input_event channel_size from an Attribute.
pub fn parse_input_event_channel_size_from_attr(attr: &Attribute) -> Option<TokenStream> {
    if let Meta::List(meta_list) = &attr.meta {
        parse_input_event_channel_size(meta_list.tokens.clone())
    } else {
        None
    }
}

/// Event trait type for generating unified EventSubscriber code.
#[derive(Clone, Copy)]
pub enum EventTraitType {
    /// Controller events (use ControllerSubscribeEvent trait)
    Controller,
    /// Input events (use InputSubscribeEvent trait)
    Input,
}

/// Generate EventSubscriber struct and its implementation.
///
/// This is a unified generator for both Controller and InputProcessor macros.
/// It generates:
/// - A subscriber struct that holds individual event subscribers
/// - `EventSubscriber` impl with `select_biased!` for event aggregation
/// - The corresponding event trait impl (`ControllerSubscribeEvent` or `InputSubscribeEvent`)
pub fn generate_event_subscriber(
    struct_name: &syn::Ident,
    event_types: &[Path],
    variant_names: &[syn::Ident],
    enum_name: &syn::Ident,
    vis: &syn::Visibility,
    event_trait: EventTraitType,
) -> TokenStream {
    let subscriber_name = format_ident!("{}EventSubscriber", struct_name);
    let num_events = event_types.len();

    // Subscriber field names
    let sub_fields: Vec<_> = (0..num_events).map(|i| format_ident!("sub{}", i)).collect();

    // Generate trait-specific code
    let (subscribe_trait_path, subscriber_method) = match event_trait {
        EventTraitType::Controller => (
            quote! { ::rmk::event::ControllerSubscribeEvent },
            quote! { controller_subscriber },
        ),
        EventTraitType::Input => (
            quote! { ::rmk::event::InputSubscribeEvent },
            quote! { input_subscriber },
        ),
    };

    // Struct field definitions with types
    let field_defs: Vec<_> = event_types
        .iter()
        .zip(&sub_fields)
        .map(|(event_type, field_name)| {
            quote! {
                #field_name: <#event_type as #subscribe_trait_path>::Subscriber
            }
        })
        .collect();

    // Field initializations in new()
    let field_inits: Vec<_> = event_types
        .iter()
        .zip(&sub_fields)
        .map(|(event_type, field_name)| {
            quote! {
                #field_name: <#event_type as #subscribe_trait_path>::#subscriber_method()
            }
        })
        .collect();

    // select_biased! arms for next_event
    let select_arms: Vec<_> = sub_fields
        .iter()
        .zip(variant_names)
        .map(|(field_name, variant_name)| {
            quote! {
                event = self.#field_name.next_event().fuse() => #enum_name::#variant_name(event),
            }
        })
        .collect();

    quote! {
        /// Event subscriber for aggregated events
        #vis struct #subscriber_name {
            #(#field_defs),*
        }

        impl #subscriber_name {
            /// Create a new event subscriber
            pub fn new() -> Self {
                Self {
                    #(#field_inits),*
                }
            }
        }

        impl ::rmk::event::EventSubscriber for #subscriber_name {
            type Event = #enum_name;

            async fn next_event(&mut self) -> Self::Event {
                use ::rmk::event::EventSubscriber;
                use ::rmk::futures::FutureExt;

                ::rmk::futures::select_biased! {
                    #(#select_arms)*
                }
            }
        }

        impl #subscribe_trait_path for #enum_name {
            type Subscriber = #subscriber_name;

            fn #subscriber_method() -> Self::Subscriber {
                #subscriber_name::new()
            }
        }
    }
}

/// Generate process_event/process match arms.
///
/// This is used by both Controller and InputProcessor macros.
pub fn generate_event_match_arms(
    event_types: &[Path],
    variant_names: &[syn::Ident],
    enum_name: &syn::Ident,
) -> Vec<TokenStream> {
    event_types
        .iter()
        .zip(variant_names)
        .map(|(event_type, variant_name)| {
            let method_name = event_type_to_handler_method_name(event_type);
            quote! {
                #enum_name::#variant_name(event) => self.#method_name(event).await
            }
        })
        .collect()
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

    #[test]
    fn test_has_derive() {
        use syn::parse_quote;

        // Test basic derive matching
        let attrs: Vec<Attribute> = vec![parse_quote!(#[derive(Clone)])];
        assert!(has_derive(&attrs, "Clone"));
        assert!(!has_derive(&attrs, "Copy"));

        // Test multiple derives
        let attrs: Vec<Attribute> = vec![parse_quote!(#[derive(Clone, Copy, Debug)])];
        assert!(has_derive(&attrs, "Clone"));
        assert!(has_derive(&attrs, "Copy"));
        assert!(has_derive(&attrs, "Debug"));
        assert!(!has_derive(&attrs, "Default"));

        // Test that it doesn't match partial names (false positive prevention)
        let attrs: Vec<Attribute> = vec![parse_quote!(#[derive(CloneInto)])];
        assert!(!has_derive(&attrs, "Clone")); // Should NOT match

        let attrs: Vec<Attribute> = vec![parse_quote!(#[derive(DeepClone)])];
        assert!(!has_derive(&attrs, "Clone")); // Should NOT match

        // Test fully qualified path
        let attrs: Vec<Attribute> = vec![parse_quote!(#[derive(std::clone::Clone)])];
        assert!(has_derive(&attrs, "Clone")); // Should match the last segment

        // Test empty attrs
        let attrs: Vec<Attribute> = vec![];
        assert!(!has_derive(&attrs, "Clone"));

        // Test non-derive attribute
        let attrs: Vec<Attribute> = vec![parse_quote!(#[repr(C)])];
        assert!(!has_derive(&attrs, "Clone"));
    }

    #[test]
    fn test_event_type_to_variant_name() {
        use syn::parse_quote;

        let path: Path = parse_quote!(BatteryEvent);
        assert_eq!(event_type_to_variant_name(&path).to_string(), "Battery");

        let path: Path = parse_quote!(ChargingStateEvent);
        assert_eq!(event_type_to_variant_name(&path).to_string(), "ChargingState");

        // Without Event suffix
        let path: Path = parse_quote!(KeyPress);
        assert_eq!(event_type_to_variant_name(&path).to_string(), "KeyPress");
    }

    #[test]
    fn test_generate_unique_variant_names() {
        use syn::parse_quote;

        // No collisions
        let paths: Vec<Path> = vec![
            parse_quote!(BatteryEvent),
            parse_quote!(ChargingEvent),
        ];
        let names = generate_unique_variant_names(&paths);
        assert_eq!(names[0].to_string(), "Battery");
        assert_eq!(names[1].to_string(), "Charging");

        // With collision (both map to same base name)
        let paths: Vec<Path> = vec![
            parse_quote!(FooEvent),
            parse_quote!(FooEvent), // Same type twice
        ];
        let names = generate_unique_variant_names(&paths);
        assert_eq!(names[0].to_string(), "Foo1");
        assert_eq!(names[1].to_string(), "Foo2");
    }
}
