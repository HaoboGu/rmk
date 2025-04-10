//! Initialize matrix initialization boilerplate of RMK
//!
use quote::quote;

use crate::{
    config::MatrixType,
    feature::is_feature_enabled,
    gpio_config::{
        convert_direct_pins_to_initializers, convert_input_pins_to_initializers,
        convert_output_pins_to_initializers, get_input_pin_type, get_output_pin_type,
    },
    keyboard_config::{BoardConfig, KeyboardConfig, UniBodyConfig},
    ChipModel, ChipSeries,
};

pub(crate) fn expand_matrix_config(
    keyboard_config: &KeyboardConfig,
    rmk_features: &Option<Vec<String>>,
) -> proc_macro2::TokenStream {
    let async_matrix = is_feature_enabled(rmk_features, "async_matrix");
    let mut matrix_config = proc_macro2::TokenStream::new();
    match &keyboard_config.board {
        BoardConfig::UniBody(UniBodyConfig { matrix, .. }) => match matrix.matrix_type {
            MatrixType::normal => {
                matrix_config.extend(expand_matrix_input_output_pins(
                    &keyboard_config.chip,
                    matrix.input_pins.clone().unwrap(),
                    matrix.output_pins.clone().unwrap(),
                    async_matrix,
                ));
            }
            MatrixType::direct_pin => {
                matrix_config.extend(expand_matrix_direct_pins(
                    &keyboard_config.chip,
                    matrix.direct_pins.clone().unwrap(),
                    async_matrix,
                    matrix.direct_pin_low_active,
                ));
                // `generic_arg_infer` is a nightly feature. Const arguments cannot yet be inferred with `_` in stable now.
                // So we need to declaring them in advance.
                let rows = keyboard_config.layout.rows as usize;
                let cols = keyboard_config.layout.cols as usize;
                let size =
                    keyboard_config.layout.rows as usize * keyboard_config.layout.cols as usize;
                let layers = keyboard_config.layout.layers as usize;
                let low_active = matrix.direct_pin_low_active;
                matrix_config.extend(quote! {
                    pub(crate) const ROW: usize = #rows;
                    pub(crate) const COL: usize = #cols;
                    pub(crate) const SIZE: usize = #size;
                    pub(crate) const LAYER_NUM: usize = #layers;
                    let low_active = #low_active;
                });
            }
        },
        BoardConfig::Split(split_config) => {
            // Matrix config for split central
            match split_config.central.matrix.matrix_type {
                MatrixType::normal => matrix_config.extend(expand_matrix_input_output_pins(
                    &keyboard_config.chip,
                    split_config.central.matrix.input_pins.clone().unwrap(),
                    split_config.central.matrix.output_pins.clone().unwrap(),
                    async_matrix,
                )),
                MatrixType::direct_pin => matrix_config.extend(expand_matrix_direct_pins(
                    &keyboard_config.chip,
                    split_config.central.matrix.direct_pins.clone().unwrap(),
                    async_matrix,
                    split_config.central.matrix.direct_pin_low_active,
                )),
            }
        }
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
    // Get input pin type
    let input_pin_type = get_input_pin_type(chip, async_matrix);
    let rows = direct_pins.len();
    let cols = if direct_pins.len() == 0 {
        0
    } else {
        direct_pins[0].len()
    };
    // Initialize input pins
    pin_initialization.extend(convert_direct_pins_to_initializers(
        &chip,
        direct_pins,
        async_matrix,
        low_active,
    ));
    // Generate a macro that does pin matrix config

    quote! {
        let direct_pins: [[Option<#input_pin_type>; #cols]; #rows] = {
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
    let input_pin_len = input_pins.len();
    let output_pin_len = output_pins.len();

    // Get pin types
    let input_pin_type = get_input_pin_type(chip, async_matrix);
    let output_pin_type = get_output_pin_type(chip);

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
        let (input_pins, output_pins): ([ #input_pin_type; #input_pin_len], [ #output_pin_type; #output_pin_len]) = {
            #pin_initialization
            (input_pins, output_pins)
        };
    }
}
