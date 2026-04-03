#![no_std]

mod bongo_cat;
mod frames;

pub use bongo_cat::BongoCatRenderer;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;

/// Draw a frame stored in SSD1306 page format onto an embedded-graphics display.
///
/// `cols` is the number of columns in the source frame (e.g. 128 for full-width,
/// 64 for the raw logo). The data must contain `cols * (pages)` bytes where
/// pages = ceil(height / 8), packed as `data[page * cols + col]`.
///
/// Each byte encodes 8 vertical pixels in one column of one page,
/// with bit 0 at the top of the 8-pixel strip.
pub(crate) fn draw_page_format_frame<D: DrawTarget<Color = BinaryColor>>(
    display: &mut D,
    data: &[u8],
    cols: usize,
    offset_x: i32,
    offset_y: i32,
) {
    let pages = data.len() / cols;

    for page in 0..pages {
        for col in 0..cols {
            let byte = data[page * cols + col];
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
