use crate::{
    config::InputDeviceConfig,
    keyboard_config::{BoardConfig, KeyboardConfig},
    keyboard_config::{CommunicationConfig, SingleConfig},
};
use adc::expand_adc_device;
use proc_macro2::TokenStream;
use quote::quote;

mod adc;

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
        BoardConfig::Single(SingleConfig { input_device, .. }) => expand_adc_device(
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

    // TODO
    // config.extend(expand_encoder_config())

    (config, devices, processors)
}
