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
        .map(|p| (p.clone(), convert_gpio_str_to_output_pin(chip, p, false)))
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
    async_matrix: bool,
) -> proc_macro2::TokenStream {
    let mut initializers = proc_macro2::TokenStream::new();
    let mut idents = vec![];
    let pin_initializers = pins
        .into_iter()
        .map(|p| {
            (
                p.clone(),
                convert_gpio_str_to_input_pin(chip, p, async_matrix),
            )
        })
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
    low_active: bool,
) -> proc_macro2::TokenStream {
    let gpio_ident = format_ident!("{}", gpio_name);
    let default_level_ident = if low_active {
        format_ident!("High")
    } else {
        format_ident!("Low")
    };
    match chip.series {
        ChipSeries::Stm32 => {
            quote! {
                ::embassy_stm32::gpio::Output::new(p.#gpio_ident, ::embassy_stm32::gpio::Level::#default_level_ident, ::embassy_stm32::gpio::Speed::VeryHigh).degrade()
            }
        }
        ChipSeries::Nrf52 => {
            quote! {
                ::embassy_nrf::gpio::Output::new(::embassy_nrf::gpio::AnyPin::from(p.#gpio_ident), ::embassy_nrf::gpio::Level::#default_level_ident, ::embassy_nrf::gpio::OutputDrive::Standard)
            }
        }
        ChipSeries::Rp2040 => {
            quote! {
                ::embassy_rp::gpio::Output::new(::embassy_rp::gpio::AnyPin::from(p.#gpio_ident), ::embassy_rp::gpio::Level::#default_level_ident)
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
    async_matrix: bool,
) -> proc_macro2::TokenStream {
    let gpio_ident = format_ident!("{}", gpio_name);
    match chip.series {
        ChipSeries::Stm32 => {
            if async_matrix {
                // If async_matrix is enabled, use ExtiInput for input pins
                match get_pin_num_stm32(&gpio_name) {
                    Some(pin_num) => {
                        let pin_num_ident = format_ident!("EXTI{}", pin_num);
                        quote! {
                            ::embassy_stm32::exti::ExtiInput::new(::embassy_stm32::gpio::Input::new(p.#gpio_ident, ::embassy_stm32::gpio::Pull::Down).degrade(), p.#pin_num_ident.degrade())
                        }
                    }
                    None => {
                        let message = format!("Invalid pin definition: {}", gpio_name);
                        quote! { compile_error!(#message); }
                    }
                }
            } else {
                quote! {
                    ::embassy_stm32::gpio::Input::new(p.#gpio_ident, ::embassy_stm32::gpio::Pull::Down).degrade()
                }
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

/// Get pin number from pin str.
/// For example, if the pin str is "PD13", this function will return "13".
fn get_pin_num_stm32(gpio_name: &String) -> Option<String> {
    if gpio_name.len() < 3 {
        None
    } else {
        Some(gpio_name[2..].to_string())
    }
}
