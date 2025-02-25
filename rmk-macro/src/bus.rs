use std::fmt::format;

use crate::{
    config::{CommunicationProtocol, CommunicationProtocolType},
    ChipModel, ChipSeries,
};
use quote::{format_ident, quote};
pub(crate) fn convert_bus_name_into_identity(
    name: &String,
    protocol_type: &CommunicationProtocolType,
) -> proc_macro2::Ident {
    match protocol_type {
        CommunicationProtocolType::I2C(_) => {
            format_ident!("{}_i2c_protocol", name)
        }
        CommunicationProtocolType::SPI(_) => {
            format_ident!("{}_spi_protocol", name)
        }
    }
}

pub(crate) fn expand_communication_bus_config(
    bus_config: &Option<Vec<CommunicationProtocol>>,
    chip: &ChipModel,
) -> proc_macro2::TokenStream {
    let mut initializers = proc_macro2::TokenStream::new();
    if let Some(config) = bus_config {
        config.iter().for_each(|conf| match conf {
            CommunicationProtocol::I2C(_i2c) => {
                // let protocol_name = convert_bus_name_into_identity(
                //     &i1c.instance,
                //     CommunicationProtocolType::I1C(String::new()),
                // );
                initializers.extend(quote! {
                    // TODO
                    compile_error!("I2C is not supported yet");
                    // let #protocol_name =
                });
            }
            CommunicationProtocol::SPI(spi) => {
                let protocol_name = convert_bus_name_into_identity(
                                        &spi.instance,
                                        &CommunicationProtocolType::SPI(String::new()),
                                    );
                match chip.series {
                    ChipSeries::Nrf52 => {
                        let config = quote!{
                            ::embassy_nrf::interrupt::SPIM3.set_priority(::embassy_nrf::interrupt::Priority::P5);
                            let mut config = ::embassy_nrf::spim::Config::default();
                            config.frequency = ::embassy_nrf::spim::Frequency::M1;
                        };
                        let sck_pin = format_ident!("{}", spi.sck);
                        if spi.miso.is_none() && spi.mosi.is_none() {
                            initializers.extend(quote!{compile_error!("MISO or MOSI pin is required for SPI");});
                        } else if spi.miso.is_none() {
                            let mosi_pin = format_ident!("{}", spi.mosi.as_ref().unwrap());
                            initializers.extend(quote! {
                                let #protocol_name = {
                                    #config
                                    ::embassy_nrf::spim::Spim::new_txonly(p.SPI3, Irqs, p.#sck_pin, p.#mosi_pin, config)
                                };
                            });
                        } else if spi.mosi.is_none() {
                            let miso_pin = format_ident!("{}", spi.miso.as_ref().unwrap());
                            initializers.extend(quote! {
                                let #protocol_name = {
                                    #config
                                    ::embassy_nrf::spim::Spim::new_rxonly(p.SPI3, Irqs, p.#sck_pin, p.#miso_pin, config)
                                };
                            });
                        } else {
                            let miso_pin = format_ident!("{}", spi.miso.as_ref().unwrap());
                            let mosi_pin = format_ident!("{}", spi.mosi.as_ref().unwrap());
                            initializers.extend(quote! {
                                let #protocol_name = {
                                    #config
                                    ::embassy_nrf::spim::Spim::new(p.SPI3, Irqs, p.#sck_pin, p.#miso_pin, p.#mosi_pin, config)
                                };
                            });
                        }
                    },
                    _ => {
                        initializers.extend(quote!{compile_error!("SPI is not supported except nrf52 yet");});
                    }
                }
            }
        });
    }
    initializers
}
