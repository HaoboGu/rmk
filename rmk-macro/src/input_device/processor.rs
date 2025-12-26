use proc_macro2::TokenStream;
use quote::quote;
use syn::{ItemFn, ItemMod};

/// Information about a custom input processor
pub(crate) struct ProcessorInfo {
    /// Initialization code for the processor
    pub(crate) init: TokenStream,
    /// Processor variable name
    pub(crate) name: syn::Ident,
}

/// Expands custom processor initialization from #[processor] attributes
///
/// Searches for all functions with #[processor] attribute in the item_mod
/// and generates initialization code for each processor.
///
/// # Example
/// ```rust
/// #[processor]
/// fn scroll_processor() -> ScrollWheelProcessor {
///     ScrollWheelProcessor::new(&keymap)
/// }
/// ```
///
/// Generates:
/// ```rust
/// let mut scroll_processor = {
///     ScrollWheelProcessor::new(&keymap)
/// };
/// ```
pub(crate) fn expand_custom_processors(item_mod: &ItemMod) -> Vec<ProcessorInfo> {
    let mut processors = vec![];

    if let Some((_, items)) = &item_mod.content {
        items.iter().for_each(|item| {
            if let syn::Item::Fn(item_fn) = &item {
                if let Some(_attr) = item_fn
                    .attrs
                    .iter()
                    .find(|attr| attr.path().is_ident("processor"))
                {
                    let processor_info = expand_single_processor(item_fn);
                    processors.push(processor_info);
                }
            }
        });
    }

    processors
}

/// Expands a single custom processor function into initialization code
fn expand_single_processor(fn_item: &ItemFn) -> ProcessorInfo {
    let processor_name = &fn_item.sig.ident;
    let content = &fn_item.block;

    let init = quote! {
        let mut #processor_name = #content;
    };

    ProcessorInfo {
        init,
        name: processor_name.clone(),
    }
}
