use crate::config::EncoderConfig;
use crate::gpio_config::convert_gpio_str_to_input_pin;
use crate::ChipModel;
use proc_macro2::Ident;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub(crate) fn expand_encoder_device(
    encoder_config: Vec<EncoderConfig>,
    chip: &ChipModel,
) -> (TokenStream, Vec<TokenStream>, Vec<Ident>) {
    if encoder_config.is_empty() {
        return (quote! {}, Vec::new(), Vec::new());
    }

    let mut config = TokenStream::new();
    let mut processor_names = vec![];
    let mut encoder_names = vec![];

    // Add encoder processor
    let encoder_processor_ident = format_ident!("encoder_processor");
    processor_names.push(quote!(#encoder_processor_ident));
    config.extend(quote! {
        let mut #encoder_processor_ident = ::rmk::input_device::rotary_encoder::RotaryEncoderProcessor::new(&keymap);
    });

    // Create rotary encoders
    for (idx, encoder) in encoder_config.iter().enumerate() {
        let encoder_id = idx as u8;

        let pull = if encoder.internal_pullup {
            Some(true)
        } else {
            None
        };

        // Initialize pins
        let pin_a = convert_gpio_str_to_input_pin(&chip, encoder.pin_a.clone(), false, pull);
        let pin_b = convert_gpio_str_to_input_pin(&chip, encoder.pin_b.clone(), false, pull);

        let encoder_name = format_ident!("encoder_{}", encoder_id);
        encoder_names.push(encoder_name.clone());

        // Create different types of encoders based on the phase field
        let encoder_device = match encoder.phase.as_deref() {
            Some("e8h7") => {
                quote! {
                    let mut #encoder_name = ::rmk::input_device::rotary_encoder::RotaryEncoder::with_phase(
                        #pin_a,
                        #pin_b,
                        ::rmk::input_device::rotary_encoder::E8H7Phase,
                        #encoder_id
                    );
                }
            }
            Some("resolution") => {
                // When phase is "resolution", ensure resolution and reverse are set
                let resolution = encoder
                    .resolution
                    .expect("Resolution value must be specified when phase is 'resolution'");
                let reverse = encoder.reverse.unwrap_or(false);

                quote! {
                    let mut #encoder_name = ::rmk::input_device::rotary_encoder::RotaryEncoder::with_resolution(
                        #pin_a,
                        #pin_b,
                        #resolution,
                        #reverse,
                        #encoder_id
                    );
                }
            }
            Some("default") => {
                // Default phase
                quote! {
                    let mut #encoder_name = ::rmk::input_device::rotary_encoder::RotaryEncoder::with_phase(
                        #pin_a,
                        #pin_b,
                        ::rmk::input_device::rotary_encoder::DefaultPhase,
                        #encoder_id
                    );
                }
            }
            _ => {
                panic!("Invalid rotary encoder phase, available phase: default, resolution, e8h7");
            }
        };

        config.extend(quote! {
            #encoder_device
        });
    }

    (config, processor_names, encoder_names)
}
