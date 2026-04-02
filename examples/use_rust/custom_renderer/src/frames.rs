// Bongo Cat animation frames for 128x32 OLED
// Original art: QMK firmware, keyboards/torn (GPL-2.0)
// Format: column-major page format — 4 pages × 128 columns
// Each byte encodes 8 vertical pixels in one column of one page.
//
// To draw with embedded-graphics, iterate pages (0..4) and columns (0..128):
//   pixel(col, page*8 + bit) = (frame[page*128 + col] >> bit) & 1

pub const FRAME_SIZE: usize = 512; // 128 * 32 / 8

// 5 idle frames — cat resting, subtle breathing animation
pub const IDLE: [[u8; FRAME_SIZE]; 5] = [
    include!("frames/idle_0.hex"),
    include!("frames/idle_1.hex"),
    include!("frames/idle_2.hex"),
    include!("frames/idle_3.hex"),
    include!("frames/idle_4.hex"),
];

// 1 prep frame — cat with both paws raised
pub const PREP: [u8; FRAME_SIZE] = include!("frames/prep_0.hex");

// 1 fury frame — both paws down (left half of tap0 + right half of tap1)
pub const FURY: [u8; FRAME_SIZE] = include!("frames/fury_0.hex");

// 2 tap frames — cat alternating paw strikes
pub const TAP: [[u8; FRAME_SIZE]; 2] = [include!("frames/tap_0.hex"), include!("frames/tap_1.hex")];
