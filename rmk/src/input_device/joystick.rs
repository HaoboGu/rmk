use rmk_macro::processor;
use usbd_hid::descriptor::MouseReport;

use crate::channel::KEYBOARD_REPORT_CHANNEL;
use crate::event::PointingEvent;
use crate::hid::Report;
use crate::keymap::KeyMap;

#[processor(subscribe = [PointingEvent])]
pub struct JoystickProcessor<
    'a,
    const N: usize,
> {
    transform: [[i16; N]; N],
    bias: [i16; N],
    keymap: &'a KeyMap<'a>,
    record: [i16; N],
    resolution: u16,
}

impl<'a, const N: usize> JoystickProcessor<'a, N> {
    pub fn new(
        transform: [[i16; N]; N],
        bias: [i16; N],
        resolution: u16,
        keymap: &'a KeyMap<'a>,
    ) -> Self {
        Self {
            transform,
            bias,
            resolution,
            keymap,
            record: [0; N],
        }
    }

    async fn on_pointing_event(&mut self, event: PointingEvent) {
        for (rec, e) in self.record.iter_mut().zip(event.0.iter()) {
            *rec = e.value;
        }
        debug!("Joystick info: {:#?}", self.record);
        self.generate_report().await;
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
        let buttons = self.keymap.mouse_buttons();
        let mouse_report = MouseReport {
            buttons,
            x: (report[0].clamp(i8::MIN as i16, i8::MAX as i16)) as i8,
            y: (report[1].clamp(i8::MIN as i16, i8::MAX as i16)) as i8,
            wheel: 0,
            pan: 0,
        };

        // Send mouse report directly
        KEYBOARD_REPORT_CHANNEL.send(Report::MouseReport(mouse_report)).await;
    }
}
