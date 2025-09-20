use proc_macro2::TokenStream;
use quote::quote;
use rmk_config::KeyboardTomlConfig;
use syn::ItemMod;

use crate::gpio_config::convert_gpio_str_to_output_pin;

/// Expands the controller initialization code based on the keyboard configuration.
/// Returns a tuple containing: (controller_initialization, controller_execution)
pub(crate) fn expand_controller_init(
    keyboard_config: &KeyboardTomlConfig,
    item_mod: &ItemMod,
) -> (TokenStream, Vec<TokenStream>) {
    // TODO: Check whether the `controller` feature is enabled
    let chip = keyboard_config.get_chip_model().unwrap();

    let light_config = keyboard_config.get_light_config();
    let mut initializers = TokenStream::new();
    let mut executors = vec![];
    if let Some(c) = light_config.numslock {
        let p = convert_gpio_str_to_output_pin(&chip, c.pin.clone(), c.low_active);
        let low_active = c.low_active;
        let numlock_init = quote! {
            let mut numslock_controller = ::rmk::controller::led_indicator::KeyboardIndicatorController::new(
                #p,
                #low_active,
                ::rmk::types::led_indicator::LedIndicatorType::NumLock,
            );
        };
        initializers.extend(numlock_init);
        executors.push(quote! { numslock_controller.event_loop() });
    }

    if let Some(c) = light_config.scrolllock {
        let p = convert_gpio_str_to_output_pin(&chip, c.pin.clone(), c.low_active);
        let low_active = c.low_active;
        let scrollock_init = quote! {
            let mut scrolllock_controller = ::rmk::controller::led_indicator::KeyboardIndicatorController::new(
                #p,
                #low_active,
                ::rmk::types::led_indicator::LedIndicatorType::ScrollLock,
            );
        };
        initializers.extend(scrollock_init);
        executors.push(quote! { scrolllock_controller.event_loop() });
    }

    if let Some(c) = light_config.capslock {
        let p = convert_gpio_str_to_output_pin(&chip, c.pin.clone(), c.low_active);
        let low_active = c.low_active;
        let capslock_init = quote! {
            let mut capslock_controller = ::rmk::controller::led_indicator::KeyboardIndicatorController::new(
                #p,
                #low_active,
                ::rmk::types::led_indicator::LedIndicatorType::CapsLock,
            );
        };
        initializers.extend(capslock_init);
        executors.push(quote! { capslock_controller.event_loop() });
    }

    // external controller
    if let Some((_, items)) = &item_mod.content {
        items.iter().for_each(|item| {
            if let syn::Item::Fn(item_fn) = &item {
                if let Some(attr) = item_fn.attrs.iter().find(|attr| attr.path().is_ident("controller")) {
                    let _ = attr.parse_nested_meta(|meta| {
                        let (custom_init, custom_exec) = expand_custom_controller(&item_fn);
                        initializers.extend(custom_init);

                        if meta.path.is_ident("event") {
                            // #[controller(event)]
                            executors.push(quote! { #custom_exec.event_loop() });
                            return Ok(());
                        } else if meta.path.is_ident("poll") {
                            // #[controller(poll)]
                            executors.push(quote! { #custom_exec.polling_loop() });
                            return Ok(());
                        }

                        panic!("\"controller\" attrubute must specify executon mode with #[controller(event)] or #[controller(poll)]")
                    });
                }
            }
        });
    }

    (initializers, executors)
}

fn expand_custom_controller(fn_item: &syn::ItemFn) -> (TokenStream, &syn::Ident) {
    let task_name = &fn_item.sig.ident;

    let content = &fn_item.block.stmts;
    let initializer = quote! {
        let mut #task_name = {
            #(#content)*
        };
    };

    (initializer, task_name)
}
