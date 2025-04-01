use core::cell::RefCell;

use usbd_hid::descriptor::MouseReport;

use crate::channel::KEYBOARD_REPORT_CHANNEL;
use crate::event::Event;
use crate::hid::Report;
use crate::input_device::{InputProcessor, ProcessResult};
use crate::keymap::KeyMap;

pub struct JoystickProcessor<
    'a,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
    const N: usize,
> {
    transform: [[i16; N]; N],
    bias: [i16; N],
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    record: [i16; N],
    resolution: u16,
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize, const N: usize>
    JoystickProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER, N>
{
    pub fn new(
        transform: [[i16; N]; N],
        bias: [i16; N],
        resolution: u16,
        keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    ) -> Self {
        Self {
            transform,
            bias,
            resolution,
            keymap,
            record: [0; N],
        }
    }
    async fn generate_report(&mut self) {
        let mut report = [0i16; N];

        debug!("JoystickProcessor::generate_report: record = {:?}", self.record);
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
    }
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize, const N: usize>
    InputProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
    for JoystickProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER, N>
{
    async fn process(&mut self, event: Event) -> ProcessResult {
        embassy_time::Timer::after_millis(5).await;
        match event {
            Event::Joystick(event) => {
                for (rec, e) in self.record.iter_mut().zip(event.iter()) {
                    *rec = e.value;
                }
                debug!("Joystick info: {:#?}", self.record);
                self.generate_report().await;
                ProcessResult::Stop
            }
            _ => ProcessResult::Continue(event),
        }
    }

    /// Send the processed report.
    async fn send_report(&self, report: Report) {
        KEYBOARD_REPORT_CHANNEL.send(report).await;
    }

    /// Get the current keymap
    fn get_keymap(&self) -> &RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>> {
        self.keymap
    }
}
