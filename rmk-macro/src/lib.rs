use proc_macro::TokenStream;
use quote::quote;
use rmk_config::{self, KeyboardInfo, KeyboardTomlConfig};
use std::fs;
use syn::{parse_macro_input, Stmt};

enum ChipSeries {
    Stm32,
    Nrf52,
    Rp2040,
    Esp32,
    Unsupported,
}

fn get_chip_model(chip: String) -> ChipSeries {
    if chip.to_lowercase().starts_with("stm32") {
        return ChipSeries::Stm32;
    } else if chip.to_lowercase().starts_with("nrf52") {
        return ChipSeries::Nrf52;
    } else if chip.to_lowercase().starts_with("rp2040") {
        return ChipSeries::Rp2040;
    } else if chip.to_lowercase().starts_with("esp32") {
        return ChipSeries::Esp32;
    } else {
        return ChipSeries::Unsupported;
    }
}

fn build_keyboard_info(keyboard_info: KeyboardInfo) -> proc_macro2::TokenStream {
    let pid = keyboard_info.product_id;
    let vid = keyboard_info.vendor_id;
    let product_name: proc_macro2::TokenStream = match keyboard_info.product_name {
        None => quote! {None},
        Some(s) => quote! {Some(#s)},
    };
    let manufacturer: proc_macro2::TokenStream = match keyboard_info.manufacturer {
        None => quote! {None},
        Some(s) => quote! {Some(#s)},
    };
    let serial_number: proc_macro2::TokenStream = match keyboard_info.serial_number {
        None => quote! {None},
        Some(s) => quote! {Some(#s)},
    };
    quote! {
        static keyboard_usb_config: ::rmk_config::rmk_keyboard_config::KeyboardUsbConfig = ::rmk_config::rmk_keyboard_config::KeyboardUsbConfig {
            vid: #vid,
            pid: #pid,
            manufacturer: #manufacturer,
            product_name: #product_name,
            serial_number: #serial_number,
        };
    }
}

#[proc_macro_attribute]
pub fn rmk_main(attr: TokenStream, item: TokenStream) -> TokenStream {
    let s = match fs::read_to_string("keyboard.toml") {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Read keyboard config file `keyboard.toml` error: {}", e);
            return syn::Error::new_spanned::<proc_macro2::TokenStream, String>(attr.into(), msg)
                .to_compile_error()
                .into();
        }
    };
    let c: KeyboardTomlConfig = match toml::from_str(&s) {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("Parse `keyboard.toml` error: {}", e.message());
            return syn::Error::new_spanned::<proc_macro2::TokenStream, String>(attr.into(), msg)
                .to_compile_error()
                .into();
        }
    };

    let chip = get_chip_model(c.keyboard.chip.clone());
    let q = build_keyboard_info(c.keyboard);
    eprintln!("q {:#?}", q);
    let mut f = parse_macro_input!(item as syn::ItemFn);
    let mut f_body = f.block;
    let mut stmts = f_body.stmts;
    let stmt: Stmt = syn::parse_str("println!(\"info: {:?} \", keyboard_usb_config);").unwrap();
    stmts.push(stmt);
    f_body.stmts = stmts;
    f.block = f_body;
    quote! {
        #q
        #f
    }
    .into()
}
