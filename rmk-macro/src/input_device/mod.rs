use adc::expand_adc_device;
use encoder::expand_encoder_device;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use rmk_config::{BoardConfig, CommunicationConfig, InputDeviceConfig, KeyboardTomlConfig, UniBodyConfig};

pub(crate) mod adc;
pub(crate) mod encoder;

/// Initializer struct for input devices
pub(crate) struct Initializer {
    pub(crate) initializer: TokenStream,
    pub(crate) var_name: Ident,
}

pub(crate) fn expand_input_device_config(
    keyboard_config: &KeyboardTomlConfig,
) -> (TokenStream, Vec<TokenStream>, Vec<TokenStream>) {
    let mut config = TokenStream::new();
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
    let (adc_config, adc_processors) = match &board {
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
    config.extend(adc_config);
    if !adc_processors.is_empty() {
        devices.push(quote! {adc_device});
    }
    processors.extend(adc_processors);

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
        config.extend(initializer.initializer);
        let device_name = initializer.var_name;
        devices.push(quote! { #device_name });
    }

    for initializer in processor_initializer {
        config.extend(initializer.initializer);
        let processor_name = initializer.var_name;
        processors.push(quote! { #processor_name });
    }

    (config, devices, processors)
}
