use crate::{
    config::InputDeviceConfig,
    keyboard_config::{BoardConfig, KeyboardConfig},
    keyboard_config::{CommunicationConfig, UniBodyConfig},
};
use adc::expand_adc_device;
use encoder::expand_encoder_device;
use proc_macro2::TokenStream;
use quote::quote;

mod adc;
mod encoder;

pub(crate) fn expand_input_device_config(
    keyboard_config: &KeyboardConfig,
) -> (TokenStream, Vec<TokenStream>, Vec<TokenStream>) {
    let mut config = TokenStream::new();
    let mut devices = Vec::new();
    let mut processors = Vec::new();

    // generate ADC configuration
    let ble_config = match &keyboard_config.communication {
        CommunicationConfig::Ble(ble_config) | CommunicationConfig::Both(_, ble_config) => {
            Some(ble_config.clone())
        }
        _ => None,
    };
    let (adc_config, adc_processors) = match &keyboard_config.board {
        BoardConfig::UniBody(UniBodyConfig { input_device, .. }) => expand_adc_device(
            input_device.clone().joystick.unwrap_or(Vec::new()),
            ble_config,
            keyboard_config.chip.series,
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
            keyboard_config.chip.series,
        ),
    };
    config.extend(adc_config);
    if !adc_processors.is_empty() {
        devices.push(quote! {adc_device});
    }
    processors.extend(adc_processors);

    // generate encoder configuration
    let (encoder_config, encoder_processors, encoder_names) = match &keyboard_config.board {
        BoardConfig::UniBody(UniBodyConfig { input_device, .. }) => expand_encoder_device(
            input_device.clone().encoder.unwrap_or(Vec::new()),
            &keyboard_config.chip,
        ),
        BoardConfig::Split(split_config) => expand_encoder_device(
            split_config
                .central
                .input_device
                .clone()
                .unwrap_or(InputDeviceConfig::default())
                .encoder
                .unwrap_or(Vec::new()),
            &keyboard_config.chip,
        ),
    };
    config.extend(encoder_config);
    if !encoder_processors.is_empty() {
        for encoder_name in encoder_names {
            devices.push(quote! {#encoder_name});
        }
    }
    processors.extend(encoder_processors);

    (config, devices, processors)
}
