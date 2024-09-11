use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::toml_config::{SplitBoardConfig, SplitConfig};
use syn::ItemMod;

use crate::{
    chip_init::expand_chip_init,
    feature::{get_rmk_features, is_feature_enabled},
    import::expand_imports,
    keyboard::get_chip_info,
    keyboard_config::read_keyboard_config,
    matrix::expand_matrix_input_output_pins,
    split::central::expand_serial_init,
    ChipModel, ChipSeries,
};

/// Parse split central mod and generate a valid RMK main function with all needed code
pub(crate) fn parse_split_peripheral_mod(
    id: usize,
    attr: proc_macro::TokenStream,
    item_mod: ItemMod,
) -> TokenStream2 {
    let rmk_features = get_rmk_features();
    if !is_feature_enabled(&rmk_features, "split") {
        return quote! {
            compile_error!("\"split\" feature of RMK should be enabled");
        };
    }

    let async_matrix = is_feature_enabled(&rmk_features, "async_matrix");

    let toml_config = match read_keyboard_config(attr) {
        Ok(c) => c,
        Err(e) => return e,
    };

    let (chip, _comm_type, _usb_info) = match get_chip_info(&toml_config) {
        Ok(value) => value,
        Err(e) => return e,
    };

    // Check whether keyboard.toml contains split section
    let split_config = match &toml_config.split {
        Some(c) => c,
        None => return quote! { compile_error!("No `split` field in `keyboard.toml`"); }.into(),
    };

    let main_function = expand_split_peripheral(id, &chip, split_config, item_mod, async_matrix);

    let main_function_sig = if chip.series == ChipSeries::Esp32 {
        quote! {
            use ::esp_idf_svc::hal::gpio::*;
            use esp_println as _;
            fn main()
        }
    } else {
        quote! {
            #[::embassy_executor::main]
            async fn main(spawner: ::embassy_executor::Spawner)
        }
    };

    quote! {
        use defmt_rtt as _;
        use panic_probe as _;
        use defmt::*;

        #main_function_sig {
            ::defmt::info!("RMK start!");
            #main_function
            // // Initialize peripherals as `p`
            // #chip_init

            // // Initialize matrix config as `(input_pins, output_pins)`
            // #matrix_config;

            // #split_communicate

            // // Start serving
            // #run_rmk
        }
    }
}

fn expand_split_peripheral(
    id: usize,
    chip: &ChipModel,
    split_config: &SplitConfig,
    item_mod: ItemMod,
    async_matrix: bool,
) -> TokenStream2 {
    let peripheral_config = split_config
        .peripheral
        .get(id)
        .expect("Missing peripheral config");

    let central_config = &split_config.central;

    let imports = expand_imports(&item_mod);
    let chip_init = expand_chip_init(chip, &item_mod);
    let matrix_config = expand_matrix_input_output_pins(
        chip,
        peripheral_config.input_pins.clone(),
        peripheral_config.output_pins.clone(),
        async_matrix,
    );

    let run_rmk_peripheral =
        expand_split_peripheral_entry(chip, peripheral_config, &central_config);

    quote! {
        #imports
        #chip_init
        #matrix_config; // FIXME: remove symbol ;
        #run_rmk_peripheral
    }
}
fn expand_split_peripheral_entry(
    chip: &ChipModel,
    peripheral_config: &SplitBoardConfig,
    central_config: &SplitBoardConfig,
) -> TokenStream2 {
    match chip.series {
        ChipSeries::Stm32 => todo!(),
        ChipSeries::Nrf52 => {
            let central_addr = central_config
                .ble_addr
                .expect("Missing central ble address");
            let row = peripheral_config.rows;
            let col = peripheral_config.cols;
            let peripheral_addr = peripheral_config.ble_addr.expect(
                "Peripheral should have a ble address, please check the `ble_addr` field in `keyboard.toml`",
            );
            quote! {
                ::rmk::split::peripheral::run_rmk_split_peripheral::<
                    ::embassy_nrf::gpio::Input<'_>,
                    ::embassy_nrf::gpio::Output<'_>,
                    #row,
                    #col
                > (
                    input_pins,
                    output_pins,
                    [#(#central_addr), *],
                    [#(#peripheral_addr), *],
                    spawner,
                ).await
            }
        }
        ChipSeries::Rp2040 => {
            let peripheral_serial = peripheral_config
                .serial
                .clone()
                .expect("Missing peripheral serial config");
            if peripheral_serial.len() != 1 {
                panic!("Peripheral should have only one serial config");
            }
            let serial_init = expand_serial_init(chip, peripheral_serial);

            let row = peripheral_config.rows as usize;
            let col = peripheral_config.cols as usize;
            let peripheral_run = quote! {
                ::rmk::split::peripheral::run_rmk_split_peripheral::<
                    ::embassy_rp::gpio::Input<'_>,
                    ::embassy_rp::gpio::Output<'_>,
                    _,
                    #row,
                    #col,
                >(input_pins, output_pins, uart0).await;
            };
            quote! {
                #serial_init
                #peripheral_run
            }
        }
        ChipSeries::Esp32 => todo!(),
    }
}
