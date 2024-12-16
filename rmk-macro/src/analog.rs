use proc_macro2::{TokenStream, Ident};
use quote::{ quote, format_ident };
use crate::keyboard_config::{ KeyboardConfig, CommunicationConfig };

pub fn expand_analog(keyboard_config: &KeyboardConfig, channel_config:Vec<(Vec<TokenStream>, Ident)>) -> (TokenStream, TokenStream) {
    let mut config = TokenStream::new();
    let mut task = TokenStream::new();
    let mut channels = vec!();
    let mut channel_sep = vec!(0usize);
    let mut pin_channels = vec!();
    match &keyboard_config.communication {
        CommunicationConfig::Ble(ble) | CommunicationConfig::Both(_, ble) => {
            if ble.enabled {
                if let Some(adc_pin) = ble.battery_adc_pin.clone() {
                    let adc_pin_def = if adc_pin == "vddh" {
                        quote! { ::embassy_nrf::saadc::VddhDiv5Input }
                    } else {
                        let adc_pin_ident = format_ident!("{}", adc_pin);
                        quote! { p.#adc_pin_ident.degrade_saadc() }
                    };
                    let channel_cfg = format_ident!("channel_cfg_{}", channel_sep.len());
                    pin_channels.push(quote!{ ::embassy_nrf::saadc::ChannelConfig::single_ended(#adc_pin_def) });
                    channels.push(quote!{ ::rmk::ble::nrf::BLE_BATTERY_CHANNEL });
                    channel_sep.push(channel_sep.last().unwrap() + 1);
                }
            }
        }
        _ => {}
    };

    channel_config.into_iter().for_each(|(mut channel_config, channel_ident)| {
        channel_sep.push(channel_sep.last().unwrap() + channel_config.len());
        pin_channels.append(&mut channel_config);
        channels.push(quote!{ #channel_ident });
    });
    
    let rev_side = channel_sep.iter().rev().collect::<Vec<_>>();
    let range = rev_side[1..].into_iter().rev().zip(channel_sep[1..].into_iter());
    let send_expr = range.zip(channels).map(|((l, r), c)| {
        let idx = **l..*r;
        quote!{
            if !#c.is_full() {
                if let Err(e) = #c.try_send([#(buf[#idx]), *]) {
                    warn!("failed to send analog info between {}, {}, because {}", #l, #r, e);
                }
            }
        }
    });
    
    let buf_size = channel_sep.last();
    
    config.extend(quote! {
        let run_analog_monitor = {
            let config = ::embassy_nrf::saadc::Config::default();
            ::embassy_nrf::interrupt::SAADC.set_priority(::embassy_nrf::interrupt::Priority::P3);
            let mut saadc = ::embassy_nrf::saadc::Saadc::new(p.SAADC, Irqs, config, [#(#pin_channels), *]);
            // Wait for ADC calibration.
            saadc.calibrate().await;
            
            || async move {
                let mut buf = [0i16; #buf_size];
                loop {
                    //info!("start to sample!");
                    saadc.sample(&mut buf).await;
                    
                    //info!("saadc sample: {}", buf);
                    #(#send_expr)*
                }
            }
        };
    });

    task.extend(quote!{
        run_analog_monitor()
    });
    (config, task)
}
