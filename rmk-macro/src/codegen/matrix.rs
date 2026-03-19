//! Initialize matrix initialization boilerplate of RMK
//!
use quote::quote;
use rmk_config::resolved::Hardware;
use rmk_config::resolved::hardware::{
    BoardConfig, ChipModel, ChipSeries, MatrixType, UniBodyConfig,
};

use super::chip::gpio::{
    convert_direct_pins_to_initializers, convert_input_pins_to_initializers,
    convert_output_pins_to_initializers, get_input_pin_type, get_output_pin_type,
};
use super::feature::is_feature_enabled;

pub(crate) fn expand_matrix_config(
    hardware: &Hardware,
    rmk_features: &Option<Vec<String>>,
) -> proc_macro2::TokenStream {
    let async_matrix = is_feature_enabled(rmk_features, "async_matrix");
    let mut matrix_config = proc_macro2::TokenStream::new();
    match &hardware.board {
        BoardConfig::UniBody(UniBodyConfig { matrix, .. }) => match matrix.matrix_type {
            MatrixType::Normal => {
                matrix_config.extend(expand_matrix_input_output_pins(
                    &hardware.chip,
                    matrix.row_pins.clone().unwrap(),
                    matrix.col_pins.clone().unwrap(),
                    matrix.row2col,
                    async_matrix,
                ));
            }
            MatrixType::DirectPin => {
                matrix_config.extend(expand_matrix_direct_pins(
                    &hardware.chip,
                    matrix.direct_pins.clone().unwrap(),
                    async_matrix,
                    matrix.direct_pin_low_active,
                ));
                // `generic_arg_infer` is a nightly feature. Const arguments cannot yet be inferred with `_` in stable now.
                // So we need to declare them in advance.
                let direct_pins = matrix.direct_pins.as_ref().unwrap();
                let rows = direct_pins.len();
                let cols = direct_pins.first().map_or(0, |row| row.len());
                let size = rows * cols;
                let low_active = matrix.direct_pin_low_active;
                matrix_config.extend(quote! {
                    pub(crate) const SIZE: usize = #size;
                    let low_active = #low_active;
                });
            }
        },
        BoardConfig::Split(split_config) => {
            // Matrix config for split central
            match split_config.central.matrix.matrix_type {
                MatrixType::Normal => matrix_config.extend(expand_matrix_input_output_pins(
                    &hardware.chip,
                    split_config.central.matrix.row_pins.clone().unwrap(),
                    split_config.central.matrix.col_pins.clone().unwrap(),
                    split_config.central.matrix.row2col,
                    async_matrix,
                )),
                MatrixType::DirectPin => matrix_config.extend(expand_matrix_direct_pins(
                    &hardware.chip,
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
    let cols = direct_pins.first().map_or(0, |row| row.len());
    // Initialize input pins
    pin_initialization.extend(convert_direct_pins_to_initializers(
        chip,
        direct_pins,
        async_matrix,
        low_active,
    ));

    quote! {
        let direct_pins: [[Option<#input_pin_type>; #cols]; #rows] = {
            #pin_initialization
            direct_pins
        };
    }
}

pub(crate) fn expand_matrix_input_output_pins(
    chip: &ChipModel,
    row_pins: Vec<String>,
    col_pins: Vec<String>,
    row2col: bool,
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
    let (input_pin_len, output_pin_len) = if row2col {
        (col_pins.len(), row_pins.len())
    } else {
        (row_pins.len(), col_pins.len())
    };

    let (input_pins, output_pins) = if row2col {
        (col_pins, row_pins)
    } else {
        (row_pins, col_pins)
    };

    // Get pin types
    let input_pin_type = get_input_pin_type(chip, async_matrix);
    let output_pin_type = get_output_pin_type(chip);

    // Initialize input pins
    pin_initialization.extend(convert_input_pins_to_initializers(
        chip,
        input_pins,
        async_matrix,
    ));
    // Initialize output pins
    pin_initialization.extend(convert_output_pins_to_initializers(chip, output_pins));
    let pin_names = if row2col {
        quote! { (col_pins, row_pins) }
    } else {
        quote! { (row_pins, col_pins) }
    };
    // Generate a macro that does pin matrix config
    quote! {
        #extra_import
        let #pin_names: ([ #input_pin_type; #input_pin_len], [ #output_pin_type; #output_pin_len]) = {
            #pin_initialization
            (input_pins, output_pins)
        };
    }
}
