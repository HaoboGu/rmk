use proc_macro2::TokenStream;
use quote::{ quote, format_ident };
use crate::keyboard_config::{ KeyboardConfig, CommunicationConfig };

pub fn expand_analog(keyboard_config: &KeyboardConfig) -> (TokenStream, TokenStream) {
    let mut config = TokenStream::new();
    let mut task = TokenStream::new();
    let mut pins = vec!();
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
                    pins.push(quote! {
                        // use ::embassy_nrf::saadc::Input as _;
                        // Then we initialize the ADC. We are only using one channel in this example.
                        let #channel_cfg = ::embassy_nrf::saadc::ChannelConfig::single_ended(#adc_pin_def);
                    });
                    pin_channels.push(channel_cfg);
                    channels.push(quote!{ ::rmk::ble::nrf::BLE_BATTERY_CHANNEL.try_send });
                    channel_sep.push(channel_sep.last().unwrap() + 1);
                }
            }
        }
        _ => {}
    };
    let rev_side = channel_sep.iter().rev().collect::<Vec<_>>();
    let range = rev_side[1..].into_iter().rev().zip(channel_sep[1..].into_iter());
    let send_expr = range.zip(channels).map(|((l, r), c)| {
        let idx = **l..*r;
        quote!{
            match #c([#(buf[#idx]), *]) {
                Ok(_) => break,
                Err(e) => warn!("failed to send analog info between {}, {}", #l, #r),
            }
        }
    });
    
    let buf_size = channel_sep.last();
    
    config.extend(quote! {
        let run_analog_monitor = {
           #(#pins) *
            let config = ::embassy_nrf::saadc::Config::default();
            ::embassy_nrf::interrupt::SAADC.set_priority(::embassy_nrf::interrupt::Priority::P3);
            let mut saadc = ::embassy_nrf::saadc::Saadc::new(p.SAADC, Irqs, config, [#(#pin_channels), *]);
            // Wait for ADC calibration.
            saadc.calibrate().await;
            
            || async move {
                let mut buf = [0i16; #buf_size];
                loop {
                    saadc.sample(&mut buf).await;
                    info!("saadc sample: {}", buf);
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
