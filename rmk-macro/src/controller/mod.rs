use proc_macro2::TokenStream;
use quote::quote;
use rmk_config::{ChipModel, KeyboardTomlConfig, PinConfig};
use syn::ItemMod;

use crate::{
    feature::{get_rmk_features, is_feature_enabled},
    gpio_config::convert_gpio_str_to_output_pin,
};

/// Expands the controller initialization code based on the keyboard configuration.
/// Returns a tuple containing: (controller_initialization, controller_execution)
pub(crate) fn expand_controller_init(
    keyboard_config: &KeyboardTomlConfig,
    item_mod: &ItemMod,
) -> (TokenStream, Vec<TokenStream>) {
    let controller_feature_enabled = is_feature_enabled(&get_rmk_features(), "controller");

    let mut initializers = TokenStream::new();
    let mut executors = vec![];

    let (i, e) = expand_light_controllers(keyboard_config, controller_feature_enabled);
    initializers.extend(i);
    executors.extend(e);

    // external controller
    if let Some((_, items)) = &item_mod.content {
        items.iter().for_each(|item| {
            if let syn::Item::Fn(item_fn) = &item {
                if let Some(attr) = item_fn.attrs.iter().find(|attr| attr.path().is_ident("controller")) {
                    let _ = attr.parse_nested_meta(|meta| {
                        if !controller_feature_enabled {
                            panic!("\"controller\" feature of RMK must be enabled to use the #[controller] attribute");
                        }
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

fn expand_light_controllers(
    keyboard_config: &KeyboardTomlConfig,
    controller_feature_enabled: bool,
) -> (TokenStream, Vec<TokenStream>) {
    let chip = keyboard_config.get_chip_model().unwrap();
    let light_config = keyboard_config.get_light_config();

    let mut initializers = TokenStream::new();
    let mut executors = vec![];

    create_keyboard_indicator_controller(
        &chip,
        &light_config.numslock,
        quote! { numlock_controller },
        quote! { NumLock },
        controller_feature_enabled,
        &mut initializers,
        &mut executors,
    );

    create_keyboard_indicator_controller(
        &chip,
        &light_config.scrolllock,
        quote! { scrolllock_controller },
        quote! { ScrollLock },
        controller_feature_enabled,
        &mut initializers,
        &mut executors,
    );

    create_keyboard_indicator_controller(
        &chip,
        &light_config.capslock,
        quote! { capslock_controller },
        quote! { CapsLock },
        controller_feature_enabled,
        &mut initializers,
        &mut executors,
    );

    (initializers, executors)
}

fn create_keyboard_indicator_controller(
    chip: &ChipModel,
    pin_config: &Option<PinConfig>,
    controller_ident: TokenStream,
    led_indicator_variant: TokenStream,
    controller_feature_enabled: bool,
    initializers: &mut TokenStream,
    executors: &mut Vec<TokenStream>,
) {
    if let Some(c) = pin_config {
        if !controller_feature_enabled {
            panic!("\"controller\" feature of RMK must be enabled to use the [light] configuration")
        }
        let p = convert_gpio_str_to_output_pin(chip, c.pin.clone(), c.low_active);
        let low_active = c.low_active;
        let controller_init = quote! {
            let mut #controller_ident = ::rmk::controller::led_indicator::KeyboardIndicatorController::new(
                #p,
                #low_active,
                ::rmk::types::led_indicator::LedIndicatorType::#led_indicator_variant,
            );
        };
        initializers.extend(controller_init);
        executors.push(quote! { #controller_ident.event_loop() });
    }
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
