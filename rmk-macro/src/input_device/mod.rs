use adc::expand_adc_device;
use encoder::expand_encoder_device;
use pmw3610::expand_pmw3610_device;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use rmk_config::{BleConfig, BoardConfig, CommunicationConfig, InputDeviceConfig, KeyboardTomlConfig, UniBodyConfig};
use std::collections::HashMap;
use syn::ItemMod;

pub(crate) mod adc;
pub(crate) mod device;
pub(crate) mod encoder;
pub(crate) mod pmw3610;
pub(crate) mod processor;

/// Initializer struct for input devices
pub(crate) struct Initializer {
    pub(crate) initializer: TokenStream,
    pub(crate) var_name: Ident,
}

/// Expands the input device configuration.
/// Returns a tuple containing: (device_and_processors_initialization, devices, processors)
pub(crate) fn expand_input_device_config(
    keyboard_config: &KeyboardTomlConfig,
    item_mod: &ItemMod,
) -> (TokenStream, Vec<TokenStream>, Vec<TokenStream>) {
    let mut initialization = TokenStream::new();
    let mut devices = Vec::new();
    let mut built_in_processors = Vec::new();

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
        BoardConfig::Split(split_config) => {
            // For split central, read battery config from split.central instead of [ble]
            // This provides better consistency with peripheral configuration
            let central_ble_config = if split_config.central.battery_adc_pin.is_some() {
                // Validate: warn if both [ble] and [split.central] have different battery configs
                if let Some(ref ble_cfg) = ble_config
                    && ble_cfg.battery_adc_pin.is_some()
                {
                    let ble_matches = ble_cfg.battery_adc_pin == split_config.central.battery_adc_pin
                        && ble_cfg.adc_divider_measured == split_config.central.adc_divider_measured
                        && ble_cfg.adc_divider_total == split_config.central.adc_divider_total;

                    if !ble_matches {
                        eprintln!(
                            "warning: Battery configuration found in both [ble] and [split.central] sections with different values"
                        );
                        eprintln!(
                            "help: [split.central] configuration will be used. Remove [ble] battery config to avoid confusion."
                        );
                    }
                }

                // Central has its own battery config in [split.central]
                Some(BleConfig {
                    enabled: ble_config.as_ref().map(|c| c.enabled).unwrap_or(false),
                    battery_adc_pin: split_config.central.battery_adc_pin.clone(),
                    adc_divider_measured: split_config.central.adc_divider_measured,
                    adc_divider_total: split_config.central.adc_divider_total,
                    ..Default::default()
                })
            } else {
                // Fall back to [ble] section for backward compatibility
                ble_config
            };

            expand_adc_device(
                split_config
                    .central
                    .input_device
                    .clone()
                    .unwrap_or(InputDeviceConfig::default())
                    .joystick
                    .unwrap_or(Vec::new()),
                central_ble_config,
                chip.series.clone(),
            )
        }
    };

    for initializer in adc_initializers {
        initialization.extend(initializer.initializer);
        let device_name = initializer.var_name;
        devices.push(quote! { #device_name });
    }

    for initializer in adc_processors {
        initialization.extend(initializer.initializer);
        let processor_name = initializer.var_name.to_string();
        let processor_ident = initializer.var_name;
        built_in_processors.push((processor_name, quote! { #processor_ident }));
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
        let processor_name = initializer.var_name.to_string();
        let processor_ident = initializer.var_name;
        built_in_processors.push((processor_name, quote! { #processor_ident }));
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
        let processor_name = initializer.var_name.to_string();
        let processor_ident = initializer.var_name;
        built_in_processors.push((processor_name, quote! { #processor_ident }));
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
                let processor_name = initializer.var_name.to_string();
                let processor_ident = initializer.var_name;
                built_in_processors.push((processor_name, quote! { #processor_ident }));
            }
        }
    }

    // Expand custom devices from #[device] attributes
    let custom_device_infos = device::expand_custom_devices(item_mod);
    for device_info in custom_device_infos {
        initialization.extend(device_info.init);
        let device_name = device_info.name;
        devices.push(quote! { #device_name });
    }

    // Expand custom processors from #[processor] attributes
    let custom_processor_infos = processor::expand_custom_processors(item_mod);

    // Add custom processor initialization
    for processor_info in &custom_processor_infos {
        initialization.extend(processor_info.init.clone());
    }

    // Order processors according to processor_chain config
    let input_device_config = match &board {
        BoardConfig::UniBody(UniBodyConfig { input_device, .. }) => input_device.clone(),
        BoardConfig::Split(split_config) => split_config.central.input_device.clone().unwrap_or_default(),
    };

    let processors = order_processors(
        built_in_processors,
        custom_processor_infos,
        input_device_config.processor_chain,
    );

    (initialization, devices, processors)
}

/// Orders processors according to processor_chain configuration
///
/// If processor_chain is specified, processors are ordered according to the chain.
/// Otherwise, default order is: built-in processors (in TOML order) then custom processors (in code order)
fn order_processors(
    built_in: Vec<(String, TokenStream)>,
    custom: Vec<processor::ProcessorInfo>,
    processor_chain: Option<Vec<String>>,
) -> Vec<TokenStream> {
    // Build a map of all processors (name -> TokenStream)
    let mut processor_map: HashMap<String, TokenStream> = HashMap::new();

    // Keep track of order for default behavior
    let mut built_in_order = Vec::new();
    let mut custom_order = Vec::new();

    // Add built-in processors to map
    for (name, token_stream) in built_in {
        built_in_order.push(name.clone());
        processor_map.insert(name, token_stream);
    }

    // Add custom processors to map (initialization was already added)
    for processor_info in custom {
        let name = processor_info.name.to_string();
        let processor_ident = processor_info.name;
        custom_order.push(name.clone());
        processor_map.insert(name, quote! { #processor_ident });
    }

    // If processor_chain is specified, use that order
    if let Some(chain) = processor_chain {
        // Validate that all processors in the chain exist
        let available_processors: Vec<String> = processor_map.keys().cloned().collect();
        for processor_name in &chain {
            if !processor_map.contains_key(processor_name) {
                panic!(
                    "Unknown processor '{}' in processor_chain.\n\n\
                     Available processors:\n  {}\n\n\
                     Built-in processor naming rules:\n\
                     - battery_processor: Battery level monitoring (if BLE battery ADC configured)\n\
                     - joystick_processor_{{name}}: Joystick (name from [[input_device.joystick]])\n\
                     - {{name}}_processor: PMW3610 sensor (name from [[input_device.pmw3610]], defaults to pmw3610_{{idx}}_processor)\n\n\
                     Custom processors: Define with #[processor] attribute in your code",
                    processor_name,
                    available_processors.join("\n  ")
                );
            }
        }

        chain
            .iter()
            .filter_map(|name| processor_map.get(name).cloned())
            .collect()
    } else {
        // Default order: built-in processors first, then custom processors
        let mut result = Vec::new();

        // Add built-in processors in order
        for name in built_in_order {
            if let Some(ts) = processor_map.get(&name) {
                result.push(ts.clone());
            }
        }

        // Add custom processors in order
        for name in custom_order {
            if let Some(ts) = processor_map.get(&name) {
                result.push(ts.clone());
            }
        }

        result
    }
}
