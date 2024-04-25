use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use rmk_config::toml_config::BleConfig;

use crate::{ChipModel, ChipSeries};

// Default implementations of chip initialization
pub(crate) fn expand_ble_config(
    chip: &ChipModel,
    ble_config: Option<BleConfig>,
) -> TokenStream2 {
    // Support nrf52 only (for now)
    if chip.series != ChipSeries::Nrf52 {
        if chip.series == ChipSeries::Esp32 {
            return quote! {
                let ble_battery_config = ::rmk::config::BleBatteryConfig::default();
            };
        } else {
            return quote!{};
        }
    }
    match ble_config {
        Some(ble) => {
            if ble.enabled {
                let mut ble_config_tokens = TokenStream2::new();
                if let Some(adc_pin) = ble.battery_adc_pin {
                    let adc_pin_ident = format_ident!("{}", adc_pin);
                    ble_config_tokens.extend(quote! {
                        use ::embassy_nrf::saadc::Input as _;

                        let adc_pin = p.#adc_pin_ident.degrade_saadc();
                        // Then we initialize the ADC. We are only using one channel in this example.
                        let config = ::embassy_nrf::saadc::Config::default();
                        let channel_cfg = ::embassy_nrf::saadc::ChannelConfig::single_ended(adc_pin);
                        ::embassy_nrf::interrupt::SAADC.set_priority(::embassy_nrf::interrupt::Priority::P3);
                        let saadc = ::embassy_nrf::saadc::Saadc::new(p.SAADC, Irqs, config, [channel_cfg]);
                        // Wait for ADC calibration.
                        saadc.calibrate().await;
                        let saadc_option = Some(saadc);
                    });
                } else {
                    ble_config_tokens.extend(quote! {
                        // TODO: Use full path of Option
                        let saadc_option: Option<::embassy_nrf::saadc::Saadc<'_, 1>> = None;
                    });
                };

                if let Some(charging_state_config) = ble.charge_state {
                    let charging_state_pin = format_ident!("{}", charging_state_config.pin);
                    ble_config_tokens.extend(quote! {
                        let is_charging_pin = Some(::embassy_nrf::gpio::Input::new(::embassy_nrf::gpio::AnyPin::from(p.#charging_state_pin), ::embassy_nrf::gpio::Pull::None));
                    });
                } else {
                    ble_config_tokens.extend(
                        quote! {
                            // TODO: Use full path of Option
                            let is_charging_pin: Option<::embassy_nrf::gpio::Input<'_, ::embassy_nrf::gpio::AnyPin>> = None;
                        }
                    )
                }

                ble_config_tokens.extend(
                    quote! {
                        let ble_battery_config = ::rmk::config::BleBatteryConfig::new(is_charging_pin, saadc_option);       
                    }
                );

                ble_config_tokens
            } else {
                quote! {
                    let ble_battery_config = ::rmk::config::BleBatteryConfig::default();
                }
            }
        }
        None => quote! {
            let ble_battery_config = ::rmk::config::BleBatteryConfig::default();
        }
    }
}
