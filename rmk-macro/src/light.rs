//! Initialize light config boilerplate of RMK, including USB or BLE
//! 
use rmk_config::toml_config::{PinConfig, LightConfig};
use quote::quote;

use crate::{gpio_config::convert_gpio_str_to_output_pin, ChipModel};

pub(crate) fn build_light_config(
    chip: &ChipModel,
    pin_config: Option<PinConfig>,
) -> proc_macro2::TokenStream {
    match pin_config {
        Some(c) => {
            let p = convert_gpio_str_to_output_pin(chip, c.pin);
            let low_active = c.low_active;
            quote! {
                Some(::rmk::config::keyboard_config::LightPinConfig {
                    pin: #p,
                    low_active: #low_active,
                })
            }
        }
        None => quote! {None},
    }
}


pub(crate) fn expand_light_config(
    chip: &ChipModel,
    light_config: LightConfig,
) -> proc_macro2::TokenStream {
    let numslock = build_light_config(chip, light_config.numslock);
    let capslock = build_light_config(chip, light_config.capslock);
    let scrolllock = build_light_config(chip, light_config.scrolllock);

    // Generate a macro that does light config
    quote! {
        let light_config = ::rmk::config::keyboard_config::LightConfig {
            capslock: #capslock,
            numslock: #numslock,
            scrolllock: #scrolllock,
        };
    }
}