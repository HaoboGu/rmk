//! Initialize matrix initialization boilerplate of RMK
//!
use quote::quote;

use crate::{
    gpio_config::{convert_input_pins_to_initializers, convert_output_pins_to_initializers},
    keyboard_config::{BoardConfig, KeyboardConfig},
    ChipModel, ChipSeries,
};

pub(crate) fn expand_matrix_config(
    keyboard_config: &KeyboardConfig,
    async_matrix: bool,
) -> proc_macro2::TokenStream {
    if let BoardConfig::Normal(matrix) = &keyboard_config.board {
        expand_matrix_input_output_pins(
            &keyboard_config.chip,
            matrix.input_pins.clone(),
            matrix.output_pins.clone(),
            async_matrix,
        )
    } else {
        quote! {}
    }
}

pub(crate) fn expand_matrix_input_output_pins(
    chip: &ChipModel,
    input_pins: Vec<String>,
    output_pins: Vec<String>,
    async_matrix: bool,
) -> proc_macro2::TokenStream {
    let mut pin_initialization = proc_macro2::TokenStream::new();
    // Extra import when using `ExtiInput`
    let extra_import = if chip.series == ChipSeries::Stm32 && async_matrix {
        quote! {
            use ::embassy_stm32::exti::Channel;
        }
    } else {
        quote! {}
    };
    // Initialize input pins
    pin_initialization.extend(convert_input_pins_to_initializers(
        &chip,
        input_pins,
        async_matrix,
    ));
    // Initialize output pins
    pin_initialization.extend(convert_output_pins_to_initializers(&chip, output_pins));

    // Generate a macro that does pin matrix config
    quote! {
        #extra_import
        let (input_pins, output_pins) = {
            #pin_initialization
            (output_pins, input_pins)
        };
    }
}
