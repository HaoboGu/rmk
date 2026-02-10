//! Event system utility functions for rmk-macro.
//!
//! Contains utilities used by event system macros.
//! Case conversion utilities remain in the root utils module.

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Parser;
use syn::{Attribute, GenericParam, Meta};

/// Generic attribute parser for extracting values from macro attributes.
///
/// Parses attribute tokens in the form of `name = value` or `name = [value1, value2]`.
pub struct AttributeParser {
    metas: Vec<Meta>,
}

impl AttributeParser {
    /// Create a new parser from attribute tokens.
    pub fn new(tokens: impl Into<TokenStream>) -> Result<Self, syn::Error> {
        use syn::Token;
        use syn::punctuated::Punctuated;

        let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
        let tokens: TokenStream = tokens.into();
        let metas = parser.parse2(tokens)?;
        Ok(Self {
            metas: metas.into_iter().collect(),
        })
    }

    /// Create a new parser and validate keys in one step.
    ///
    /// This is more ergonomic than calling `new()` followed by `validate_keys()`.
    pub fn new_validated(
        tokens: impl Into<TokenStream>,
        allowed_keys: &[&str],
    ) -> Result<Self, TokenStream> {
        let parser = Self::new(tokens).map_err(|e| e.to_compile_error())?;
        parser.validate_keys(allowed_keys)?;
        Ok(parser)
    }

    /// Get an integer value for `name = N`.
    ///
    /// Returns an error when the key exists but is not an integer literal,
    /// or cannot be parsed into the requested integer type.
    pub fn get_int<T>(&self, name: &str) -> Result<Option<T>, TokenStream>
    where
        T: std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        let Some(meta) = self
            .metas
            .iter()
            .find(|meta| matches!(meta, Meta::NameValue(nv) if nv.path.is_ident(name)))
        else {
            return Ok(None);
        };

        let Meta::NameValue(nv) = meta else {
            return Ok(None);
        };

        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(lit),
            ..
        }) = &nv.value
        else {
            return Err(syn::Error::new_spanned(
                &nv.value,
                format!("`{name}` must be an integer literal"),
            )
            .to_compile_error());
        };

        lit.base10_parse().map(Some).map_err(|err| {
            syn::Error::new_spanned(lit, format!("invalid `{name}` value: {err}")).to_compile_error()
        })
    }

    /// Get an array of paths for `name = [Type1, Type2]`.
    ///
    /// Returns an error when the key exists but the value is not an array,
    /// or when any array element is not a path.
    pub fn get_path_array(&self, name: &str) -> Result<Vec<syn::Path>, TokenStream> {
        let Some(meta) = self
            .metas
            .iter()
            .find(|meta| matches!(meta, Meta::NameValue(nv) if nv.path.is_ident(name)))
        else {
            return Ok(vec![]);
        };

        let Meta::NameValue(nv) = meta else {
            return Ok(vec![]);
        };

        let syn::Expr::Array(arr) = &nv.value else {
            return Err(syn::Error::new_spanned(
                &nv.value,
                format!("`{name}` must be an array of type paths, e.g. `[EventA, EventB]`"),
            )
            .to_compile_error());
        };

        let mut result = Vec::with_capacity(arr.elems.len());
        for elem in &arr.elems {
            if let syn::Expr::Path(path_expr) = elem {
                result.push(path_expr.path.clone());
            } else {
                return Err(syn::Error::new_spanned(
                    elem,
                    format!("invalid `{name}` element: expected a type path"),
                )
                .to_compile_error());
            }
        }

        Ok(result)
    }

    /// Get a single path for `name = Type`.
    pub fn get_path(&self, name: &str) -> Option<syn::Path> {
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

    /// Validate attribute key/value pairs against the allowed set.
    ///
    /// Enforces `key = value` syntax and rejects unknown keys.
    pub fn validate_keys(&self, allowed: &[&str]) -> Result<(), TokenStream> {
        for meta in &self.metas {
            let Meta::NameValue(nv) = meta else {
                return Err(syn::Error::new_spanned(
                    meta,
                    "invalid attribute syntax. Expected `key = value`",
                )
                .to_compile_error());
            };

            let Some(key_ident) = nv.path.get_ident() else {
                return Err(syn::Error::new_spanned(
                    &nv.path,
                    "invalid attribute key. Expected a simple identifier",
                )
                .to_compile_error());
            };

            let key = key_ident.to_string();
            if !allowed.contains(&key.as_str()) {
                let allowed_list = allowed.join(", ");
                return Err(syn::Error::new_spanned(
                    &nv.path,
                    format!("unknown attribute `{key}`. Expected one of: {allowed_list}"),
                )
                .to_compile_error());
            }
        }
        Ok(())
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

/// Extract processor config from runnable_generated marker attribute.
///
/// When `#[processor]` runs before `#[input_device]`, it embeds the processor config
/// in a marker like: `#[::rmk::macros::runnable_generated(subscribe = [...], poll_interval = N)]`
///
/// This function extracts that embedded config so `#[input_device]` can generate
/// the combined Runnable implementation.
pub fn extract_processor_config_from_marker(
    attrs: &[Attribute],
) -> Option<crate::processor::ProcessorConfig> {
    for attr in attrs {
        if !is_runnable_generated_attr(attr) {
            continue;
        }

        // Check if this marker has embedded config
        if let Meta::List(meta_list) = &attr.meta
            && !meta_list.tokens.is_empty()
        {
            // Try to parse as processor config
            if let Ok(config) = crate::processor::parse_processor_config(meta_list.tokens.clone())
                && !config.event_types.is_empty()
            {
                return Some(config);
            }
        }
    }
    None
}
