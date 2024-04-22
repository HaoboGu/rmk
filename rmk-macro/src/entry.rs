use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::{keyboard::CommunicationType, ChipModel, ChipSeries};

pub(crate) fn expand_rmk_entry(
    chip: &ChipModel,
    communication_type: CommunicationType,
) -> TokenStream2 {
    match chip.series {
        ChipSeries::Stm32 => quote! {
            // TODO: Be compatible with all stm32s, which may not have `usb_otg` or `USB_OTG_HS`
            ::rmk::initialize_keyboard_with_config_and_run::<
                ::embassy_stm32::flash::Flash<'_, ::embassy_stm32::flash::Blocking>,
                ::embassy_stm32::usb_otg::Driver<'_, ::embassy_stm32::peripherals::USB_OTG_HS>,
                ::embassy_stm32::gpio::Input<'_, ::embassy_stm32::gpio::AnyPin>,
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
        },
        ChipSeries::Nrf52 => {
            match communication_type {
                CommunicationType::Usb => quote! {
                    ::rmk::initialize_keyboard_with_config_and_run::<
                        ::embassy_nrf::nvmc::Nvmc,
                        ::embassy_nrf::usb::Driver<'_, ::embassy_nrf::peripherals::USBD, ::embassy_nrf::usb::vbus_detect::HardwareVbusDetect>,
                        ::embassy_nrf::gpio::Input<'_, ::embassy_nrf::gpio::AnyPin>,
                        ::embassy_nrf::gpio::Output<'_, ::embassy_nrf::gpio::AnyPin>,
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
                },
                CommunicationType::Both => quote! {
                    ::rmk::initialize_nrf_ble_keyboard_with_config_and_run::<
                        ::embassy_nrf::usb::Driver<'_, ::embassy_nrf::peripherals::USBD, &::embassy_nrf::usb::vbus_detect::SoftwareVbusDetect>,
                        ::embassy_nrf::gpio::Input<'_, ::embassy_nrf::gpio::AnyPin>,
                        ::embassy_nrf::gpio::Output<'_, ::embassy_nrf::gpio::AnyPin>,
                        ROW,
                        COL,
                        NUM_LAYER,
                    >(
                        crate::keymap::KEYMAP,
                        input_pins,
                        output_pins,
                        Some(driver),
                        keyboard_config,
                        spawner,
                    )
                    .await;
                },
                CommunicationType::Ble => quote! {
                    // FIXME: This would result in an error when using nRF52840 + BLE ONLY
                    ::rmk::initialize_nrf_ble_keyboard_with_config_and_run::<
                        ::embassy_nrf::gpio::Input<'_, ::embassy_nrf::gpio::AnyPin>,
                        ::embassy_nrf::gpio::Output<'_, ::embassy_nrf::gpio::AnyPin>,
                        ROW,
                        COL,
                        NUM_LAYER,
                    >(
                        crate::keymap::KEYMAP,
                        input_pins,
                        output_pins,
                        keyboard_config,
                        spawner,
                    )
                    .await;
                },
                CommunicationType::None => quote! {},
            }
        }
        ChipSeries::Rp2040 => quote! {
            ::rmk::initialize_keyboard_with_config_and_run_async_flash::<
                ::embassy_rp::flash::Flash<::embassy_rp::peripherals::FLASH, ::embassy_rp::flash::Async, FLASH_SIZE>,
                ::embassy_rp::usb::Driver<'_, ::embassy_rp::peripherals::USB>,
                ::embassy_rp::gpio::Input<'_, ::embassy_rp::gpio::AnyPin>,
                ::embassy_rp::gpio::Output<'_, ::embassy_rp::gpio::AnyPin>,
                ROW,
                COL,
                NUM_LAYER,
            >(
                driver,
                input_pins,
                output_pins,
                Some(flash),
                crate::keymap::KEYMAP,
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
                crate::keymap::KEYMAP,
                input_pins,
                output_pins,
                keyboard_config,
            ));
        },
        ChipSeries::Unsupported => quote! {},
    }
}
