//! Joystick
//!

use quote::{format_ident, quote};
use crate::{config::JoystickConfig, ChipModel, ChipSeries};

pub(crate) fn expand_joystick(
    chip: &ChipModel,
    joystick_config: Vec<JoystickConfig>
) -> Vec<(proc_macro2::TokenStream, (Vec<proc_macro2::TokenStream>, proc_macro2::Ident), proc_macro2::TokenStream)> {
    let mut ret = vec!();
    if chip.series == ChipSeries::Nrf52 {
        for conf in joystick_config.into_iter() {
            let mut task = proc_macro2::TokenStream::new();
            let mut channel_config = vec!();
            let joystick_ident = format_ident!("joystick_{}", conf.instance);
            let mut num = 2usize;
            for (idx, axis_pin) in conf.axis.iter().enumerate() {
                if axis_pin != "_" {
                    let pin = convert_gpio_str_to_adc_pin(chip, axis_pin.to_string());
                    channel_config.push(quote!{ {
                        use ::embassy_nrf::saadc::{Reference, Gain, Time};
                        let mut p = ::embassy_nrf::saadc::ChannelConfig::single_ended(#pin);
                        p.reference = Reference::VDD1_4;
                        p.gain = Gain::GAIN1_4;
                        p
                    } });
                } else {
                    num = 1;
                }
            }
            let trans_config = conf.transform.unwrap_or([[1i8, 1i8], [1i8, 1i8]]);
            let trans = trans_config.map(|n| {
                quote!{ [#(#n), *] }
            });
            let channel_name = format_ident!("joystick_channel_{}", conf.instance);
            let channel_sender_name = format_ident!("joystick_channel_send_{}", conf.instance);
            let channel_receiver_name = format_ident!("joystick_channel_recv_{}", conf.instance);

            let joystick_run_ident = format_ident!("joystick_run_{}", conf.instance);
            let initializer = quote!{
                    let #channel_name = Channel::<CriticalSectionRawMutex, [i16; #num], 4>::new();
                    use ::embassy_sync::{
                        channel::Channel,
                        blocking_mutex::raw::CriticalSectionRawMutex
                    };
                let #channel_sender_name = #channel_name.sender();
                let #channel_receiver_name = #channel_name.receiver();
                
                let #joystick_run_ident = || async move {
                    use ::embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
                    ::rmk::joystick::run_joystick::<#num, 1600, 4, CriticalSectionRawMutex>([#(#trans), *], #channel_receiver_name).await;
                };
            };

            task.extend(quote!{
                #joystick_run_ident()
            });

            ret.push((initializer, (channel_config, channel_sender_name), task))
        };
        ret
    } else {
        vec!((quote!{ compile_error!("\"Joystick\" only support Nrf52 now"); },
            (vec!(), proc_macro2::Ident::new("Useless", proc_macro2::Span::call_site())),
            proc_macro2::TokenStream::new()))
    }

}

pub(crate) fn convert_gpio_str_to_adc_pin(chip: &ChipModel, axis_pin: String) -> proc_macro2::TokenStream {
    if chip.series == ChipSeries::Nrf52 {
        let axis_ident = format_ident!("{}", axis_pin);
        quote!{ p.#axis_ident }
    } else {
        quote!{ compile_error!("\"Joystick\" only support for Nrf52 now"); }
    }
}
