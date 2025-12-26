use adc::expand_adc_device;
use encoder::expand_encoder_device;
use iqs5xx::expand_iqs5xx_device;
use pmw3610::expand_pmw3610_device;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use rmk_config::{BoardConfig, CommunicationConfig, InputDeviceConfig, KeyboardTomlConfig, UniBodyConfig};

pub(crate) mod adc;
pub(crate) mod encoder;
pub(crate) mod iqs5xx;
pub(crate) mod pmw3610;

/// Initializer struct for input devices
pub(crate) struct Initializer {
    pub(crate) initializer: TokenStream,
    pub(crate) var_name: Ident,
}

/// Expands the input device configuration.
/// Returns a tuple containing: (device_and_processors_initialization, devices, processors)
pub(crate) fn expand_input_device_config(
    keyboard_config: &KeyboardTomlConfig,
) -> (TokenStream, Vec<TokenStream>, Vec<TokenStream>) {
    let mut initialization = TokenStream::new();
    let mut devices = Vec::new();
    let mut processors = Vec::new();

    // generate ADC configuration
    let communication = keyboard_config.get_communication_config().unwrap();
    let ble_config = match &communication {
        CommunicationConfig::Ble(ble_config) | CommunicationConfig::Both(_, ble_config) => Some(ble_config.clone()),
        _ => None,
    };
    let board = keyboard_config.get_board_config().unwrap();
    let chip = keyboard_config.get_chip_model().unwrap();
    let (adc_initializers, adc_processors) = match &board {
        BoardConfig::UniBody(UniBodyConfig { input_device, .. }) => expand_adc_device(
            input_device.clone().joystick.unwrap_or(Vec::new()),
            ble_config,
            chip.series.clone(),
        ),
        BoardConfig::Split(split_config) => expand_adc_device(
            split_config
                .central
                .input_device
                .clone()
                .unwrap_or(InputDeviceConfig::default())
                .joystick
                .unwrap_or(Vec::new()),
            ble_config,
            chip.series.clone(),
        ),
    };

    for initializer in adc_initializers {
        initialization.extend(initializer.initializer);
        let device_name = initializer.var_name;
        devices.push(quote! { #device_name });
    }

    for initializer in adc_processors {
        initialization.extend(initializer.initializer);
        let processor_name = initializer.var_name;
        processors.push(quote! { #processor_name });
    }

    // generate encoder configuration
    let (device_initializer, processor_initializer) = match &board {
        BoardConfig::UniBody(UniBodyConfig { input_device, .. }) => {
            expand_encoder_device(0, input_device.clone().encoder.unwrap_or(Vec::new()), &chip)
        }
        BoardConfig::Split(split_config) => expand_encoder_device(
            0,
            split_config
                .central
                .input_device
                .clone()
                .unwrap_or(InputDeviceConfig::default())
                .encoder
                .unwrap_or(Vec::new()),
            &chip,
        ),
    };
    for initializer in device_initializer {
        initialization.extend(initializer.initializer);
        let device_name = initializer.var_name;
        devices.push(quote! { #device_name });
    }

    for initializer in processor_initializer {
        initialization.extend(initializer.initializer);
        let processor_name = initializer.var_name;
        processors.push(quote! { #processor_name });
    }

    // generate PMW3610 configuration
    let (pmw3610_device_initializers, pmw3610_processor_initializers) = match &board {
        BoardConfig::UniBody(UniBodyConfig { input_device, .. }) => {
            expand_pmw3610_device(input_device.clone().pmw3610.unwrap_or(Vec::new()), &chip)
        }
        BoardConfig::Split(split_config) => expand_pmw3610_device(
            split_config
                .central
                .input_device
                .clone()
                .unwrap_or(InputDeviceConfig::default())
                .pmw3610
                .unwrap_or(Vec::new()),
            &chip,
        ),
    };

    for initializer in pmw3610_device_initializers {
        initialization.extend(initializer.initializer);
        let device_name = initializer.var_name;
        devices.push(quote! { #device_name });
    }

    for initializer in pmw3610_processor_initializers {
        initialization.extend(initializer.initializer);
        let processor_name = initializer.var_name;
        processors.push(quote! { #processor_name });
    }

    // generate IQS5xx configuration
    let (iqs5xx_device_initializers, iqs5xx_processor_initializers) = match &board {
        BoardConfig::UniBody(UniBodyConfig { input_device, .. }) => {
            expand_iqs5xx_device(input_device.clone().iqs5xx.unwrap_or(Vec::new()), &chip)
        }
        BoardConfig::Split(split_config) => expand_iqs5xx_device(
            split_config
                .central
                .input_device
                .clone()
                .unwrap_or(InputDeviceConfig::default())
                .iqs5xx
                .unwrap_or(Vec::new()),
            &chip,
        ),
    };

    for initializer in iqs5xx_device_initializers {
        initialization.extend(initializer.initializer);
        let device_name = initializer.var_name;
        devices.push(quote! { #device_name });
    }

    for initializer in iqs5xx_processor_initializers {
        initialization.extend(initializer.initializer);
        let processor_name = initializer.var_name;
        processors.push(quote! { #processor_name });
    }

    // For split keyboards, also generate processors for PMW3610 devices on peripherals
    // The devices run on peripherals, but processors need to run on central to handle the events
    if let BoardConfig::Split(split_config) = &board {
        for peripheral in &split_config.peripheral {
            let peripheral_pmw3610_config = peripheral
                .input_device
                .clone()
                .unwrap_or(InputDeviceConfig::default())
                .pmw3610
                .unwrap_or(Vec::new());

            // Only generate processors (not devices) for peripheral PMW3610
            let (_, peripheral_pmw3610_processors) = expand_pmw3610_device(peripheral_pmw3610_config, &chip);

            for initializer in peripheral_pmw3610_processors {
                initialization.extend(initializer.initializer);
                let processor_name = initializer.var_name;
                processors.push(quote! { #processor_name });
            }
        }

        for peripheral in &split_config.peripheral {
            let peripheral_iqs5xx_config = peripheral
                .input_device
                .clone()
                .unwrap_or(InputDeviceConfig::default())
                .iqs5xx
                .unwrap_or(Vec::new());

            // Only generate processors (not devices) for peripheral IQS5xx
            let (_, peripheral_iqs5xx_processors) = expand_iqs5xx_device(peripheral_iqs5xx_config, &chip);

            for initializer in peripheral_iqs5xx_processors {
                initialization.extend(initializer.initializer);
                let processor_name = initializer.var_name;
                processors.push(quote! { #processor_name });
            }
        }
    }

    (initialization, devices, processors)
}
