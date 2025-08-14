use proc_macro2::TokenStream;
use quote::quote;
use rmk_config::KeyboardTomlConfig;
use syn::ItemMod;

use crate::gpio_config::convert_gpio_str_to_output_pin;

/// Expands the controller initialization code based on the keyboard configuration.
/// Returns a tuple containing: (controller_initialization, controller names)
pub(crate) fn expand_controller_init(
    keyboard_config: &KeyboardTomlConfig,
    item_mod: &ItemMod,
) -> (TokenStream, Vec<TokenStream>) {
    // TODO: Check whether the `controller` feature is enabled
    let chip = keyboard_config.get_chip_model().unwrap();

    let light_config = keyboard_config.get_light_config();
    let mut initializers = TokenStream::new();
    let mut controller_names = vec![];
    if let Some(c) = light_config.numslock {
        let p = convert_gpio_str_to_output_pin(&chip, c.pin.clone(), c.low_active);
        let low_active = c.low_active;
        let numlock_init = quote! {
            let mut numslock_controller = ::rmk::controller::led_indicator::KeyboardIndicatorController::new(
                #p,
                #low_active,
                ::rmk::controller::led_indicator::KeyboardIndicator::NumLock,
            );
        };
        initializers.extend(numlock_init);
        controller_names.push(quote! { numslock_controller });
    }

    if let Some(c) = light_config.scrolllock {
        let p = convert_gpio_str_to_output_pin(&chip, c.pin.clone(), c.low_active);
        let low_active = c.low_active;
        let scrollock_init = quote! {
            let mut scrolllock_controller = ::rmk::controller::led_indicator::KeyboardIndicatorController::new(
                #p,
                #low_active,
                ::rmk::controller::led_indicator::KeyboardIndicator::ScrollLock,
            );
        };
        initializers.extend(scrollock_init);
        controller_names.push(quote! { scrolllock_controller });
    }

    if let Some(c) = light_config.capslock {
        let p = convert_gpio_str_to_output_pin(&chip, c.pin.clone(), c.low_active);
        let low_active = c.low_active;
        let capslock_init = quote! {
            let mut capslock_controller = ::rmk::controller::led_indicator::KeyboardIndicatorController::new(
                #p,
                #low_active,
                ::rmk::controller::led_indicator::KeyboardIndicator::CapsLock,
            );
        };
        initializers.extend(capslock_init);
        controller_names.push(quote! { capslock_controller });
    }

    // external controller
    if let Some((_, items)) = &item_mod.content {
        items.iter().for_each(|item| {
            if let syn::Item::Fn(item_fn) = &item {
                if item_fn.attrs.iter().any(|attr| attr.path().is_ident("controller")) {
                    let (custom_init, custom_name) = expand_custom_controller(&item_fn);
                    initializers.extend(custom_init);
                    controller_names.push(custom_name);
                }
            }
        });
    }

    (initializers, controller_names)
}

fn expand_custom_controller(fn_item: &syn::ItemFn) -> (TokenStream, TokenStream) {
    let task_name = &fn_item.sig.ident;

    let content = &fn_item.block.stmts;
    let initializer = quote! {
        let mut #task_name = {
            #(#content)*
        };
    };

    (initializer, quote! { #task_name })
}
