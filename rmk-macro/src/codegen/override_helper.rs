use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use syn::{ItemFn, ItemMod};

/// List of functions that can be overwritten
#[derive(Debug, Clone, Copy, FromMeta)]
pub enum Overwritten {
    Usb,
    ChipConfig,
    ChipInit,
    Entry,
    /// Accepted so `#[Override(bind_interrupt)]` (used in existing examples)
    /// passes validation; the bind_interrupt override itself is matched
    /// separately in `bind_interrupt.rs`.
    BindInterrupt,
}

/// Find the override attribute on a function and parse it. Both documented
/// spellings are accepted: `#[Override(...)]` (docs, examples) and
/// `#[Overwritten(...)]` (this enum's name; the previous matching ignored the
/// attribute path entirely, so both were in use).
///
/// Returns:
/// - `None`: the function carries no override attribute
/// - `Some(Ok(_))`: a valid override marker (other attributes, doc comments
///   included, are allowed alongside it)
/// - `Some(Err(_))`: an override attribute that failed to parse
///   (unknown or miscased variant, missing argument, ...)
pub(crate) fn find_overwritten(item_fn: &ItemFn) -> Option<darling::Result<Overwritten>> {
    item_fn
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("Override") || attr.path().is_ident("Overwritten"))
        .map(|attr| Overwritten::from_meta(&attr.meta))
}

/// Validate every `#[Overwritten(...)]` attribute in the module.
///
/// Returns darling's diagnostics rendered as `compile_error!` tokens, or
/// `None` if all attributes are valid. This turns a typo like
/// `#[Overwritten(Entry)]` into a compile error instead of a silent fallback
/// to the generated default (#966).
pub(crate) fn validate_overwritten_attrs(item_mod: &ItemMod) -> Option<TokenStream2> {
    let mut errors: Vec<darling::Error> = Vec::new();
    if let Some((_, items)) = &item_mod.content {
        for item in items {
            if let syn::Item::Fn(item_fn) = item
                && let Some(Err(e)) = find_overwritten(item_fn)
            {
                errors.push(e);
            }
        }
    }
    if errors.is_empty() {
        None
    } else {
        Some(darling::Error::multiple(errors).write_errors())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_fn(src: &str) -> ItemFn {
        syn::parse_str(src).expect("test fn should parse")
    }

    #[test]
    fn no_overwritten_attribute_is_none() {
        assert!(find_overwritten(&parse_fn("fn f() {}")).is_none());
        assert!(find_overwritten(&parse_fn("#[inline]\nfn f() {}")).is_none());
    }

    #[test]
    fn valid_variant_parses() {
        let res = find_overwritten(&parse_fn("#[Overwritten(entry)]\nfn f() {}"));
        assert!(matches!(res, Some(Ok(Overwritten::Entry))));
    }

    #[test]
    fn override_spelling_is_accepted() {
        // The documented spelling (docs/examples) is `#[Override(...)]`.
        let res = find_overwritten(&parse_fn("#[Override(chip_config)]\nfn f() {}"));
        assert!(matches!(res, Some(Ok(Overwritten::ChipConfig))));
    }

    #[test]
    fn bind_interrupt_passes_validation() {
        // Used by existing examples; consumed separately in bind_interrupt.rs.
        let res = find_overwritten(&parse_fn("#[Override(bind_interrupt)]\nfn f() {}"));
        assert!(matches!(res, Some(Ok(Overwritten::BindInterrupt))));
    }

    #[test]
    fn doc_comment_does_not_disable_override() {
        // Doc comments are attributes; they used to make the override
        // silently ineligible via the old `attrs.len() == 1` check.
        let res = find_overwritten(&parse_fn("/// my entry\n#[Overwritten(entry)]\nfn f() {}"));
        assert!(matches!(res, Some(Ok(Overwritten::Entry))));
    }

    #[test]
    fn miscased_variant_is_an_error() {
        let res = find_overwritten(&parse_fn("#[Overwritten(Entry)]\nfn f() {}"));
        assert!(matches!(res, Some(Err(_))));
        let res = find_overwritten(&parse_fn("#[Override(Entry)]\nfn f() {}"));
        let err = match res {
            Some(Err(e)) => e,
            other => panic!("expected Some(Err(_)), got {other:?}"),
        };
        // darling's diagnostic suggests the snake_case spelling
        assert!(
            err.to_string().contains("entry"),
            "unexpected message: {err}"
        );
    }

    #[test]
    fn invalid_attr_in_module_becomes_compile_error() {
        let item_mod: ItemMod =
            syn::parse_str("mod kb { #[Overwritten(Entry)] fn run() {} }").unwrap();
        let tokens = validate_overwritten_attrs(&item_mod).expect("should produce errors");
        assert!(tokens.to_string().contains("compile_error"));
    }

    #[test]
    fn valid_module_produces_no_errors() {
        let item_mod: ItemMod =
            syn::parse_str("mod kb { #[Overwritten(entry)] fn run() {} fn helper() {} }").unwrap();
        assert!(validate_overwritten_attrs(&item_mod).is_none());
    }
}
