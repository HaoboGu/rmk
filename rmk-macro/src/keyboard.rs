use std::fs;

use crate::keyboard_config::{
    expand_keyboard_info, expand_light_config, expand_matrix_config, expand_vial_config,
    get_chip_model, ChipSeries,
};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::toml_config::KeyboardTomlConfig;
use syn::ItemMod;

pub(crate) fn parse_keyboard_mod(attr: proc_macro::TokenStream, item_mod: ItemMod) -> TokenStream2 {
    // Read keyboard config file at project root
    let s = match fs::read_to_string("keyboard.toml") {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Read keyboard config file `keyboard.toml` error: {}", e);
            return syn::Error::new_spanned::<TokenStream2, String>(attr.into(), msg)
                .to_compile_error()
                .into();
        }
    };
    // Parse keyboard config file content to `KeyboardTomlConfig`
    let toml_config: KeyboardTomlConfig = match toml::from_str(&s) {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("Parse `keyboard.toml` error: {}", e.message());
            return syn::Error::new_spanned::<TokenStream2, String>(attr.into(), msg)
                .to_compile_error()
                .into();
        }
    };

    // Generate code from toml config
    let chip = get_chip_model(toml_config.keyboard.chip.clone());
    if chip == ChipSeries::Unsupported {
        return quote! {
            compile_error!("Unsupported chip series, please check `chip` field in `keyboard.toml`");
        }
        .into();
    }
    // Create keyboard info and vial struct
    let keyboard_info_static_var = expand_keyboard_info(toml_config.keyboard);
    let vial_static_var = expand_vial_config();
    // Create macros that initialize light config and matrix config
    let light_config_macro = expand_light_config(&chip, toml_config.light);
    let matrix_config_macro = expand_matrix_config(&chip, toml_config.matrix);

    // TODO: 2. Generate main function

    // TODO: 3. Insert customization code

    quote! {
        #keyboard_info_static_var
        #vial_static_var
        #light_config_macro
        #matrix_config_macro

    }
}
