use crate::keyboard_config::{
    expand_keyboard_info, expand_light_config, expand_matrix_config, expand_vial_config,
    get_chip_model, ChipSeries,
};
use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use rmk_config::toml_config::KeyboardTomlConfig;
use std::fs;
use syn::{ItemFn, ItemMod};

/// List of functions that can be overwritten
#[derive(Debug, Clone, Copy, FromMeta)]
pub enum Overwritten {
    Usb,
    ChipConfig,
}

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
    let keyboard_info_static_var = expand_keyboard_info(
        toml_config.keyboard.clone(),
        toml_config.matrix.rows as usize,
        toml_config.matrix.cols as usize,
        toml_config.matrix.layers as usize,
    );
    let vial_static_var = expand_vial_config();

    // TODO: 2. Generate main function
    let imports = get_imports(&chip);
    let main_function = expand_main(&chip, toml_config, item_mod);

    // TODO: 3. Insert customization code

    quote! {
        #imports

        #keyboard_info_static_var
        #vial_static_var

        #main_function
    }
}

// fn get_interrupt_binding(chip: &ChipSeries, chip_name: String) -> TokenStream2 {
//     // FIXME: The interrupt bindings varies for chips, it's impossible now to automatically set it
//     // Leave it to users for now, until there's better solution
//     match chip {
//         ChipSeries::Stm32 => {}
//         ChipSeries::Nrf52 => todo!(),
//         ChipSeries::Rp2040 => todo!(),
//         ChipSeries::Esp32 => todo!(),
//         ChipSeries::Unsupported => todo!(),
//     }
//     quote! {
//         use embassy_stm32::bind_interrupts;
//         bind_interrupts!(struct Irqs {
//             OTG_HS => InterruptHandler<USB_OTG_HS>;
//         });
//     }
// }

fn expand_main(
    chip: &ChipSeries,
    toml_config: KeyboardTomlConfig,
    item_mod: ItemMod,
) -> TokenStream2 {
    let light_config = expand_light_config(&chip, toml_config.light);
    let matrix_config = expand_matrix_config(&chip, toml_config.matrix);
    let UsbInit {
        imports: usb_import,
        initialization: usb_initialization,
    } = parse_usb_init(&chip, &item_mod);
    let ChipInit {
        imports: chip_init_imports,
        initialization: chip_init,
    } = parse_config_init(&chip, &item_mod);

    quote! {
        #usb_import
        #chip_init_imports

        #[::embassy_executor::main]
        async fn main(_spawner: ::embassy_executor::Spawner) {
            info!("RMK start!");
            // Initialize peripherals
            #chip_init

            // Usb config
            // FIXME: usb initialization (with interrupt binding)
            // It needs 3 inputs from users chip,which cannot be automatically extracted:
            // 1. USB Interrupte name
            // 2. USB periphral name
            // 3. USB GPIO
            // So, I'll leave it to users, make a stub function here
            #usb_initialization

            // FIXME: if storage is enabled
            // Use internal flash to emulate eeprom
            let f = Flash::new_blocking(p.FLASH);

            // FIXME: FIX macro
            let light_config = #light_config;
            let (input_pins, output_pins) = #matrix_config;

            let keyboard_config = RmkConfig {
                usb_config: keyboard_usb_config,
                vial_config,
                light_config,
                ..Default::default()
            };

            // Start serving
            initialize_keyboard_with_config_and_run::<
                Flash<'_, Blocking>,
                Driver<'_, USB_OTG_HS>,
                Input<'_, AnyPin>,
                Output<'_, AnyPin>,
                ROW,
                COL,
                NUM_LAYER,
            >(
                driver,
                input_pins,
                output_pins,
                Some(f),
                KEYMAP,
                keyboard_config,
            )
            .await;
        }
    }
}

fn get_imports(chip: &ChipSeries) -> TokenStream2 {
    let chip_specific_imports = match chip {
        ChipSeries::Stm32 => {
            quote! {
                // TODO: different imports by chip_name
                use embassy_stm32::{
                    flash::{Blocking, Flash},
                    gpio::{AnyPin, Input, Output},
                    peripherals::USB_OTG_HS,
                    time::Hertz,
                    usb_otg::InterruptHandler,
                };
            }
        }
        ChipSeries::Nrf52 => todo!(),
        ChipSeries::Rp2040 => todo!(),
        ChipSeries::Esp32 => todo!(),
        ChipSeries::Unsupported => todo!(),
    };

    quote! {
        use defmt::*;
        use defmt_rtt as _;
        use panic_probe as _;
        use embassy_executor::Spawner;
        #chip_specific_imports
    }
}

#[derive(Debug, Default)]
pub struct ChipInit {
    imports: TokenStream2,
    initialization: TokenStream2,
}

impl ChipInit {
    fn new_default(chip: &ChipSeries) -> Self {
        match chip {
            ChipSeries::Stm32 => ChipInit {
                imports: quote! {
                    use embassy_stm32::Config;
                },
                initialization: quote! {
                    let config = Config::default();
                },
            },
            ChipSeries::Nrf52 => todo!(),
            ChipSeries::Rp2040 => todo!(),
            ChipSeries::Esp32 => todo!(),
            ChipSeries::Unsupported => todo!(),
        }
    }
}

fn parse_config_init(chip: &ChipSeries, item_mod: &ItemMod) -> ChipInit {
    // If there is a function with `#[Overwritten(usb)]`, override the chip initialization
    if let Some((_, items)) = &item_mod.content {
        items
            .iter()
            .find_map(|item| {
                if let syn::Item::Fn(item_fn) = &item {
                    if item_fn.attrs.len() == 1 {
                        if let Ok(Overwritten::ChipConfig) =
                            Overwritten::from_meta(&item_fn.attrs[0].meta)
                        {
                            return Some(override_chip_init(item_fn));
                        }
                    }
                }
                None
            })
            .unwrap_or(ChipInit::new_default(chip))
    } else {
        ChipInit::new_default(chip)
    }
}

fn override_chip_init(item_fn: &ItemFn) -> ChipInit {
    let initialization = item_fn.block.to_token_stream();
    let imports = quote! {
        use embassy_stm32::Config;
    };
    return ChipInit {
        imports,
        initialization: quote! {
            let config = #initialization;
            let p = embassy_stm32::init(config);
        },
    };
}

#[derive(Debug)]
pub struct UsbInit {
    imports: TokenStream2,
    initialization: TokenStream2,
}

impl UsbInit {
    /// Default implementation of usb initialization
    fn new_default(chip: &ChipSeries) -> Self {
        match chip {
            ChipSeries::Stm32 => UsbInit {
                imports: quote! {
                    use static_cell::StaticCell;
                    use embassy_stm32::usb_otg::Driver;
                },
                initialization: quote! {
                    static EP_OUT_BUFFER: StaticCell<[u8; 1024]> = StaticCell::new();
                    let mut usb_config = embassy_stm32::usb_otg::Config::default();
                    usb_config.vbus_detection = false;
                    let driver = Driver::new_fs(
                        p.USB_OTG_HS,
                        Irqs,
                        p.PA12,
                        p.PA11,
                        &mut EP_OUT_BUFFER.init([0; 1024])[..],
                        usb_config,
                    );
                },
            },
            ChipSeries::Nrf52 => todo!(),
            ChipSeries::Rp2040 => todo!(),
            ChipSeries::Esp32 => todo!(),
            ChipSeries::Unsupported => todo!(),
        }
    }
}

fn parse_usb_init(chip: &ChipSeries, item_mod: &ItemMod) -> UsbInit {
    // If there is a function with `#[Overwritten(usb)]`, override the chip initialization
    if let Some((_, items)) = &item_mod.content {
        items
            .iter()
            .find_map(|item| {
                if let syn::Item::Fn(item_fn) = &item {
                    if item_fn.attrs.len() == 1 {
                        if let Ok(Overwritten::Usb) = Overwritten::from_meta(&item_fn.attrs[0].meta)
                        {
                            return Some(override_usb_init(item_fn));
                        }
                    }
                }
                None
            })
            .unwrap_or(UsbInit::new_default(chip))
    } else {
        UsbInit::new_default(chip)
    }
}

fn override_usb_init(item_fn: &ItemFn) -> UsbInit {
    // TODO: Check function definition
    let initialization = item_fn.block.to_token_stream();
    let imports = quote! {
        use static_cell::StaticCell;
        use embassy_stm32::usb_otg::Driver;
    };
    return UsbInit {
        imports,
        initialization: quote! {
            let driver = #initialization;
        },
    };
}
