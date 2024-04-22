use crate::{ChipModel, ChipSeries};
use quote::{format_ident, quote};

pub(crate) fn convert_output_pins_to_initializers(
    chip: &ChipModel,
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
    chip: &ChipModel,
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
    chip: &ChipModel,
    gpio_name: String,
) -> proc_macro2::TokenStream {
    let gpio_ident = format_ident!("{}", gpio_name);
    match chip.series {
        ChipSeries::Stm32 => {
            quote! {
                ::embassy_stm32::gpio::Output::new(p.#gpio_ident, ::embassy_stm32::gpio::Level::Low, ::embassy_stm32::gpio::Speed::VeryHigh).degrade()
            }
        }
        ChipSeries::Nrf52 => {
            quote! {
                ::embassy_nrf::gpio::Output::new(AnyPin::from(p.#gpio_ident), ::embassy_nrf::gpio::Level::Low, ::embassy_nrf::gpio::OutputDrive::Standard)
            }
        }
        ChipSeries::Rp2040 => {
            quote! {
                ::embassy_rp::gpio::Output::new(::embassy_rp::gpio::AnyPin::from(p.#gpio_ident), ::embassy_rp::gpio::Level::Low)
            }
        }
        ChipSeries::Esp32 => {
            quote! {
                ::esp_idf_svc::hal::gpio::PinDriver::output(p.pins.#gpio_ident.downgrade_output()).unwrap()
            }
        }
        ChipSeries::Unsupported => todo!(),
    }
}

pub(crate) fn convert_gpio_str_to_input_pin(
    chip: &ChipModel,
    gpio_name: String,
) -> proc_macro2::TokenStream {
    let gpio_ident = format_ident!("{}", gpio_name);
    match chip.series {
        ChipSeries::Stm32 => {
            quote! {
                ::embassy_stm32::gpio::Input::new(p.#gpio_ident, ::embassy_stm32::gpio::Pull::Down).degrade()
            }
        }
        ChipSeries::Nrf52 => {
            quote! {
                ::embassy_nrf::gpio::Input::new(::embassy_nrf::gpio::AnyPin::from(p.#gpio_ident), ::embassy_nrf::gpio::Pull::Down)
            }
        }
        ChipSeries::Rp2040 => {
            quote! {
                ::embassy_rp::gpio::Input::new(::embassy_rp::gpio::AnyPin::from(p.#gpio_ident), ::embassy_rp::gpio::Pull::Down)
            }
        }
        ChipSeries::Esp32 => {
            quote! {
                ::esp_idf_svc::hal::gpio::PinDriver::input(p.pins.#gpio_ident.downgrade_input()).unwrap()
            }
        }
        ChipSeries::Unsupported => todo!(),
    }
}
