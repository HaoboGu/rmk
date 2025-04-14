use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

use crate::keyboard_config::{CommunicationConfig, KeyboardConfig};
use crate::ChipSeries;

// Default implementations of ble configuration.
// Because ble configuration in `config` is enabled by a feature gate, so this function returns two TokenStreams.
// One for initialization ble config, another one for filling this field into `RmkConfig`.
pub(crate) fn expand_ble_config(keyboard_config: &KeyboardConfig) -> (TokenStream2, TokenStream2) {
    if !keyboard_config.communication.ble_enabled() {
        return (quote! {}, quote! {});
    }
    // Support only nrf52 and esp32 (for now)
    if keyboard_config.chip.series != ChipSeries::Nrf52 {
        if keyboard_config.chip.series == ChipSeries::Esp32 {
            return (
                quote! {
                    let ble_battery_config = ::rmk::config::BleBatteryConfig::default();
                },
                quote! {
                    ble_battery_config,
                },
            );
        } else {
            return (quote! {}, quote! {});
        }
    }
    match &keyboard_config.communication {
        CommunicationConfig::Ble(ble) | CommunicationConfig::Both(_, ble) => {
            if ble.enabled {
                let mut ble_config_tokens = TokenStream2::new();
                if let Some(charging_state_config) = ble.charge_state.clone() {
                    let charging_state_pin = format_ident!("{}", charging_state_config.pin);
                    let low_active = charging_state_config.low_active;
                    let pull = if low_active {
                        quote! { ::embassy_nrf::gpio::Pull::Up }
                    } else {
                        quote! { ::embassy_nrf::gpio::Pull::Down }
                    };
                    ble_config_tokens.extend(quote! {
                        let is_charging_pin = Some(::embassy_nrf::gpio::Input::new(p.#charging_state_pin, #pull));
                        let charging_state_low_active = #low_active;
                    });
                } else {
                    ble_config_tokens.extend(quote! {
                        let charging_state_low_active = false;
                        let is_charging_pin: ::core::option::Option<::embassy_nrf::gpio::Input<'_>> = None;
                    })
                }

                if let Some(charging_led_config) = ble.charge_led.clone() {
                    let charging_led_pin = format_ident!("{}", charging_led_config.pin);
                    let charging_led_low_active = charging_led_config.low_active;
                    let default_level = if charging_led_low_active {
                        quote! { ::embassy_nrf::gpio::Level::High }
                    } else {
                        quote! { ::embassy_nrf::gpio::Level::Low }
                    };
                    ble_config_tokens.extend(quote! {
                        let charge_led_pin = Some(::embassy_nrf::gpio::Output::new(p.#charging_led_pin, #default_level, ::embassy_nrf::gpio::OutputDrive::Standard));
                        let charge_led_low_active = #charging_led_low_active;
                    });
                } else {
                    ble_config_tokens.extend(quote! {
                        let charge_led_low_active = false;
                        let charge_led_pin: ::core::option::Option<::embassy_nrf::gpio::Output<'_>>  = None;
                    })
                }

                ble_config_tokens.extend(
                    quote! {
                        let ble_battery_config = ::rmk::config::BleBatteryConfig::new(is_charging_pin, charging_state_low_active, charge_led_pin, charge_led_low_active);
                    }
                );

                (
                    ble_config_tokens,
                    quote! {
                        ble_battery_config,
                    },
                )
            } else {
                (
                    quote! {
                        let ble_battery_config = ::rmk::config::BleBatteryConfig::default();
                    },
                    quote! {
                        ble_battery_config,
                    },
                )
            }
        }
        _ => (quote! {}, quote! {}),
    }
}
