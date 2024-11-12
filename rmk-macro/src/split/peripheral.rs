use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::toml_config::SplitBoardConfig;
use syn::ItemMod;

use crate::{
    chip_init::expand_chip_init,
    feature::{get_rmk_features, is_feature_enabled},
    import::expand_imports,
    keyboard_config::{read_keyboard_toml_config, BoardConfig, KeyboardConfig},
    matrix::expand_matrix_input_output_pins,
    split::central::expand_serial_init,
    ChipModel, ChipSeries,
};

/// Parse split central mod and generate a valid RMK main function with all needed code
pub(crate) fn parse_split_peripheral_mod(
    id: usize,
    _attr: proc_macro::TokenStream,
    item_mod: ItemMod,
) -> TokenStream2 {
    let rmk_features = get_rmk_features();
    if !is_feature_enabled(&rmk_features, "split") {
        return quote! {
            compile_error!("\"split\" feature of RMK should be enabled");
        };
    }

    let async_matrix = is_feature_enabled(&rmk_features, "async_matrix");

    let toml_config = match read_keyboard_toml_config() {
        Ok(c) => c,
        Err(e) => return e,
    };

    let keyboard_config = match KeyboardConfig::new(toml_config) {
        Ok(c) => c,
        Err(e) => return e,
    };

    let main_function = expand_split_peripheral(id, &keyboard_config, item_mod, async_matrix);

    let main_function_sig = if keyboard_config.chip.series == ChipSeries::Esp32 {
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
        }
    }
}

fn expand_split_peripheral(
    id: usize,
    keyboard_config: &KeyboardConfig,
    item_mod: ItemMod,
    async_matrix: bool,
) -> TokenStream2 {
    // Check whether keyboard.toml contains split section
    let split_config = match &keyboard_config.board {
        BoardConfig::Split(split) => split,
        _ => {
            return quote! {
                compile_error!("No `split` field in `keyboard.toml`");
            }
        }
    };

    let peripheral_config = split_config
        .peripheral
        .get(id)
        .expect("Missing peripheral config");

    let central_config = &split_config.central;

    let imports = expand_imports(&item_mod);
    let chip_init = expand_chip_init(keyboard_config, &item_mod);
    let matrix_config = expand_matrix_input_output_pins(
        &keyboard_config.chip,
        peripheral_config.input_pins.clone(),
        peripheral_config.output_pins.clone(),
        async_matrix,
    );

    let run_rmk_peripheral =
        expand_split_peripheral_entry(&keyboard_config.chip, peripheral_config, &central_config);

    quote! {
        #imports
        #chip_init
        #matrix_config
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
