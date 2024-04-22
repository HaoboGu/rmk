//! Initialize matrix initialization boilerplate of RMK
//!
use quote::quote;
use rmk_config::toml_config::MatrixConfig;

use crate::{
    gpio_config::{convert_input_pins_to_initializers, convert_output_pins_to_initializers},
    ChipModel,
};

pub(crate) fn expand_matrix_config(
    chip: &ChipModel,
    matrix_config: MatrixConfig,
) -> proc_macro2::TokenStream {
    let mut pin_initialization = proc_macro2::TokenStream::new();
    // Initialize input pins
    pin_initialization.extend(convert_input_pins_to_initializers(
        &chip,
        matrix_config.input_pins,
    ));
    // Initialize output pins
    pin_initialization.extend(convert_output_pins_to_initializers(
        &chip,
        matrix_config.output_pins,
    ));

    // Generate a macro that does pin matrix config
    quote! {
        {
            #pin_initialization
            (output_pins, input_pins)
        }
    }
}
