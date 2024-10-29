//! Initialize matrix initialization boilerplate of RMK
//!
use quote::quote;

use crate::{
    gpio_config::{convert_input_pins_to_initializers, convert_output_pins_to_initializers, convert_direct_pins_to_initializers},
    keyboard_config::{BoardConfig, KeyboardConfig},
    ChipModel, ChipSeries,
};

pub(crate) fn expand_matrix_config(
    keyboard_config: &KeyboardConfig,
    async_matrix: bool,
) -> proc_macro2::TokenStream {
    let mut matrix_config = proc_macro2::TokenStream::new();
    match &keyboard_config.board {
        BoardConfig::Normal(matrix) => {
            matrix_config.extend(expand_matrix_input_output_pins(
                &keyboard_config.chip,
                matrix.input_pins.clone().unwrap(),
                matrix.output_pins.clone().unwrap(),
                async_matrix,
            ));
        }
        BoardConfig::DirectPin(matrix) => {
            matrix_config.extend(expand_matrix_direct_pins(
                &keyboard_config.chip,
                matrix.direct_pins.clone().unwrap(),
                async_matrix,
                matrix.direct_pin_low_active
            ));
            // `generic_arg_infer` is a nightly feature. Const arguments cannot yet be inferred with `_` in stable now.
            // So we need to declaring them in advance.
            let rows = keyboard_config.layout.rows as usize;
            let cols = keyboard_config.layout.cols as usize;
            let layers = keyboard_config.layout.layers as usize;
            matrix_config.extend(quote! {
                pub(crate) const ROW: usize = #rows;
                pub(crate) const COL: usize = #cols;
                pub(crate) const LAYER_NUM: usize = #layers;
            });
        }
        _ => (),
    };
    matrix_config
}

pub(crate) fn expand_matrix_direct_pins(
    chip: &ChipModel,
    direct_pins: Vec<Vec<String>>,
    async_matrix: bool,
    low_active: bool,
) -> proc_macro2::TokenStream {
    let mut pin_initialization = proc_macro2::TokenStream::new();
    // Initialize input pins
    pin_initialization.extend(convert_direct_pins_to_initializers(
        &chip,
        direct_pins,
        async_matrix,
        low_active,
    ));
    // Generate a macro that does pin matrix config
    quote! {
        let direct_pins = {
            #pin_initialization
            direct_pins
        };
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
            (input_pins, output_pins)
        };
    }
}
