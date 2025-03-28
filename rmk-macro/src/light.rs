//! Initialize light config boilerplate of RMK, including USB or BLE
//!
use quote::quote;

use crate::config::PinConfig;
use crate::gpio_config::convert_gpio_str_to_output_pin;
use crate::keyboard_config::KeyboardConfig;
use crate::ChipModel;

pub(crate) fn build_light_config(chip: &ChipModel, pin_config: &Option<PinConfig>) -> proc_macro2::TokenStream {
    match pin_config {
        Some(c) => {
            let p = convert_gpio_str_to_output_pin(chip, c.pin.clone(), c.low_active);
            let low_active = c.low_active;
            quote! {
                Some(::rmk::config::LightPinConfig {
                    pin: #p,
                    low_active: #low_active,
                })
            }
        }
        None => quote! {None},
    }
}

pub(crate) fn expand_light_config(keyboard_config: &KeyboardConfig) -> proc_macro2::TokenStream {
    let numslock = build_light_config(&keyboard_config.chip, &keyboard_config.light.numslock);
    let capslock = build_light_config(&keyboard_config.chip, &keyboard_config.light.capslock);
    let scrolllock = build_light_config(&keyboard_config.chip, &keyboard_config.light.scrolllock);

    // Generate a macro that does light config
    quote! {
        let light_config = ::rmk::config::LightConfig {
            capslock: #capslock,
            numslock: #numslock,
            scrolllock: #scrolllock,
        };
    }
}
