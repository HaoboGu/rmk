use crate::ChipSeries;
use quote::{format_ident, quote};

pub(crate) fn convert_output_pins_to_initializers(
    chip: &ChipSeries,
    pins: Vec<String>,
) -> proc_macro2::TokenStream {
    let mut initializers = proc_macro2::TokenStream::new();
    let mut idents = vec![];
    let pin_initializers = pins
        .into_iter()
        .map(|p| (p.clone(), convert_gpio_str_to_output_pin(chip, p)))
        .map(|(p, ts)| {
            let ident_name = format_ident!("{}", p.to_lowercase());
            idents.push(ident_name.clone());
            quote! { let #ident_name = #ts;}
        });

    initializers.extend(pin_initializers);
    initializers.extend(quote! {let input_pins = [#(#idents), *];});
    initializers
}

pub(crate) fn convert_input_pins_to_initializers(
    chip: &ChipSeries,
    pins: Vec<String>,
) -> proc_macro2::TokenStream {
    let mut initializers = proc_macro2::TokenStream::new();
    let mut idents = vec![];
    let pin_initializers = pins
        .into_iter()
        .map(|p| (p.clone(), convert_gpio_str_to_input_pin(chip, p)))
        .map(|(p, ts)| {
            let ident_name = format_ident!("{}", p.to_lowercase());
            idents.push(ident_name.clone());
            quote! { let #ident_name = #ts;}
        });
    initializers.extend(pin_initializers);
    initializers.extend(quote! {let output_pins = [#(#idents), *];});
    initializers
}

pub(crate) fn convert_gpio_str_to_output_pin(
    chip: &ChipSeries,
    gpio_name: String,
) -> proc_macro2::TokenStream {
    let gpio_ident = format_ident!("{}", gpio_name);
    match chip {
        ChipSeries::Stm32 => {
            quote! {
                ::embassy_stm32::gpio::Output::new($p.#gpio_ident, ::embassy_stm32::gpio::Level::Low, ::embassy_stm32::gpio::Speed::VeryHigh).degrade()
            }
        }
        ChipSeries::Nrf52 => todo!(),
        ChipSeries::Rp2040 => todo!(),
        ChipSeries::Esp32 => todo!(),
        ChipSeries::Unsupported => todo!(),
    }
}

pub(crate) fn convert_gpio_str_to_input_pin(
    chip: &ChipSeries,
    gpio_name: String,
) -> proc_macro2::TokenStream {
    let gpio_ident = format_ident!("{}", gpio_name);
    match chip {
        ChipSeries::Stm32 => {
            quote! {
                ::embassy_stm32::gpio::Input::new($p.#gpio_ident, ::embassy_stm32::gpio::Pull::Down).degrade()
            }
        }
        ChipSeries::Nrf52 => todo!(),
        ChipSeries::Rp2040 => todo!(),
        ChipSeries::Esp32 => todo!(),
        ChipSeries::Unsupported => todo!(),
    }
}
