use quote::quote;
use rmk_config::KeyboardTomlConfig;

pub(crate) fn read_keyboard_toml_config() -> KeyboardTomlConfig {
    // Get the path of the keyboard config file from the environment variable
    let config_toml_path =
        std::env::var("KEYBOARD_TOML_PATH").expect("[ERROR]: KEYBOARD_TOML_PATH should be set in `.cargo/config.toml`");

    KeyboardTomlConfig::new_from_toml_str(&config_toml_path)
}

pub(crate) fn expand_keyboard_info(keyboard_config: &KeyboardTomlConfig) -> proc_macro2::TokenStream {
    let basic = keyboard_config.get_basic_info();
    let layout = keyboard_config.get_layout_config().unwrap();
    let board = keyboard_config.get_board_config().unwrap();
    let pid = basic.product_id;
    let vid = basic.vendor_id;
    let product_name = basic.product_name.clone();
    let manufacturer = basic.manufacturer.clone();
    let serial_number = basic.serial_number.clone();

    let num_col = layout.cols as usize;
    let num_row = layout.rows as usize;
    let num_layer = layout.layers as usize;
    let num_encoder = board.get_num_encoder();
    let total_num_encoder = num_encoder.iter().sum::<usize>();
    quote! {
        pub(crate) const COL: usize = #num_col;
        pub(crate) const ROW: usize = #num_row;
        pub(crate) const NUM_LAYER: usize = #num_layer;
        pub(crate) const NUM_ENCODER: usize = #total_num_encoder;
        const KEYBOARD_USB_CONFIG: ::rmk::config::KeyboardUsbConfig = ::rmk::config::KeyboardUsbConfig {
            vid: #vid,
            pid: #pid,
            manufacturer: #manufacturer,
            product_name: #product_name,
            serial_number: #serial_number,
        };
    }
}

pub(crate) fn expand_vial_config() -> proc_macro2::TokenStream {
    quote! {
        include!(concat!(env!("OUT_DIR"), "/config_generated.rs"));
        const VIAL_CONFIG: ::rmk::config::VialConfig = ::rmk::config::VialConfig {
            vial_keyboard_id: &VIAL_KEYBOARD_ID,
            vial_keyboard_def: &VIAL_KEYBOARD_DEF,
        };
    }
}
