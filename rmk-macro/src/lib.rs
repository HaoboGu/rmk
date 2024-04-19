mod gpio_config;
mod keyboard;
mod keyboard_config;

use crate::{
    keyboard::parse_keyboard_mod,
    keyboard_config::{
        expand_keyboard_info, expand_light_config, expand_matrix_config, expand_vial_config,
        get_chip_model, ChipSeries,
    },
};
use proc_macro::TokenStream;
use quote::quote;
use rmk_config::{self, toml_config::KeyboardTomlConfig};
use std::fs;
use syn::parse_macro_input;

#[proc_macro_attribute]
pub fn rmk_keyboard(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item_mod = parse_macro_input!(item as syn::ItemMod);
    parse_keyboard_mod(attr, item_mod).into()
}

#[proc_macro_attribute]
pub fn rmk_main(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Read keyboard config file at project root
    let s = match fs::read_to_string("keyboard.toml") {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Read keyboard config file `keyboard.toml` error: {}", e);
            return syn::Error::new_spanned::<proc_macro2::TokenStream, String>(attr.into(), msg)
                .to_compile_error()
                .into();
        }
    };
    // Parse keyboard config file content to `KeyboardTomlConfig`
    let c: KeyboardTomlConfig = match toml::from_str(&s) {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("Parse `keyboard.toml` error: {}", e.message());
            return syn::Error::new_spanned::<proc_macro2::TokenStream, String>(attr.into(), msg)
                .to_compile_error()
                .into();
        }
    };

    // Generate code from toml config
    let chip = get_chip_model(c.keyboard.chip.clone());
    if chip == ChipSeries::Unsupported {
        return quote! {
            compile_error!("Unsupported chip series, please check `chip` field in `keyboard.toml`");
        }
        .into();
    }
    // Create keyboard info and vial struct
    let keyboard_info_static_var = expand_keyboard_info(
        c.keyboard.clone(),
        c.matrix.rows as usize,
        c.matrix.cols as usize,
        c.matrix.layers as usize,
    );
    let vial_static_var = expand_vial_config();
    // Create macros that initialize light config and matrix config
    let light_config_macro = expand_light_config(&chip, c.light);
    let matrix_config_macro = expand_matrix_config(&chip, c.matrix);

    // Original function body
    let f = parse_macro_input!(item as syn::ItemFn);

    // Prepend all generated contents before function
    quote! {
        #keyboard_info_static_var
        #vial_static_var
        #light_config_macro
        #matrix_config_macro

        #f
    }
    .into()
}
