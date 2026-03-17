use quote::quote;
use rmk_config::KeyboardTomlConfig;
use rmk_config::resolved::{Hardware, Identity, Layout};

pub(crate) fn read_keyboard_toml_config() -> KeyboardTomlConfig {
    // Get the path of the keyboard config file from the environment variable
    let config_toml_path = std::env::var("KEYBOARD_TOML_PATH")
        .expect("[ERROR]: KEYBOARD_TOML_PATH should be set in `.cargo/config.toml`");

    KeyboardTomlConfig::new_from_toml_path(&config_toml_path)
}

pub(crate) fn expand_keyboard_info(
    identity: &Identity,
    layout: &Layout,
    hardware: &Hardware,
) -> proc_macro2::TokenStream {
    let pid = identity.product_id;
    let vid = identity.vendor_id;
    let product_name = identity.product_name.clone();
    let manufacturer = identity.manufacturer.clone();
    let serial_number = identity.serial_number.clone();

    let num_col = layout.cols as usize;
    let num_row = layout.rows as usize;
    let num_layer = layout.layers as usize;
    let num_encoder = &layout.encoder_counts;
    let total_num_encoder: usize = num_encoder.iter().sum();
    let _ = hardware; // hardware available for future use
    quote! {
        pub(crate) const COL: usize = #num_col;
        pub(crate) const ROW: usize = #num_row;
        pub(crate) const NUM_LAYER: usize = #num_layer;
        pub(crate) const NUM_ENCODER: usize = #total_num_encoder;
        const KEYBOARD_DEVICE_CONFIG: ::rmk::config::DeviceConfig = ::rmk::config::DeviceConfig {
            vid: #vid,
            pid: #pid,
            manufacturer: #manufacturer,
            product_name: #product_name,
            serial_number: #serial_number,
        };
    }
}

pub(crate) fn expand_vial_config(hardware: &Hardware) -> proc_macro2::TokenStream {
    if !hardware.host.vial_enabled {
        return quote! {};
    }
    let unlock_keys = if !hardware.host.unlock_keys.is_empty() {
        let keys_expr = hardware
            .host
            .unlock_keys
            .iter()
            .map(|key| {
                let row = key[0];
                let col = key[1];
                quote! { (#row, #col) }
            })
            .collect::<Vec<_>>();
        quote! { &[#(#keys_expr), *] }
    } else {
        quote! { &[] }
    };
    quote! {
        include!(concat!(env!("OUT_DIR"), "/config_generated.rs"));
        const VIAL_CONFIG: ::rmk::config::VialConfig = ::rmk::config::VialConfig {
            vial_keyboard_id: &VIAL_KEYBOARD_ID,
            vial_keyboard_def: &VIAL_KEYBOARD_DEF,
            unlock_keys: #unlock_keys
        };
    }
}
