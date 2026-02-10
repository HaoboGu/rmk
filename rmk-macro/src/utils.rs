//! Generic utility functions for rmk-macro.
//!
//! Contains common utilities used across input and controller modules.

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

    /// Create an empty parser (for error fallback).
    pub fn empty() -> Self {
        Self { metas: vec![] }
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

    /// Get a string value for `name = "value"`.
    pub fn get_str(&self, name: &str) -> Option<String> {
        self.metas.iter().find_map(|meta| {
            if let Meta::NameValue(nv) = meta
                && nv.path.is_ident(name)
                && let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(lit),
                    ..
                }) = &nv.value
            {
                Some(lit.value())
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
                    format!(
                        "unknown attribute `{key}`. Expected one of: {allowed_list}"
                    ),
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

/// Internal case conversion function.
fn convert_case_internal(s: &str, to_upper: bool) -> String {
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];

        if c.is_uppercase() {
            // Add underscore before uppercase when:
            // 1) not at start
            // 2) previous is lowercase
            // 3) next is lowercase (acronym end)
            let add_underscore = i > 0
                && (chars[i - 1].is_lowercase()
                    || (i + 1 < chars.len() && chars[i + 1].is_lowercase()));

            if add_underscore {
                result.push('_');
            }
            result.push(if to_upper { c } else { c.to_ascii_lowercase() });
        } else {
            result.push(if to_upper { c.to_ascii_uppercase() } else { c });
        }
    }

    result
}

/// Convert CamelCase to snake_case.
pub fn to_snake_case(s: &str) -> String {
    convert_case_internal(s, false)
}

/// Convert CamelCase to UPPER_SNAKE_CASE.
pub fn to_upper_snake_case(s: &str) -> String {
    convert_case_internal(s, true)
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

/// Check whether an attribute path matches a given RMK macro name.
///
/// Matches bare `#[name]`, qualified `#[rmk::name]`, and fully qualified
/// `#[rmk::macros::name]` / `#[::rmk::macros::name]` forms.
pub fn is_rmk_attr(attr: &Attribute, name: &str) -> bool {
    let path = attr.path();
    path.is_ident(name)
        || (path.segments.len() == 2
            && path.segments[0].ident == "rmk"
            && path.segments[1].ident == name)
        || (path.segments.len() == 3
            && path.segments[0].ident == "rmk"
            && path.segments[1].ident == "macros"
            && path.segments[2].ident == name)
}

/// Check for the runnable_generated marker.
/// Prevents duplicate Runnable impls when macros combine.
pub fn has_runnable_marker(attrs: &[Attribute]) -> bool {
    attrs.iter().any(is_runnable_generated_attr)
}

/// Check runnable_generated attribute.
/// Accepts `#[runnable_generated]`, `#[rmk::runnable_generated]`, and `#[rmk::macros::runnable_generated]`.
pub fn is_runnable_generated_attr(attr: &Attribute) -> bool {
    is_rmk_attr(attr, "runnable_generated")
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
}
