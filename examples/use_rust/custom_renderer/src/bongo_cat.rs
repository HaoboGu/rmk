use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::Text;
use rmk::display::{DisplayRenderer, RenderContext};

use crate::{draw_page_format_frame, frames};

const DEFAULT_IDLE_WPM: u16 = 10;
const DEFAULT_FURY_WPM: u16 = 80;
const DEFAULT_IDLE_TICKS_PER_FRAME: u8 = 3;
const DEFAULT_TAP_HOLD_TICKS: u8 = 3;
const DEFAULT_TAP_IDLE_TICKS: u8 = 5;

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
/// - **Idle** (low WPM): subtle breathing animation, one frame per render.
/// - **Tap** (moderate WPM): each key press alternates left/right paw.
/// - **Fury** (high WPM): both paws smashing, one toggle per render.
///
/// Animation speed is controlled by `render_interval` in `keyboard.toml`.
/// Frame data from QMK firmware (GPL-2.0, keyboards/torn).
pub struct BongoCatRenderer {
    // config
    /// WPM threshold below which the cat idles.
    idle_wpm: u16,
    /// WPM threshold above which the cat enters fury mode.
    fury_wpm: u16,
    /// Number of renders before advancing to the next idle frame.
    idle_ticks_per_frame: u8,
    /// Number of renders the paw stays down after a tap.
    tap_hold_ticks: u8,
    /// Number of renders in PREP pose before falling back to idle animation.
    tap_idle_ticks: u8,
    // state
    state: State,
    tap_paw: bool,
    idle_frame: u8,
    idle_tick: u8,
    tap_hold: u8,
    tap_inactivity: u8,
}

impl Default for BongoCatRenderer {
    fn default() -> Self {
        Self {
            idle_wpm: DEFAULT_IDLE_WPM,
            fury_wpm: DEFAULT_FURY_WPM,
            idle_ticks_per_frame: DEFAULT_IDLE_TICKS_PER_FRAME,
            tap_hold_ticks: DEFAULT_TAP_HOLD_TICKS,
            tap_idle_ticks: DEFAULT_TAP_IDLE_TICKS,
            state: State::Idle,
            tap_paw: false,
            idle_frame: 0,
            idle_tick: 0,
            tap_hold: 0,
            tap_inactivity: 0,
        }
    }
}

impl BongoCatRenderer {
    pub fn with_idle_wpm(mut self, wpm: u16) -> Self {
        self.idle_wpm = wpm;
        self
    }

    pub fn with_fury_wpm(mut self, wpm: u16) -> Self {
        self.fury_wpm = wpm;
        self
    }

    /// Set how many renders must pass before the idle animation advances one frame.
    pub fn with_idle_ticks_per_frame(mut self, ticks: u8) -> Self {
        self.idle_ticks_per_frame = ticks;
        self
    }

    /// Set how many renders the paw stays in the TAP pose after a key press.
    pub fn with_tap_hold_ticks(mut self, ticks: u8) -> Self {
        self.tap_hold_ticks = ticks;
        self
    }

    /// Set how many renders the cat stays in PREP pose before returning to idle animation.
    pub fn with_tap_idle_ticks(mut self, ticks: u8) -> Self {
        self.tap_idle_ticks = ticks;
        self
    }
}

impl DisplayRenderer<BinaryColor> for BongoCatRenderer {
    fn render<D: DrawTarget<Color = BinaryColor>>(&mut self, ctx: &RenderContext, display: &mut D) {
        let new_press = ctx.key_press_latch;

        // WPM drives state transitions
        let new_state = if ctx.wpm >= self.fury_wpm {
            State::Fury
        } else if ctx.wpm >= self.idle_wpm {
            State::Tap
        } else {
            State::Idle
        };

        if new_state != self.state {
            self.state = new_state;
            self.idle_frame = 0;
            self.idle_tick = 0;
        }

        // Alternate paw on each key press (only in Tap/Idle — Fury manages tap_paw itself)
        if new_press && self.state != State::Fury {
            self.tap_paw = !self.tap_paw;
            self.tap_hold = self.tap_hold_ticks;
            self.tap_inactivity = 0;
        }

        display.clear(BinaryColor::Off).ok();

        let data: &[u8; frames::FRAME_SIZE] = match self.state {
            State::Idle => self.next_idle_frame(),
            State::Tap => {
                if self.tap_hold > 0 {
                    self.tap_hold -= 1;
                    self.tap_inactivity = 0;
                    &frames::TAP[self.tap_paw as usize]
                } else if self.tap_inactivity < self.tap_idle_ticks {
                    self.tap_inactivity += 1;
                    &frames::PREP
                } else {
                    self.next_idle_frame()
                }
            }
            State::Fury => {
                self.tap_paw = !self.tap_paw;
                if self.tap_paw { &frames::FURY } else { &frames::PREP }
            }
        };

        draw_page_format_frame(display, data, 128, 0, 0);

        // WPM overlay — bottom-right corner
        let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        let mut buf: heapless::String<8> = heapless::String::new();
        core::fmt::write(&mut buf, format_args!("{}", ctx.wpm)).ok();
        let x = 119 - buf.len() as i32 * 6;
        Text::new(&buf, Point::new(x, 31), style).draw(display).ok();
    }
}

impl BongoCatRenderer {
    fn next_idle_frame(&mut self) -> &'static [u8; frames::FRAME_SIZE] {
        self.idle_tick += 1;
        if self.idle_tick >= self.idle_ticks_per_frame {
            self.idle_tick = 0;
            self.idle_frame = (self.idle_frame + 1) % frames::IDLE.len() as u8;
        }
        &frames::IDLE[self.idle_frame as usize]
    }
}
