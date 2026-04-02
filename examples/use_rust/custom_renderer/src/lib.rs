#![no_std]

mod frames;

use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use rmk::display::{DisplayRenderer, RenderContext};

/// WPM below this → Idle (breathing animation).
const IDLE_WPM: u16 = 10;
/// WPM above this → Fury (both paws smashing).
const FURY_WPM: u16 = 40;
/// Render ticks between idle animation frame advances.
const IDLE_TICKS_PER_FRAME: u8 = 5;
/// Render ticks between fury frame toggles.
const FURY_TICKS_PER_FRAME: u8 = 2;
/// Render ticks to hold the tap frame before returning to prep.
const TAP_HOLD_TICKS: u8 = 3;

/// Animation state.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    /// Cat resting — cycles idle frames.
    Idle,
    /// Cat typing — alternates paws on each key press.
    Tap,
    /// Cat going wild — rapidly alternates both paws.
    Fury,
}

/// Bongo Cat OLED renderer.
///
/// Draws the classic Bongo Cat animation on a 128×32 OLED.
/// - **Idle** (low WPM): subtle breathing animation.
/// - **Tap** (moderate WPM): each key press alternates left/right paw.
/// - **Fury** (high WPM): both paws smashing rapidly.
///
/// Frame data from QMK firmware (GPL-2.0, keyboards/torn).
pub struct BongoCatRenderer {
    state: State,
    /// Which tap frame to show (false = left paw, true = right paw).
    tap_paw: bool,
    /// Edge detection for key_pressed.
    prev_pressed: bool,
    /// Idle animation frame index.
    idle_frame: u8,
    /// Tick counter for frame timing.
    tick: u8,
    /// Remaining ticks to hold the tap frame before returning to prep.
    tap_hold: u8,
}

impl Default for BongoCatRenderer {
    fn default() -> Self {
        Self {
            state: State::Idle,
            tap_paw: false,
            prev_pressed: false,
            idle_frame: 0,
            tick: 0,
            tap_hold: 0,
        }
    }
}

impl DisplayRenderer<BinaryColor> for BongoCatRenderer {
    fn render<D: DrawTarget<Color = BinaryColor>>(&mut self, ctx: &RenderContext, display: &mut D) {
        // Detect new key press (rising edge)
        let new_press = ctx.key_pressed && !self.prev_pressed;
        self.prev_pressed = ctx.key_pressed;

        // WPM drives state transitions
        let new_state = if ctx.wpm >= FURY_WPM {
            State::Fury
        } else if ctx.wpm >= IDLE_WPM {
            State::Tap
        } else {
            State::Idle
        };

        if new_state != self.state {
            self.state = new_state;
            self.tick = 0;
            self.idle_frame = 0;
        }

        // Alternate paw on each key press (used in Tap state)
        if new_press {
            self.tap_paw = !self.tap_paw;
            self.tap_hold = TAP_HOLD_TICKS;
        }

        display.clear(BinaryColor::Off).ok();

        let data: &[u8; frames::FRAME_SIZE] = match self.state {
            State::Idle => {
                self.tick += 1;
                if self.tick >= IDLE_TICKS_PER_FRAME {
                    self.tick = 0;
                    self.idle_frame = (self.idle_frame + 1) % frames::IDLE.len() as u8;
                }
                &frames::IDLE[self.idle_frame as usize]
            }
            State::Tap => {
                if self.tap_hold > 0 {
                    self.tap_hold -= 1;
                    &frames::TAP[self.tap_paw as usize]
                } else {
                    &frames::PREP
                }
            }
            State::Fury => {
                // Alternate: both paws down → prep (raised) → ...
                self.tick += 1;
                if self.tick >= FURY_TICKS_PER_FRAME {
                    self.tick = 0;
                    self.tap_paw = !self.tap_paw;
                }
                if self.tap_paw { &frames::FURY } else { &frames::PREP }
            }
        };

        draw_page_format_frame(display, data, 0, 0);
    }
}

/// Draw a frame stored in SSD1306 page format onto an embedded-graphics display.
///
/// Page format: 4 pages of 128 bytes each. Each byte is a column of 8 vertical
/// pixels in one page, with bit 0 at the top of the 8-pixel strip.
fn draw_page_format_frame<D: DrawTarget<Color = BinaryColor>>(
    display: &mut D,
    data: &[u8; frames::FRAME_SIZE],
    offset_x: i32,
    offset_y: i32,
) {
    const PAGES: usize = 4; // 32 / 8
    const COLS: usize = 128;

    for page in 0..PAGES {
        for col in 0..COLS {
            let byte = data[page * COLS + col];
            if byte == 0 {
                continue;
            }
            for bit in 0..8u32 {
                if byte & (1 << bit) != 0 {
                    let x = col as i32 + offset_x;
                    let y = page as i32 * 8 + bit as i32 + offset_y;
                    Pixel(Point::new(x, y), BinaryColor::On).draw(display).ok();
                }
            }
        }
    }
}
