//! Initialize communication boilerplate of RMK, including USB or BLE
//!

use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident, quote};
use rmk_config::resolved::Hardware;
use rmk_config::resolved::hardware::ChipSeries;
use syn::{ItemFn, ItemMod};

use crate::codegen::override_helper::Overwritten;

pub(crate) fn expand_usb_init(hardware: &Hardware, item_mod: &ItemMod) -> TokenStream2 {
    // If there is a function with `#[Overwritten(usb)]`, override the chip initialization
    if let Some((_, items)) = &item_mod.content {
        items
            .iter()
            .find_map(|item| {
                if let syn::Item::Fn(item_fn) = &item
                    && item_fn.attrs.len() == 1
                    && let Ok(Overwritten::Usb) = Overwritten::from_meta(&item_fn.attrs[0].meta)
                {
                    return Some(override_usb_init(item_fn));
                }
                None
            })
            .unwrap_or(usb_config_default(hardware))
    } else {
        usb_config_default(hardware)
    }
}

/// Default implementation of usb initialization.
///
/// Also emits a `type RmkUsbDriverTy = ...;` aliased to the chip's concrete
/// `Driver` type so downstream codegen (e.g. `rmk_protocol`'s static
/// `UsbServerStorage<RmkUsbDriverTy>`) can reference it without duplicating
/// chip-specific knowledge.
pub(crate) fn usb_config_default(hardware: &Hardware) -> TokenStream2 {
    if let Some(usb_info) = hardware.communication.get_usb_info() {
        let peripheral_name = format_ident!("{}", usb_info.peripheral_name);
        let peripheral_path = quote! { ::embassy_stm32::peripherals::#peripheral_name };
        let _ = peripheral_path; // currently only used in some chip arms
        match hardware.chip.series {
            ChipSeries::Stm32 => {
                let dp = format_ident!("{}", usb_info.dp);
                let dm = format_ident!("{}", usb_info.dm);
                if usb_info.peripheral_name.contains("OTG") {
                    quote! {
                        static EP_OUT_BUFFER: ::static_cell::StaticCell<[u8; 1024]> = ::static_cell::StaticCell::new();
                        let mut usb_config = ::embassy_stm32::usb::Config::default();
                        usb_config.vbus_detection = false;
                        type RmkUsbDriverTy = ::embassy_stm32::usb::Driver<'static, ::embassy_stm32::peripherals::#peripheral_name>;
                        let driver: RmkUsbDriverTy = ::embassy_stm32::usb::Driver::new_fs(
                            p.#peripheral_name,
                            Irqs,
                            p.#dp,
                            p.#dm,
                            &mut EP_OUT_BUFFER.init([0; 1024])[..],
                            usb_config,
                        );
                    }
                } else {
                    quote! {
                        {
                            let _dp = ::embassy_stm32::gpio::Output::new(p.#dp.reborrow(), ::embassy_stm32::gpio::Level::Low, ::embassy_stm32::gpio::Speed::Low);
                            ::embassy_time::Timer::after_millis(10).await;
                        }
                        // Usb driver
                        type RmkUsbDriverTy = ::embassy_stm32::usb::Driver<'static, ::embassy_stm32::peripherals::#peripheral_name>;
                        let driver: RmkUsbDriverTy = ::embassy_stm32::usb::Driver::new(p.#peripheral_name, Irqs, p.#dp, p.#dm);
                    }
                }
            }
            ChipSeries::Nrf52 => quote! {
                // use hardware vbus
                type RmkUsbDriverTy = ::embassy_nrf::usb::Driver<
                    'static,
                    ::embassy_nrf::peripherals::#peripheral_name,
                    ::embassy_nrf::usb::vbus_detect::HardwareVbusDetect,
                >;
                let driver: RmkUsbDriverTy = ::embassy_nrf::usb::Driver::new(p.#peripheral_name, Irqs, ::embassy_nrf::usb::vbus_detect::HardwareVbusDetect::new(Irqs));
            },
            ChipSeries::Rp2040 => quote! {
                type RmkUsbDriverTy = ::embassy_rp::usb::Driver<'static, ::embassy_rp::peripherals::#peripheral_name>;
                let driver: RmkUsbDriverTy = ::embassy_rp::usb::Driver::new(p.#peripheral_name, Irqs);
            },
            ChipSeries::Esp32 => {
                let dp = format_ident!("{}", usb_info.dp);
                let dm = format_ident!("{}", usb_info.dm);
                quote! {
                    static mut EP_MEMORY: [u8; 1024] = [0; 1024];
                    let usb = ::esp_hal::otg_fs::Usb::new(p.#peripheral_name, p.#dp, p.#dm);
                    let usb_config = ::esp_hal::otg_fs::asynch::Config::default();
                    type RmkUsbDriverTy = ::esp_hal::otg_fs::asynch::Driver<'static>;
                    let driver: RmkUsbDriverTy = ::esp_hal::otg_fs::asynch::Driver::new(usb, unsafe { &mut *core::ptr::addr_of_mut!(EP_MEMORY) }, usb_config);
                }
            }
        }
    } else {
        quote! {}
    }
}

fn override_usb_init(item_fn: &ItemFn) -> TokenStream2 {
    let initialization = item_fn.block.to_token_stream();
    quote! {
        let driver = #initialization;
    }
}
