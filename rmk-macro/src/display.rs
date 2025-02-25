use std::convert;

use crate::{
    bus::convert_bus_name_into_identity,
    config::{CommunicationProtocolType, DisplayConfig},
};
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

pub(crate) fn convert_display_name_into_identity(
    config_name: &Option<String>,
) -> proc_macro2::Ident {
    let name = if let Some(n) = config_name {
        n
    } else {
        "default"
    };
    let ident = format_ident!("{}_display", name);
    ident
}

pub(crate) fn expand_display_config(config: &Option<DisplayConfig>) -> TokenStream2 {
    if let Some(conf) = config {
        let bus_ident = convert_bus_name_into_identity(&conf.bus, &conf.interface);
        let display_name = convert_display_name_into_identity(&conf.instance);
        match &conf.interface {
            CommunicationProtocolType::I2C(_address) => {
                quote! {
                    compile_error!("I2C protocol display is not supported yet");
                }
            }
            CommunicationProtocolType::SPI(cs) => {
                let cs_pin = format_ident!("{}", cs);
                quote! {
                    let #display_name =
                        ::rmk::display::memory_lcd_spi::MemoryLCD::<::rmk::display::NiceView, _, _>::new(
                            #bus_ident,
                            ::embassy_nrf::gpio::Output::new(
                                ::embassy_nrf::gpio::AnyPin::from(p.#cs_pin),
                                ::embassy_nrf::gpio::Level::High,
                                ::embassy_nrf::gpio::OutputDrive::Standard
                            ));
                }
            }
        }
    } else {
        quote! {}
    }
}
