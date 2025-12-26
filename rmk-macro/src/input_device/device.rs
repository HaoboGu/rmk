use proc_macro2::TokenStream;
use quote::quote;
use syn::{ItemFn, ItemMod};

/// Information about a custom input device
pub(crate) struct DeviceInfo {
    /// Initialization code for the device
    pub(crate) init: TokenStream,
    /// Device variable name
    pub(crate) name: syn::Ident,
}

/// Expands custom device initialization from #[device] attributes
///
/// Searches for all functions with #[device] attribute in the item_mod
/// and generates initialization code for each device.
///
/// # Example
/// ```rust
/// #[device]
/// fn trackball() -> Trackball {
///     Trackball::new(p.I2C0, p.P0_26, p.P0_27)
/// }
/// ```
///
/// Generates:
/// ```rust
/// let mut trackball = {
///     Trackball::new(p.I2C0, p.P0_26, p.P0_27)
/// };
/// ```
pub(crate) fn expand_custom_devices(item_mod: &ItemMod) -> Vec<DeviceInfo> {
    let mut devices = vec![];

    if let Some((_, items)) = &item_mod.content {
        items.iter().for_each(|item| {
            if let syn::Item::Fn(item_fn) = &item {
                if let Some(_attr) = item_fn
                    .attrs
                    .iter()
                    .find(|attr| attr.path().is_ident("device"))
                {
                    let device_info = expand_single_device(item_fn);
                    devices.push(device_info);
                }
            }
        });
    }

    devices
}

/// Expands a single custom device function into initialization code
fn expand_single_device(fn_item: &ItemFn) -> DeviceInfo {
    let device_name = &fn_item.sig.ident;
    let content = &fn_item.block;

    let init = quote! {
        let mut #device_name = #content;
    };

    DeviceInfo {
        init,
        name: device_name.clone(),
    }
}
