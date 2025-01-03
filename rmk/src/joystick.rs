use embassy_futures::yield_now;

use embassy_sync::{
    blocking_mutex::raw::RawMutex, channel::Receiver };
use crate::{
    usb::descriptor::{CompositeReport, CompositeReportType},
    keyboard::{ keyboard_report_channel, KeyboardReportMessage}
};
use defmt::info;

/// Run Joystick
/// It process the input of 2(or 1) channels analog input device.
/// Sampling the analog input device
/// `TRANS` perform transformation on the input channels,
/// `NUM` number of channels,
/// the sampler rate is 16MHz / `RATE_DIVISOR`.
pub async fn run_joystick<
    const NUM: usize,
    const RATE_DIVISOR: u32,
    const N: usize,
    M: RawMutex
    > (trans: [[i8; 2]; 2], recv: Receiver<'_, M, [i16; 2], N>) {
        assert!(NUM <= 2);
        let mut report = CompositeReport::default();
        let sender = keyboard_report_channel.sender();
        let divider = 5;
        let center = [125, 113];
        let dead = 5;

        loop {
            let buf = recv.receive().await;
            let ch1 = get_adc(buf[0]);
            let ch2 = get_adc(buf[1]);
            report.x = if ch1 > center[0] - dead && ch1 < center[0] + dead {
                0
            } else {
                (ch1 as i16 - (u8::MAX as i16 / 2)).try_into().unwrap()
            };
            
            report.y = if ch2 > center[1] - dead && ch2 < center[1] + dead {
                0
            } else {
                (ch2 as i16 - (u8::MAX as i16 / 2)).try_into().unwrap()
            };
            report.x = report.x / divider;
            report.y = report.y / divider;

            // high report rate cause error
            if report.x == 0 && report.y == 0 {
                yield_now().await;
                continue;
            }
            
            //info!("joystick: {} {}", ch1, ch2);
            sender.send(KeyboardReportMessage::CompositeReport(report, CompositeReportType::Mouse)).await;
            ::embassy_time::Timer::after_millis(20).await;
        }
    }

fn get_adc(val: i16) -> u8 {
    // Avoid overflow
    let val = val as i32;

    // According to nRF52840's datasheet, for single_ended saadc:
    // val = v_adc * (gain / reference) * 2^(resolution)
    //
    // setting, gain = 1/4, reference = VDD, resolution = 12bits, so:
    // val = v_adc / VDD * 4096
    (val * 255 / 4096).try_into().unwrap()
}
