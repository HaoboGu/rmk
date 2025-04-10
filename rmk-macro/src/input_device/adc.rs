use crate::{
    config::{BleConfig, JoystickConfig},
    ChipSeries,
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub(crate) fn expand_adc_device(
    joystick_config: Vec<JoystickConfig>,
    ble_config: Option<BleConfig>,
    chip_model: ChipSeries,
) -> (TokenStream, Vec<TokenStream>) {
    match chip_model {
        ChipSeries::Nrf52 => {
            let mut channel_cfg = vec![];
            let mut adc_type = vec![];
            let mut processor_name = vec![];
            let mut config = TokenStream::new();
            let mut default_polling_interval = 30000u16; // default 30s
            let mut light_sleep: Option<u16> = None;
            // TODO: deep sleep

            if let Some(ble) = ble_config {
                if ble.enabled {
                    if let Some(adc_pin) = ble.battery_adc_pin {
                        let adc_pin_def = if adc_pin == "vddh" {
                            quote! {
                                saadc::ChannelConfig::single_ended(saadc::VddhDiv5Input.degrade_saadc())
                            }
                        } else {
                            let adc_pin_def = format_ident!("{}", adc_pin);
                            quote! {
                                saadc::ChannelConfig::single_ended(p.#adc_pin_def.degrade_saadc())
                            }
                        };
                        channel_cfg.push(adc_pin_def);
                        adc_type.push(quote! {
                            ::rmk::input_device::adc::AnalogEventType::Battery
                        });

                        let (adc_divider_measured, adc_divider_total) =
                            (ble.adc_divider_measured, ble.adc_divider_total);
                        let bat_ident = format_ident!("battery_processor",);
                        config.extend(quote! {
                            let mut #bat_ident = ::rmk::input_device::battery::BatteryProcessor::new(#adc_divider_measured, #adc_divider_total, &keymap);
                        });
                        processor_name.push(quote!(#bat_ident));
                    }
                }
            }

            // polling interval with joystick
            if !joystick_config.is_empty() {
                default_polling_interval = 20;
                light_sleep = Some(350);
            }

            for joystick in joystick_config {
                let mut cnt = 0u8;
                for pin in [joystick.pin_x, joystick.pin_y, joystick.pin_z].iter() {
                    if pin == "_" {
                        break;
                    }
                    let adc_pin_def = format_ident!("{}", pin);
                    channel_cfg.push(quote! {
                        saadc::ChannelConfig::single_ended(p.#adc_pin_def.degrade_saadc())
                    });
                    cnt += 1;
                }

                adc_type.push(quote! {
                    ::rmk::input_device::adc::AnalogEventType::Joystick(#cnt)
                });
                let joy_ident = format_ident!("joystick_processor_{}", joystick.name);
                processor_name.push(quote!(#joy_ident));
                let JoystickConfig {
                    transform,
                    bias,
                    resolution,
                    ..
                } = joystick;
                config.extend(quote! {
                    let mut #joy_ident = rmk::input_device::joystick::JoystickProcessor::new([#([#(#transform),*]),*], [#(#bias),*], #resolution, &keymap);
                });
            }

            if !processor_name.is_empty() {
                let light_sleep_param = if let Some(light_sleep_interval) = light_sleep {
                    quote! {Some(#light_sleep_interval)}
                } else {
                    quote! {None}
                };
                config.extend(quote! {
                    let mut adc_device = {
                        use embassy_nrf::saadc::{self, Input as _};
                        let saadc_config = saadc::Config::default();
                        embassy_nrf::interrupt::SAADC.set_priority(embassy_nrf::interrupt::Priority::P3);

                        let adc = saadc::Saadc::new(p.SAADC, Irqs, saadc_config, [#(#channel_cfg),*]);
                        adc.calibrate().await;

                        rmk::input_device::adc::NrfAdc::new(
                                adc,
                                [#(#adc_type),*],
                                #default_polling_interval,
                                #light_sleep_param,
                            )};
                });
                (config, processor_name)
            } else {
                (quote! {}, Vec::new())
            }
        }
        _ => (quote! {}, Vec::new()),
    }
}
