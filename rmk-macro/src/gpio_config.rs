use quote::{format_ident, quote};
use rmk_config::{ChipModel, ChipSeries};

pub(crate) fn convert_output_pins_to_initializers(chip: &ChipModel, pins: Vec<String>) -> proc_macro2::TokenStream {
    let mut initializers = proc_macro2::TokenStream::new();
    let mut idents = vec![];
    let pin_initializers = pins
        .into_iter()
        .map(|p| (p.clone(), convert_gpio_str_to_output_pin(chip, p, false)))
        .map(|(p, ts)| {
            let ident_name = format_ident!("{}", p.to_lowercase());
            idents.push(ident_name.clone());
            quote! { let #ident_name = #ts;}
        });

    initializers.extend(pin_initializers);
    let output_pin_type = get_output_pin_type(chip);
    let len = idents.len();
    initializers.extend(quote! {let output_pins: [#output_pin_type; #len] = [#(#idents), *];});
    initializers
}

pub(crate) fn convert_input_pins_to_initializers(
    chip: &ChipModel,
    pins: Vec<String>,
    async_matrix: bool,
) -> proc_macro2::TokenStream {
    let mut initializers = proc_macro2::TokenStream::new();
    let mut idents = vec![];
    let pin_initializers = pins
        .into_iter()
        .map(|p| {
            (
                p.clone(),
                convert_gpio_str_to_input_pin(chip, p, async_matrix, Some(false)), // low active = false == pull down
            )
        })
        .map(|(p, ts)| {
            let ident_name = format_ident!("{}", p.to_lowercase());
            idents.push(ident_name.clone());
            quote! { let #ident_name = #ts;}
        });
    initializers.extend(pin_initializers);
    let input_pin_type = get_input_pin_type(chip, async_matrix);
    let len = idents.len();
    initializers.extend(quote! {let input_pins: [#input_pin_type; #len] = [#(#idents), *];});
    initializers
}

pub(crate) fn get_input_pin_type(chip: &ChipModel, async_matrix: bool) -> proc_macro2::TokenStream {
    match chip.series {
        ChipSeries::Stm32 => {
            if async_matrix {
                quote! {::embassy_stm32::exti::ExtiInput}
            } else {
                quote! {::embassy_stm32::gpio::Input}
            }
        }
        ChipSeries::Nrf52 => quote! { ::embassy_nrf::gpio::Input },
        ChipSeries::Rp2040 => quote! { ::embassy_rp::gpio::Input },
        ChipSeries::Esp32 => quote! { ::esp_hal::gpio::Input },
    }
}

pub(crate) fn get_output_pin_type(chip: &ChipModel) -> proc_macro2::TokenStream {
    match chip.series {
        ChipSeries::Stm32 => quote! {::embassy_stm32::gpio::Output},
        ChipSeries::Nrf52 => quote! {::embassy_nrf::gpio::Output},
        ChipSeries::Rp2040 => quote! {::embassy_rp::gpio::Output},
        ChipSeries::Esp32 => quote! { ::esp_hal::gpio::Output },
    }
}

pub(crate) fn convert_direct_pins_to_initializers(
    chip: &ChipModel,
    pins: Vec<Vec<String>>,
    async_matrix: bool,
    low_active: bool,
) -> proc_macro2::TokenStream {
    let mut initializers = proc_macro2::TokenStream::new();
    let mut row_idents = vec![];
    // Process each row of pins
    for (row_idx, row_pins) in pins.into_iter().enumerate() {
        let mut col_idents = vec![];
        // Process each pin in the current row
        let pin_initializers = row_pins.into_iter().map(|p| {
            let ident_name = format_ident!("{}_{}_{}", p.to_lowercase(), row_idx, col_idents.len());
            col_idents.push(ident_name.clone());
            if p != "_" && p.to_lowercase() != "trns" {
                // Convert pin to Some(pin) when it's not transparent
                let pin = convert_gpio_str_to_input_pin(chip, p, async_matrix, Some(low_active)); // low active = false == pull down
                quote! { let #ident_name = Some(#pin); }
            } else {
                quote! { let #ident_name = None; }
            }
        });
        // Extend initializers with current row's pin initializations
        initializers.extend(pin_initializers);
        // Create array for current row
        let row_ident = format_ident!("direct_pins_row_{}", row_idx);
        initializers.extend(quote! {
            let #row_ident = [#(#col_idents),*];
        });
        row_idents.push(row_ident);
    }
    // Create final 2D array
    initializers.extend(quote! {
        let direct_pins = [#(#row_idents),*];
    });
    initializers
}

pub(crate) fn convert_gpio_str_to_output_pin(
    chip: &ChipModel,
    gpio_name: String,
    low_active: bool,
) -> proc_macro2::TokenStream {
    let gpio_ident = format_ident!("{}", gpio_name);
    let default_level_ident = if low_active {
        format_ident!("High")
    } else {
        format_ident!("Low")
    };
    match chip.series {
        ChipSeries::Stm32 => {
            quote! {
                ::embassy_stm32::gpio::Output::new(p.#gpio_ident, ::embassy_stm32::gpio::Level::#default_level_ident, ::embassy_stm32::gpio::Speed::VeryHigh)
            }
        }
        ChipSeries::Nrf52 => {
            quote! {
                ::embassy_nrf::gpio::Output::new(p.#gpio_ident, ::embassy_nrf::gpio::Level::#default_level_ident, ::embassy_nrf::gpio::OutputDrive::Standard)
            }
        }
        ChipSeries::Rp2040 => {
            quote! {
                ::embassy_rp::gpio::Output::new(p.#gpio_ident, ::embassy_rp::gpio::Level::#default_level_ident)
            }
        }
        ChipSeries::Esp32 => {
            quote! {
                ::esp_hal::gpio::Output::new(p.#gpio_ident, ::esp_hal::gpio::Level::#default_level_ident, ::esp_hal::gpio::OutputConfig::default())
            }
        }
    }
}

pub(crate) fn convert_gpio_str_to_input_pin(
    chip: &ChipModel,
    gpio_name: String,
    async_matrix: bool,
    pull: Option<bool>,
) -> proc_macro2::TokenStream {
    let gpio_ident = format_ident!("{}", gpio_name);
    let default_pull_ident = match pull {
        Some(true) => format_ident!("Up"),
        Some(false) => format_ident!("Down"),
        None => format_ident!("None"),
    };
    match chip.series {
        ChipSeries::Stm32 => {
            if async_matrix {
                // If async_matrix is enabled, use ExtiInput for input pins
                match get_pin_num_stm32(&gpio_name) {
                    Some(pin_num) => {
                        let pin_num_ident = format_ident!("EXTI{}", pin_num);
                        quote! {
                            ::embassy_stm32::exti::ExtiInput::new(p.#gpio_ident, p.#pin_num_ident, ::embassy_stm32::gpio::Pull::#default_pull_ident)
                        }
                    }
                    None => {
                        panic!("\n❌ keyboard.toml: Invalid pin definition: {}", gpio_name);
                    }
                }
            } else {
                quote! {
                    ::embassy_stm32::gpio::Input::new(p.#gpio_ident, ::embassy_stm32::gpio::Pull::#default_pull_ident)
                }
            }
        }
        ChipSeries::Nrf52 => {
            quote! {
                ::embassy_nrf::gpio::Input::new(p.#gpio_ident, ::embassy_nrf::gpio::Pull::#default_pull_ident)
            }
        }
        ChipSeries::Rp2040 => {
            quote! {
                ::embassy_rp::gpio::Input::new(p.#gpio_ident, ::embassy_rp::gpio::Pull::#default_pull_ident)
            }
        }
        ChipSeries::Esp32 => {
            quote! {
                {
                    let mut pin = ::esp_hal::gpio::Input::new(p.#gpio_ident, ::esp_hal::gpio::InputConfig::default().with_pull(::esp_hal::gpio::Pull::#default_pull_ident));
                    pin
                }
            }
        }
    }
}

/// Get pin number from pin str.
/// For example, if the pin str is "PD13", this function will return "13".
fn get_pin_num_stm32(gpio_name: &str) -> Option<String> {
    if gpio_name.len() < 3 {
        None
    } else {
        Some(gpio_name[2..].to_string())
    }
}
