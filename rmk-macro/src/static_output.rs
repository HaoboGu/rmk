//! Initialize static output pins according to config
//!
use proc_macro2::TokenStream;
use rmk_config::{ChipModel, KeyboardTomlConfig, StaticOutput};

use crate::gpio_config::convert_gpio_str_to_persisted_output_pin;

pub fn expand_static_output_config(keyboard_config: &KeyboardTomlConfig) -> TokenStream {
    let chip = keyboard_config.get_chip_model().unwrap();
    let static_outputs = keyboard_config.get_static_output_config().unwrap();
    expand_static_output_initialization(static_outputs, &chip)
}

pub fn expand_static_output_initialization(static_outputs: Vec<StaticOutput>, chip: &ChipModel) -> TokenStream {
    static_outputs
        .into_iter()
        .map(|so| convert_gpio_str_to_persisted_output_pin(&chip, so.pin, so.level_high))
        .collect()
}
