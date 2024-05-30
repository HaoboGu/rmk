use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use rmk_config::toml_config::BleConfig;

use crate::{keyboard::CommunicationType, ChipModel, ChipSeries};

// Default implementations of ble configuration.
// Because ble configuration in `rmk_config` is enabled by a feature gate, so this function returns two TokenStreams.
// One for initialization ble config, another one for filling this field into `RmkConfig`.
pub(crate) fn expand_ble_config(
    chip:&ChipModel,
    comm_type: CommunicationType,
    ble_config: Option<BleConfig>,
) -> (TokenStream2, TokenStream2) {
    if !comm_type.ble_enabled() { 
        return (quote! {}, quote! {});
    }
    // Support nrf52 only (for now)
    if chip.series != ChipSeries::Nrf52 {
        if chip.series == ChipSeries::Esp32 {
            return (quote! {
                let ble_battery_config = ::rmk::config::BleBatteryConfig::default();
            }, quote! {
                ble_battery_config,
            });
        } else {
            return (quote!{}, quote!{});
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
                        let saadc_option: ::core::option::Option<::embassy_nrf::saadc::Saadc<'_, 1>> = None;
                    });
                };

                if let Some(charging_state_config) = ble.charge_state {
                    let charging_state_pin = format_ident!("{}", charging_state_config.pin);
                    let low_active = charging_state_config.low_active;
                    ble_config_tokens.extend(quote! {
                        let is_charging_pin = Some(::embassy_nrf::gpio::Input::new(::embassy_nrf::gpio::AnyPin::from(p.#charging_state_pin), ::embassy_nrf::gpio::Pull::None));
                        let charging_state_low_active = #low_active;
                    });
                } else {
                    ble_config_tokens.extend(
                        quote! {
                            let charging_state_low_active = false;
                            let is_charging_pin: ::core::option::Option<::embassy_nrf::gpio::Input<'_, ::embassy_nrf::gpio::AnyPin>> = None;
                        }
                    )
                }

                if let Some(charging_led_config) = ble.charge_led {
                    let charging_led_pin = format_ident!("{}", charging_led_config.pin);
                    let charging_led_low_active = charging_led_config.low_active;
                    ble_config_tokens.extend(quote! {
                        let charge_led_pin = Some(::embassy_nrf::gpio::Output::new(::embassy_nrf::gpio::AnyPin::from(p.#charging_led_pin), ::embassy_nrf::gpio::Level::Low, ::embassy_nrf::gpio::OutputDrive::Standard));
                        let charge_led_low_active = #charging_led_low_active;
                    });
                } else {
                    ble_config_tokens.extend(
                        quote! {
                            let charge_led_low_active = false;
                            let charge_led_pin: ::core::option::Option<::embassy_nrf::gpio::Output<'_, ::embassy_nrf::gpio::AnyPin>>  = None;
                        }
                    )
                }

                ble_config_tokens.extend(
                    quote! {
                        let ble_battery_config = ::rmk::config::BleBatteryConfig::new(is_charging_pin, charging_state_low_active, charge_led_pin, charge_led_low_active, saadc_option);       
                    }
                );

                (ble_config_tokens, quote! {
                    ble_battery_config,
                })
            } else {
                (quote! {
                    let ble_battery_config = ::rmk::config::BleBatteryConfig::default();
                }, quote! {
                    ble_battery_config,
                })
            }
        }
        None => (quote! {
            let ble_battery_config = ::rmk::config::BleBatteryConfig::default();
        }, quote! {
            ble_battery_config,
        })
    }
}
