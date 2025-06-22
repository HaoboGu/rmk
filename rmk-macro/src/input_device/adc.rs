use quote::{format_ident, quote};
use rmk_config::{BleConfig, ChipSeries, JoystickConfig};

use crate::input_device::Initializer;

/// Expand the ADC device configuration.
/// Returns (device initializers, processor initializers)
pub(crate) fn expand_adc_device(
    joystick_config: Vec<JoystickConfig>,
    ble_config: Option<BleConfig>,
    chip_model: ChipSeries,
) -> (Vec<Initializer>, Vec<Initializer>) {
    match chip_model {
        ChipSeries::Nrf52 => {
            let mut channel_cfg = vec![];
            let mut adc_type = vec![];
            let mut default_polling_interval = 30000u16; // default 30s
            let mut light_sleep: Option<u16> = None;
            // TODO: deep sleep

            let mut devices = vec![];
            let mut processors = vec![];

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

                        let (adc_divider_measured, adc_divider_total) = if adc_pin == "vddh" {
                            (1, 5)
                        } else {
                            (
                                ble.adc_divider_measured.unwrap_or(1),
                                ble.adc_divider_total.unwrap_or(1),
                            )
                        };
                        let bat_ident = format_ident!("battery_processor");
                        let battery_processor = Initializer {
                            initializer: quote! {
                                let mut #bat_ident = ::rmk::input_device::battery::BatteryProcessor::new(#adc_divider_measured, #adc_divider_total, &keymap);
                            },
                            var_name: bat_ident,
                        };
                        processors.push(battery_processor);
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
                let JoystickConfig {
                    transform,
                    bias,
                    resolution,
                    ..
                } = joystick;
                let joystick_processor = Initializer {
                    initializer: quote! {
                        let mut #joy_ident = rmk::input_device::joystick::JoystickProcessor::new([#([#(#transform),*]),*], [#(#bias),*], #resolution, &keymap);
                    },
                    var_name: joy_ident,
                };
                processors.push(joystick_processor);
            }

            if !processors.is_empty() {
                let light_sleep_option = if let Some(light_sleep_interval) = light_sleep {
                    quote! {Some(Duration::from_millis(#light_sleep_interval as u64))}
                } else {
                    quote! {None}
                };
                let adc_device = Initializer {
                    initializer: quote! {
                        let mut adc_device = {
                        use embassy_time::Duration;
                        use embassy_nrf::saadc::{self, Input as _};
                        ::embassy_nrf::bind_interrupts!(struct SaadcIrqs {
                            SAADC => ::embassy_nrf::saadc::InterruptHandler;
                        });
                        let saadc_config = saadc::Config::default();
                        embassy_nrf::interrupt::SAADC.set_priority(embassy_nrf::interrupt::Priority::P3);

                        let adc = saadc::Saadc::new(p.SAADC, SaadcIrqs, saadc_config, [#(#channel_cfg),*]);
                        adc.calibrate().await;

                        rmk::input_device::adc::NrfAdc::new(
                                adc,
                                [#(#adc_type),*],
                                Duration::from_millis(#default_polling_interval as u64),
                                #light_sleep_option,
                            )
                        };
                    },
                    var_name: format_ident!("adc_device"),
                };
                devices.push(adc_device);
                (devices, processors)
            } else {
                (Vec::new(), Vec::new())
            }
        }
        _ => (Vec::new(), Vec::new()),
    }
}
