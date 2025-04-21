use core::panic;

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

use crate::config::{SerialConfig, SplitConfig};
use crate::keyboard_config::{BoardConfig, KeyboardConfig};
use crate::{ChipModel, ChipSeries};

pub(crate) fn expand_split_central_config(config: &KeyboardConfig) -> proc_macro2::TokenStream {
    if let BoardConfig::Split(split_config) = &config.board {
        expand_split_communication_config(&config.chip, split_config)
    } else {
        quote! {}
    }
}

fn expand_split_communication_config(chip: &ChipModel, split_config: &SplitConfig) -> TokenStream2 {
    match &split_config.connection[..] {
        "ble" => {
            // We need to create addrs for BLE
            let num_peripheral = split_config.peripheral.len();
            quote! {
                // Read peripheral address from storage
                let peripheral_addrs = ::rmk::split::ble::central::read_peripheral_addresses::<#num_peripheral, _, ROW, COL, NUM_LAYER, NUM_ENCODER>(&mut storage).await;
            }
        }
        "serial" => {
            // We need to initialize serial instance for serial
            let serial_config: Vec<SerialConfig> =
                split_config.central.serial.clone().expect("central.serial is required");
            expand_serial_init(chip, serial_config)
        }
        _ => panic!("Invalid connection type for split"),
    }
}

pub(crate) fn expand_serial_init(chip: &ChipModel, serial: Vec<SerialConfig>) -> TokenStream2 {
    let mut uart_initializers = proc_macro2::TokenStream::new();
    serial.iter().enumerate().for_each(|(idx, s)| {
        let tx_buf_static = format_ident!("TX_BUF{}", idx);
        let rx_buf_static = format_ident!("RX_BUF{}", idx);
        let tx_buf_name = format_ident!("tx_buf{}", idx);
        let rx_buf_name = format_ident!("rx_buf{}", idx);
        let uart_buf_init = quote! {
            static #tx_buf_static: ::static_cell::StaticCell<[u8; ::rmk::split::SPLIT_MESSAGE_MAX_SIZE]> = ::static_cell::StaticCell::new();
            let #tx_buf_name = &mut #tx_buf_static.init([0_u8; ::rmk::split::SPLIT_MESSAGE_MAX_SIZE])[..];
            static #rx_buf_static: ::static_cell::StaticCell<[u8; ::rmk::split::SPLIT_MESSAGE_MAX_SIZE]> = ::static_cell::StaticCell::new();
            let #rx_buf_name = &mut #rx_buf_static.init([0_u8; ::rmk::split::SPLIT_MESSAGE_MAX_SIZE])[..];
        };
        let uart_init = match chip.series {
            ChipSeries::Rp2040 => {
                let uart_instance = format_ident!("{}", s.instance);
                let uart_name = format_ident!("{}", s.instance.to_lowercase());
                let tx_pin = format_ident!("{}", s.tx_pin);
                let rx_pin = format_ident!("{}", s.rx_pin);
                let irq_name = format_ident!("IrqsUart{}", idx);
                match &s.instance {
                    i if i.starts_with("UART") => {
                        let uart_irq = format_ident!("{}_IRQ", s.instance);
                        quote! {
                            ::embassy_rp::bind_interrupts!(struct #irq_name {
                                #uart_irq => ::embassy_rp::uart::BufferedInterruptHandler<::embassy_rp::peripherals::#uart_instance>;
                            });
                            let #uart_name = ::embassy_rp::uart::BufferedUart::new(
                                p.#uart_instance,
                                #irq_name,
                                p.#tx_pin,
                                p.#rx_pin,
                                #tx_buf_name,
                                #rx_buf_name,
                                ::embassy_rp::uart::Config::default(),
                            );
                        }
                    }
                    i if i.starts_with("PIO") => {
                        let uart_irq = format_ident!("{}_IRQ_0", s.instance);
                        let instance_init = if s.rx_pin.eq(&s.tx_pin) {
                            quote! {
                                let #uart_name = ::rmk::split::rp::uart::BufferedUart::new_half_duplex(
                                    p.#uart_instance,
                                    p.#rx_pin,
                                    #rx_buf_name,
                                    #irq_name,
                                );
                            }
                        } else {
                            quote! {
                                let #uart_name = ::rmk::split::rp::uart::BufferedUart::new_full_duplex(
                                    p.#uart_instance,
                                    p.#tx_pin,
                                    p.#rx_pin,
                                    #tx_buf_name,
                                    #rx_buf_name,
                                    #irq_name,
                                );
                            }
                        };
                        quote! {
                            ::embassy_rp::bind_interrupts!(struct #irq_name {
                                #uart_irq => ::rmk::split::rp::uart::UartInterruptHandler<::embassy_rp::peripherals::#uart_instance>;
                            });
                            #instance_init
                        }
                    }
                    _ => panic!("Serial instance {:?} is not recognised", s.instance),
                }
            }
            _ => panic!("Serial for chip {:?} isn't implemented yet", chip.series),
        };
        uart_initializers.extend(quote! {
            #uart_buf_init
            #uart_init
        });
    });
    uart_initializers
}
