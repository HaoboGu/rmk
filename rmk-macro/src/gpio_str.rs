use crate::ChipSeries;
use quote::{format_ident, quote};

pub(crate) fn convert_gpio_str_to_pin(
    chip: &ChipSeries,
    gpio_name: String,
) -> proc_macro2::TokenStream {
    let gpio_ident = format_ident!("{}", gpio_name);
    match chip {
        ChipSeries::Stm32 => {
            quote! {
                Some(embassy_stm32::gpio::Output::new($p.#gpio_ident, embassy_stm32::gpio::Level::Low, embassy_stm32::gpio::Speed::VeryHigh).degrade())
            }
        }
        ChipSeries::Nrf52 => todo!(),
        ChipSeries::Rp2040 => todo!(),
        ChipSeries::Esp32 => todo!(),
        ChipSeries::Unsupported => todo!(),
    }
}
