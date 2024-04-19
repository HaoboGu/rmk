use quote::quote;
use rmk_config::toml_config::{KeyboardInfo, LightConfig, MatrixConfig, PinConfig};

use crate::gpio_config::{
    convert_gpio_str_to_output_pin, convert_input_pins_to_initializers,
    convert_output_pins_to_initializers,
};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ChipSeries {
    Stm32,
    Nrf52,
    Rp2040,
    Esp32,
    Unsupported,
}

pub(crate) fn get_chip_model(chip: String) -> ChipSeries {
    if chip.to_lowercase().starts_with("stm32") {
        return ChipSeries::Stm32;
    } else if chip.to_lowercase().starts_with("nrf52") {
        return ChipSeries::Nrf52;
    } else if chip.to_lowercase().starts_with("rp2040") {
        return ChipSeries::Rp2040;
    } else if chip.to_lowercase().starts_with("esp32") {
        return ChipSeries::Esp32;
    } else {
        return ChipSeries::Unsupported;
    }
}

pub(crate) fn expand_keyboard_info(
    keyboard_info: KeyboardInfo,
    num_row: usize,
    num_col: usize,
    num_layer: usize,
) -> proc_macro2::TokenStream {
    let pid = keyboard_info.product_id;
    let vid = keyboard_info.vendor_id;
    let product_name = keyboard_info
        .product_name
        .unwrap_or("RMK Keyboard".to_string());
    let manufacturer = keyboard_info.manufacturer.unwrap_or("RMK".to_string());
    let serial_number = keyboard_info.serial_number.unwrap_or("0000000".to_string());
    quote! {
        pub(crate) const COL: usize = #num_col;
        pub(crate) const ROW: usize = #num_row;
        pub(crate) const NUM_LAYER: usize = #num_layer;
        static keyboard_usb_config: ::rmk_config::keyboard_config::KeyboardUsbConfig = ::rmk_config::keyboard_config::KeyboardUsbConfig {
            vid: #vid,
            pid: #pid,
            manufacturer: #manufacturer,
            product_name: #product_name,
            serial_number: #serial_number,
        };
    }
}

pub(crate) fn expand_vial_config() -> proc_macro2::TokenStream {
    quote! {
        static vial_config: ::rmk_config::keyboard_config::VialConfig = ::rmk_config::keyboard_config::VialConfig {
            vial_keyboard_id: &VIAL_KEYBOARD_ID,
            vial_keyboard_def: &VIAL_KEYBOARD_DEF,
        };
    }
}

pub(crate) fn build_light_config(
    chip: &ChipSeries,
    pin_config: Option<PinConfig>,
) -> proc_macro2::TokenStream {
    match pin_config {
        Some(c) => {
            let p = convert_gpio_str_to_output_pin(chip, c.pin);
            let low_active = c.low_active;
            quote! {
                Some(::rmk_config::keyboard_config::LightPinConfig {
                    pin: #p,
                    low_active: #low_active,
                })
            }
        }
        None => quote! {None},
    }
}

pub(crate) fn expand_light_config(
    chip: &ChipSeries,
    light_config: LightConfig,
) -> proc_macro2::TokenStream {
    let numslock = build_light_config(chip, light_config.numslock);
    let capslock = build_light_config(chip, light_config.capslock);
    let scrolllock = build_light_config(chip, light_config.scrolllock);

    // Generate a macro that does light config
    quote! {
        ::rmk_config::keyboard_config::LightConfig {
            capslock: #capslock,
            numslock: #numslock,
            scrolllock: #scrolllock,
        }
    }
}

pub(crate) fn expand_matrix_config(
    chip: &ChipSeries,
    matrix_config: MatrixConfig,
) -> proc_macro2::TokenStream {
    let mut pin_initialization = proc_macro2::TokenStream::new();
    // Initialize input pins
    pin_initialization.extend(convert_input_pins_to_initializers(
        &chip,
        matrix_config.input_pins,
    ));
    // Initialize output pins
    pin_initialization.extend(convert_output_pins_to_initializers(
        &chip,
        matrix_config.output_pins,
    ));

    // Generate a macro that does pin matrix config
    quote! {
        {
            #pin_initialization
            (output_pins, input_pins)
        }
    }
}
