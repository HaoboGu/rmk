//! Initialize light config boilerplate of RMK, including USB or BLE
//!
use quote::quote;
use rmk_config::{ChipModel, KeyboardTomlConfig, PinConfig};

use crate::gpio_config::convert_gpio_str_to_output_pin;

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

pub(crate) fn expand_light_config(keyboard_config: &KeyboardTomlConfig) -> proc_macro2::TokenStream {
    let chip = keyboard_config.get_chip_model().unwrap();
    let light_config = keyboard_config.get_light_config();
    let numslock = build_light_config(&chip, &light_config.numslock);
    let capslock = build_light_config(&chip, &light_config.capslock);
    let scrolllock = build_light_config(&chip, &light_config.scrolllock);

    // Generate a macro that does light config
    quote! {
        let light_config = ::rmk::config::LightConfig {
            capslock: #capslock,
            numslock: #numslock,
            scrolllock: #scrolllock,
        };
    }
}
