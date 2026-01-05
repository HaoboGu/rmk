#![no_main]
#![no_std]

use core::cell::RefCell;
use rmk::event::Event;
use rmk::input_device::{InputDevice, InputProcessor, ProcessResult};
use rmk::keymap::KeyMap;
use rmk::macros::rmk_central;

// Pimoroni Trackball device implementation
// I2C-based RGB trackball for cursor control
pub struct Trackball {
    // In real implementation: I2C peripheral and interrupt pin
    sample_count: u32,
}

impl Trackball {
    pub fn new() -> Self {
        Self { sample_count: 0 }
    }
}

impl InputDevice for Trackball {
    async fn read_event(&mut self) -> Event {
        // Simulate trackball sensor reading
        // In real implementation: read X/Y deltas and button state from I2C (address 0x0A)
        self.sample_count += 1;

        let mut data = [0u8; 16];
        // Bytes 0-1: X movement (signed 8-bit)
        data[0] = ((self.sample_count % 20) as i8 - 10) as u8;
        // Bytes 2-3: Y movement (signed 8-bit)
        data[1] = ((self.sample_count % 30) as i8 - 15) as u8;
        // Byte 4: Button state (bit 0: click, bit 1: wheel)
        data[2] = if self.sample_count % 50 == 0 { 0x01 } else { 0x00 };

        Event::Custom(data)
    }
}

// Scroll wheel processor for trackball
// Converts Y-axis movement to scroll when Fn key is held
pub struct ScrollWheelProcessor<
    'a,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
> {
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    scroll_mode: bool,
    accumulated_scroll: i16,
}

impl<
        'a,
        const ROW: usize,
        const COL: usize,
        const NUM_LAYER: usize,
        const NUM_ENCODER: usize,
    > ScrollWheelProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    pub fn new(keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>) -> Self {
        Self {
            keymap,
            scroll_mode: false,
            accumulated_scroll: 0,
        }
    }

    fn check_scroll_modifier(&self) -> bool {
        // In real implementation: check if Fn key is pressed
        // let keymap = self.keymap.borrow();
        // keymap.is_key_pressed(FN_KEY_ROW, FN_KEY_COL)
        false // Placeholder
    }
}

impl<
        'a,
        const ROW: usize,
        const COL: usize,
        const NUM_LAYER: usize,
        const NUM_ENCODER: usize,
    > InputProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
    for ScrollWheelProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    async fn process(&mut self, event: Event) -> ProcessResult {
        match event {
            Event::Custom(data) => {
                // Process trackball movement events
                let _x_movement = data[0] as i8;
                let y_movement = data[1] as i8;
                let _button_state = data[2];

                // Check if scroll modifier key is held
                self.scroll_mode = self.check_scroll_modifier();

                if self.scroll_mode && y_movement != 0 {
                    // Convert Y movement to scroll wheel events
                    self.accumulated_scroll += y_movement as i16;

                    // Send scroll report when accumulated enough movement
                    if self.accumulated_scroll.abs() >= 4 {
                        // In real implementation:
                        // - Generate HID scroll wheel report
                        // - Send via self.send_report()
                        self.accumulated_scroll = 0;
                    }

                    // Event is handled - don't pass to mouse processor
                    ProcessResult::Stop
                } else {
                    // Pass through as normal mouse movement
                    ProcessResult::Continue(event)
                }
            }
            _ => {
                // Pass other events to next processor in chain
                ProcessResult::Continue(event)
            }
        }
    }

    fn get_keymap(&self) -> &RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>> {
        self.keymap
    }
}

#[rmk_central]
mod keyboard_central {
    // Custom I2C trackball device
    // Pimoroni RGB Trackball - cursor control with click button
    #[device]
    fn trackball() -> Trackball {
        // In real implementation: pass I2C peripheral and interrupt pin from `p`
        // e.g., Trackball::new(p.I2C0, p.P0_26, p.P0_27)
        Trackball::new()
    }

    // Custom scroll wheel processor
    // Converts trackball Y-axis to scroll when Fn key is held
    #[processor]
    fn scroll_processor() -> ScrollWheelProcessor {
        ScrollWheelProcessor::new(&keymap)
    }
}
