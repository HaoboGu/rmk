use usbd_hid::descriptor::MouseReport;

use crate::{
    channel::KEYBOARD_REPORT_CHANNEL,
    event::{AnalogEvent, Event},
    hid::Report,
    input_device::{InputProcessor, ProcessResult},
    keymap::KeyMap,
};
use core::cell::RefCell;

pub struct JoystickProcessor<
    'a,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const N: usize,
> {
    adc_id: [u8; N],
    transform: [[i16; N]; N],
    bias: [i16; N],
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>>,
    record: [i16; N],
    resolution: u16,
    filled_bit: u8,
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const N: usize>
    JoystickProcessor<'a, ROW, COL, NUM_LAYER, N>
{
    pub fn new(
        adc_id: [u8; N],
        transform: [[i16; N]; N],
        bias: [i16; N],
        resolution: u16,
        keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>>,
    ) -> Self {
        Self {
            adc_id,
            transform,
            bias,
            resolution,
            keymap,
            record: [0; N],
            filled_bit: 0,
        }
    }
    async fn generate_report(&mut self) {
        let mut report = [0i16; N];

        debug!(
            "JoystickProcessor::generate_report: record = {:?}",
            self.record
        );
        for (rec, b) in self.record.iter_mut().zip(self.bias.iter()) {
            *rec = rec.saturating_add(*b);
        }

        for (rep, transform) in report.iter_mut().zip(self.transform.iter()) {
            for (w, v) in transform.iter().zip(self.record) {
                if *w == 0 {
                    // ignore zero weight
                    continue;
                }
                *rep = rep.saturating_add(v.saturating_div(*w));
                *rep = *rep - *rep % self.resolution as i16;
            }
        }

        debug!("JoystickProcessor::generate_report: report = {:?}", report);
        // map to mouse
        let mouse_report = MouseReport {
            buttons: 0,
            x: (report[0].clamp(i8::MIN as i16, i8::MAX as i16)) as i8,
            y: (report[1].clamp(i8::MIN as i16, i8::MAX as i16)) as i8,
            wheel: 0,
            pan: 0,
        };
        self.send_report(Report::MouseReport(mouse_report)).await;

        // clean the filled bits
        self.filled_bit = 0;
    }
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const N: usize>
    InputProcessor<'a, ROW, COL, NUM_LAYER> for JoystickProcessor<'a, ROW, COL, NUM_LAYER, N>
{
    async fn process(&mut self, event: Event) -> ProcessResult {
        embassy_time::Timer::after_millis(5).await;
        match event {
            Event::Analog(AnalogEvent { id, value }) => {
                if let Some(idx) = self.adc_id.iter().position(|&adc_id| adc_id == id) {
                    self.record[idx] = ((value as i32) + core::i16::MIN as i32) as i16;
                    self.filled_bit |= 1 << idx;

                    if self.filled_bit == (1 << N) - 1 {
                        self.generate_report().await;
                    }

                    ProcessResult::Stop
                } else {
                    ProcessResult::Continue(event)
                }
            }
            _ => ProcessResult::Continue(event),
        }
    }

    /// Send the processed report.
    async fn send_report(&self, report: Report) {
        KEYBOARD_REPORT_CHANNEL.send(report).await;
    }

    /// Get the current keymap
    fn get_keymap(&self) -> &RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>> {
        self.keymap
    }
}
