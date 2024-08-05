use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{ItemFn, ItemMod};

use crate::{
    keyboard::{CommunicationType, Overwritten},
    usb_interrupt_map::UsbInfo,
    ChipModel, ChipSeries,
};

pub(crate) fn expand_rmk_entry(
    chip: &ChipModel,
    usb_info: &UsbInfo,
    communication_type: CommunicationType,
    item_mod: &ItemMod,
    async_matrix: bool,
) -> TokenStream2 {
    // If there is a function with `#[Overwritten(usb)]`, override the chip initialization
    if let Some((_, items)) = &item_mod.content {
        items
            .iter()
            .find_map(|item| {
                if let syn::Item::Fn(item_fn) = &item {
                    if item_fn.attrs.len() == 1 {
                        if let Ok(Overwritten::Entry) =
                            Overwritten::from_meta(&item_fn.attrs[0].meta)
                        {
                            return Some(override_rmk_entry(item_fn));
                        }
                    }
                }
                None
            })
            .unwrap_or(rmk_entry_default(
                chip,
                usb_info,
                communication_type,
                async_matrix,
            ))
    } else {
        rmk_entry_default(chip, usb_info, communication_type, async_matrix)
    }
}

fn override_rmk_entry(item_fn: &ItemFn) -> TokenStream2 {
    let content = &item_fn.block.stmts;
    quote! {
        #(#content)*
    }
}

pub(crate) fn rmk_entry_default(
    chip: &ChipModel,
    usb_info: &UsbInfo,
    communication_type: CommunicationType,
    async_matrix: bool,
) -> TokenStream2 {
    let peripheral_name = format_ident!("{}", usb_info.peripheral_name);
    let usb_mod_path = if usb_info.peripheral_name.contains("OTG") {
        format_ident!("{}", "usb_otg")
    } else {
        format_ident!("{}", "usb")
    };
    match chip.series {
        ChipSeries::Stm32 => {
            // If async_matrix is enabled, use `ExtiInput` as input pin type in RMK entry
            let input_pin_generics = if async_matrix {
                quote! {::embassy_stm32::exti::ExtiInput<::embassy_stm32::gpio::AnyPin>}
            } else {
                quote! {::embassy_stm32::gpio::Input<'_, ::embassy_stm32::gpio::AnyPin>}
            };
            quote! {
                ::rmk::initialize_keyboard_and_run::<
                    ::embassy_stm32::flash::Flash<'_, ::embassy_stm32::flash::Blocking>,
                    ::embassy_stm32::#usb_mod_path::Driver<'_, ::embassy_stm32::peripherals::#peripheral_name>,
                    #input_pin_generics,
                    ::embassy_stm32::gpio::Output<'_, ::embassy_stm32::gpio::AnyPin>,
                    ROW,
                    COL,
                    NUM_LAYER,
                >(
                    driver,
                    input_pins,
                    output_pins,
                    Some(f),
                    KEYMAP,
                    keyboard_config,
                )
                .await;
            }
        }
        ChipSeries::Nrf52 => match communication_type {
            CommunicationType::Usb => {
                quote! {
                    ::rmk::initialize_keyboard_and_run::<
                        ::embassy_nrf::nvmc::Nvmc,
                        ::embassy_nrf::usb::Driver<'_, ::embassy_nrf::peripherals::#peripheral_name, ::embassy_nrf::usb::vbus_detect::HardwareVbusDetect>,
                        ::embassy_nrf::gpio::Input<'_>,
                        ::embassy_nrf::gpio::Output<'_>,
                        ROW,
                        COL,
                        NUM_LAYER,
                    >(
                        driver,
                        input_pins,
                        output_pins,
                        Some(f),
                        KEYMAP,
                        keyboard_config,
                    )
                    .await;
                }
            }
            CommunicationType::Both => quote! {
                ::rmk::initialize_nrf_ble_keyboard_with_config_and_run::<
                    ::embassy_nrf::usb::Driver<'_, ::embassy_nrf::peripherals::#peripheral_name, &::embassy_nrf::usb::vbus_detect::SoftwareVbusDetect>,
                    ::embassy_nrf::gpio::Input<'_,>,
                    ::embassy_nrf::gpio::Output<'_,>,
                    ROW,
                    COL,
                    NUM_LAYER,
                >(
                    KEYMAP,
                    input_pins,
                    output_pins,
                    Some(driver),
                    keyboard_config,
                    spawner,
                )
                .await;
            },
            CommunicationType::Ble => quote! {
                ::rmk::initialize_nrf_ble_keyboard_with_config_and_run::<
                    ::embassy_nrf::gpio::Input<'_>,
                    ::embassy_nrf::gpio::Output<'_>,
                    ROW,
                    COL,
                    NUM_LAYER,
                >(
                    KEYMAP,
                    input_pins,
                    output_pins,
                    keyboard_config,
                    spawner,
                )
                .await;
            },
            CommunicationType::None => quote! {},
        },
        ChipSeries::Rp2040 => quote! {
            ::rmk::initialize_keyboard_and_run_async_flash::<
                ::embassy_rp::flash::Flash<::embassy_rp::peripherals::FLASH, ::embassy_rp::flash::Async, FLASH_SIZE>,
                ::embassy_rp::usb::Driver<'_, ::embassy_rp::peripherals::USB>,
                ::embassy_rp::gpio::Input<'_>,
                ::embassy_rp::gpio::Output<'_>,
                ROW,
                COL,
                NUM_LAYER,
            >(
                driver,
                input_pins,
                output_pins,
                Some(flash),
                KEYMAP,
                keyboard_config,
            )
            .await;
        },
        ChipSeries::Esp32 => quote! {
            ::esp_idf_svc::hal::task::block_on(::rmk::initialize_esp_ble_keyboard_with_config_and_run::<
                ::esp_idf_svc::hal::gpio::PinDriver<'_, ::esp_idf_svc::hal::gpio::AnyInputPin, ::esp_idf_svc::hal::gpio::Input>,
                ::esp_idf_svc::hal::gpio::PinDriver<'_, ::esp_idf_svc::hal::gpio::AnyOutputPin, ::esp_idf_svc::hal::gpio::Output>,
                ROW,
                COL,
                NUM_LAYER,
            >(
                KEYMAP,
                input_pins,
                output_pins,
                keyboard_config,
            ));
        },
        ChipSeries::Unsupported => quote! {},
    }
}
