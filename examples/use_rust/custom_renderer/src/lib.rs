#![no_std]

use core::fmt::Write;

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_5X8},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Line, PrimitiveStyle},
    text::{Alignment, Text, TextStyleBuilder},
};
use rmk::display::{DisplayRenderer, RenderContext, write_battery};

/// A custom renderer optimized for small vertical OLED displays (e.g. 128x32 portrait).
pub struct BigLayerRenderer;

impl Default for BigLayerRenderer {
    fn default() -> Self {
        Self
    }
}

impl DisplayRenderer<BinaryColor> for BigLayerRenderer {
    fn render<D: DrawTarget<Color = BinaryColor>>(&mut self, ctx: &RenderContext, display: &mut D) {
        display.clear(BinaryColor::Off).ok();

        let w = ctx.width as i32;
        let h = ctx.height as i32;
        let style = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
        let centered = TextStyleBuilder::new().alignment(Alignment::Center).build();
        let sep = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
        let cx = w / 2;

        // Divide into 4 zones
        let zone = h / 4;

        // Baseline: vertically center a single 8px-tall line in each zone
        let y = |z: i32| -> i32 { z * zone + (zone + 8) / 2 };

        // ── Zone 0: Layer ──
        let mut buf: heapless::String<8> = heapless::String::new();
        write!(buf, "L: {}", ctx.layer).ok();
        Text::with_text_style(&buf, Point::new(cx, y(0)), style, centered)
            .draw(display)
            .ok();

        Line::new(Point::new(2, zone), Point::new(w - 3, zone))
            .into_styled(sep)
            .draw(display)
            .ok();

        // ── Zone 1: WPM ──
        buf.clear();
        write!(buf, "{:03}", ctx.wpm).ok();
        Text::with_text_style(&buf, Point::new(cx, y(1)), style, centered)
            .draw(display)
            .ok();

        Line::new(Point::new(2, zone * 2), Point::new(w - 3, zone * 2))
            .into_styled(sep)
            .draw(display)
            .ok();

        // ── Zone 2: Indicators ──
        let has_cap = ctx.caps_lock;
        let has_num = ctx.num_lock;

        if has_cap && has_num {
            // Two lines, stacked
            let top = 2 * zone + (zone - 18) / 2 + 8;
            Text::with_text_style("CAP", Point::new(cx, top), style, centered)
                .draw(display)
                .ok();
            Text::with_text_style("NUM", Point::new(cx, top + 10), style, centered)
                .draw(display)
                .ok();
        } else if has_cap {
            Text::with_text_style("CAP", Point::new(cx, y(2)), style, centered)
                .draw(display)
                .ok();
        } else if has_num {
            Text::with_text_style("NUM", Point::new(cx, y(2)), style, centered)
                .draw(display)
                .ok();
        }

        Line::new(Point::new(2, zone * 3), Point::new(w - 3, zone * 3))
            .into_styled(sep)
            .draw(display)
            .ok();

        // ── Zone 3: Battery ──
        let mut bat: heapless::String<5> = heapless::String::new();
        write_battery(&mut bat, ctx.battery);
        Text::with_text_style(&bat, Point::new(cx, y(3)), style, centered)
            .draw(display)
            .ok();
    }
}
