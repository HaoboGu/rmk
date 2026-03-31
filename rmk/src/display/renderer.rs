//! Display renderer trait and built-in renderers.
//!
//! Custom renderers implement [`DisplayRenderer`] to control what is drawn on
//! the OLED.  The built-in [`DefaultRenderer`] adapts automatically between
//! landscape and portrait layouts.

use core::fmt::Write as _;

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_5X8},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Line, PrimitiveStyle},
    text::Text,
};

use crate::event::BatteryStateEvent;

/// Snapshot of keyboard state passed to renderers on every redraw.
pub struct RenderContext {
    /// Current active layer index.
    pub layer: u8,
    /// Current words-per-minute estimate.
    pub wpm: u16,
    /// Whether Caps Lock is active.
    pub caps_lock: bool,
    /// Whether Num Lock is active.
    pub num_lock: bool,
    /// Current battery state.
    pub battery: BatteryStateEvent,
    /// Logical display width in pixels (already accounts for rotation).
    pub width: u32,
    /// Logical display height in pixels (already accounts for rotation).
    pub height: u32,
}

/// Trait for custom display renderers.
///
/// Generic over the pixel color type `C`, so it works with both monochrome
/// OLEDs (`BinaryColor`) and color LCDs (`Rgb565`, etc.).
///
/// # Example
///
/// ```rust,ignore
/// use rmk::display::{DisplayRenderer, RenderContext};
/// use embedded_graphics::{
///     prelude::*, pixelcolor::BinaryColor, text::Text,
///     mono_font::{ascii::FONT_6X10, MonoTextStyle},
/// };
///
/// struct MyRenderer;
///
/// impl DisplayRenderer<BinaryColor> for MyRenderer {
///     fn render<D: DrawTarget<Color = BinaryColor>>(
///         &mut self,
///         ctx: &RenderContext,
///         display: &mut D,
///     ) {
///         let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
///         let mut buf: heapless::String<16> = heapless::String::new();
///         core::fmt::write(&mut buf, format_args!("L{} W{}", ctx.layer, ctx.wpm)).ok();
///         Text::new(&buf, Point::new(0, 12), style).draw(display).ok();
///     }
/// }
/// ```
pub trait DisplayRenderer<C: PixelColor> {
    /// Draw the current keyboard state on the display.
    ///
    /// The display has already been cleared before this method is called.
    /// After it returns the caller flushes the display buffer.
    fn render<D: DrawTarget<Color = C>>(&mut self, ctx: &RenderContext, display: &mut D);
}

/// Built-in renderer that adapts between landscape and portrait layouts.
///
/// - **Landscape** (width >= height): two rows — layer + WPM on top,
///   indicators + battery on bottom, separated by a horizontal line.
/// - **Portrait** (height > width): four equal zones stacked vertically —
///   layer, WPM, indicators, battery — separated by horizontal lines.
pub struct DefaultRenderer;

impl DisplayRenderer<BinaryColor> for DefaultRenderer {
    fn render<D: DrawTarget<Color = BinaryColor>>(&mut self, ctx: &RenderContext, display: &mut D) {
        let w = ctx.width as i32;
        let h = ctx.height as i32;

        if w >= h {
            render_landscape(ctx, display, w, h);
        } else {
            render_portrait(ctx, display, w, h);
        }
    }
}

// ── Landscape layout ─────────────────────────────────────────────────────────
//
//  ┌──────────────────────────────────┐
//  │ Lyr:0                  WPM:045   │  ← row 1
//  ├──────────────────────────────────┤
//  │ CAP NUM                    85%   │  ← row 2
//  └──────────────────────────────────┘
fn render_landscape<D: DrawTarget<Color = BinaryColor>>(ctx: &RenderContext, display: &mut D, w: i32, h: i32) {
    const CW: i32 = 5; // FONT_5X8 character width
    let style = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
    let sep_style = PrimitiveStyle::with_stroke(BinaryColor::On, 1);

    let sep_y = h / 2;
    let row1_y = sep_y / 2 + 4;
    let row2_y = sep_y + sep_y / 2 + 4;

    // Horizontal separator
    Line::new(Point::new(0, sep_y), Point::new(w - 1, sep_y))
        .into_styled(sep_style)
        .draw(display)
        .ok();

    // Row 1: layer (left) + WPM (right)
    let mut lyr: heapless::String<8> = heapless::String::new();
    write!(lyr, "Lyr:{}", ctx.layer).ok();
    Text::new(&lyr, Point::new(2, row1_y), style).draw(display).ok();

    let mut wpm: heapless::String<8> = heapless::String::new();
    write!(wpm, "WPM:{:03}", ctx.wpm).ok();
    let wpm_x = (w - wpm.len() as i32 * CW - 2).max(0);
    Text::new(&wpm, Point::new(wpm_x, row1_y), style).draw(display).ok();

    // Row 2: indicators (left) + battery (right)
    let mut ind: heapless::String<8> = heapless::String::new();
    if ctx.caps_lock {
        write!(ind, "CAP").ok();
    }
    if ctx.num_lock {
        if !ind.is_empty() {
            ind.push(' ').ok();
        }
        write!(ind, "NUM").ok();
    }
    if !ind.is_empty() {
        Text::new(&ind, Point::new(2, row2_y), style).draw(display).ok();
    }

    let mut bat: heapless::String<5> = heapless::String::new();
    write_battery(&mut bat, ctx.battery);
    let bat_x = (w - bat.len() as i32 * CW - 2).max(0);
    Text::new(&bat, Point::new(bat_x, row2_y), style).draw(display).ok();
}

// ── Portrait layout ──────────────────────────────────────────────────────────
//
//  ┌──────┐
//  │ Lyr  │  zone 0: label + value (two lines, centred)
//  │  0   │
//  ├──────┤
//  │ WPM  │  zone 1: label + value
//  │ 045  │
//  ├──────┤
//  │ CAP  │  zone 2: one or two lines depending on active indicators
//  │ NUM  │
//  ├──────┤
//  │ 85%  │  zone 3: single centred line
//  └──────┘
fn render_portrait<D: DrawTarget<Color = BinaryColor>>(ctx: &RenderContext, display: &mut D, w: i32, h: i32) {
    const CW: i32 = 5;
    let style = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
    let sep_style = PrimitiveStyle::with_stroke(BinaryColor::On, 1);

    let zone = h / 4;

    // Horizontally centre a string of `len` characters.
    let cx = |len: usize| -> i32 { ((w - len as i32 * CW) / 2).max(0) };

    // Two-line zone:  8 (font) + 4 (gap) + 8 (font) = 20 px of content.
    let top_pad = (zone - 20) / 2;
    let dbl_y1 = |z: i32| z * zone + top_pad + 8; // first baseline
    let dbl_y2 = |z: i32| z * zone + top_pad + 20; // second baseline

    // Single-line zone: vertically centred.
    let sgl_y = |z: i32| z * zone + (zone - 8) / 2 + 8;

    // Helper: draw a horizontal separator at the bottom of zone `z`.
    let sep = |d: &mut D, z: i32| {
        Line::new(Point::new(2, (z + 1) * zone - 1), Point::new(w - 3, (z + 1) * zone - 1))
            .into_styled(sep_style)
            .draw(d)
            .ok();
    };

    // ── Zone 0 — Layer ───────────────────────────────────────────────────

    let mut lyr_val: heapless::String<4> = heapless::String::new();
    write!(lyr_val, "{}", ctx.layer).ok();

    Text::new("Lyr", Point::new(cx(3), dbl_y1(0)), style).draw(display).ok();
    Text::new(&lyr_val, Point::new(cx(lyr_val.len()), dbl_y2(0)), style)
        .draw(display)
        .ok();
    sep(display, 0);

    // ── Zone 1 — WPM ─────────────────────────────────────────────────────

    let mut wpm_val: heapless::String<4> = heapless::String::new();
    write!(wpm_val, "{:03}", ctx.wpm).ok();

    Text::new("WPM", Point::new(cx(3), dbl_y1(1)), style).draw(display).ok();
    Text::new(&wpm_val, Point::new(cx(wpm_val.len()), dbl_y2(1)), style)
        .draw(display)
        .ok();
    sep(display, 1);

    // ── Zone 2 — LED indicators ──────────────────────────────────────────

    match (ctx.caps_lock, ctx.num_lock) {
        (true, true) => {
            Text::new("CAP", Point::new(cx(3), dbl_y1(2)), style).draw(display).ok();
            Text::new("NUM", Point::new(cx(3), dbl_y2(2)), style).draw(display).ok();
        }
        (true, false) => {
            Text::new("CAP", Point::new(cx(3), sgl_y(2)), style).draw(display).ok();
        }
        (false, true) => {
            Text::new("NUM", Point::new(cx(3), sgl_y(2)), style).draw(display).ok();
        }
        (false, false) => {}
    }
    sep(display, 2);

    // ── Zone 3 — Battery ─────────────────────────────────────────────────

    let mut bat: heapless::String<5> = heapless::String::new();
    write_battery(&mut bat, ctx.battery);
    Text::new(&bat, Point::new(cx(bat.len()), sgl_y(3)), style)
        .draw(display)
        .ok();
}

/// Format a [`BatteryStateEvent`] into a short display string.
///
/// Writes one of `"---"`, `" 85%"`, `"CHG"`, or `"FUL"` into `buf`.
pub fn write_battery(buf: &mut heapless::String<5>, battery: BatteryStateEvent) {
    match battery {
        BatteryStateEvent::NotAvailable => write!(buf, "---").ok(),
        BatteryStateEvent::Normal(v) => write!(buf, "{v:3}%").ok(),
        BatteryStateEvent::Charging => write!(buf, "CHG").ok(),
        BatteryStateEvent::Charged => write!(buf, "FUL").ok(),
    };
}
