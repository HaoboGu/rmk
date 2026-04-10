//! 74HC595 Shift Register Matrix
//!
//! A keyboard matrix implementation that uses SPI-connected 74HC595 shift
//! registers as column drivers.  Row pins are regular GPIO inputs.
//!
//! The 595 chain length is derived from `COL`: one 595 provides 8 outputs,
//! so two daisy-chained 595s handle up to 16 columns, etc.
//!
//! # Wiring
//!
//! ```text
//!   MCU                  74HC595 (×N)          Matrix
//!   ───────────────────────────────────────────────────
//!   SPI MOSI  ──────►  SER (data in)
//!   SPI SCK   ──────►  SRCLK (shift clock)
//!   CS / Latch ─────►  RCLK (storage clock)    COL 0..N
//!   GPIO In   ◄──────────────────────────────── ROW 0..M
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use rmk::shift_register::ShiftRegisterMatrix;
//!
//! let mut matrix = ShiftRegisterMatrix::<_, _, _, _, 2, 16>::new(
//!     spi,          // impl SpiBus
//!     cs_pin,       // impl OutputPin (active-high latch)
//!     row_pins,     // [impl InputPin; ROW]
//!     debouncer,
//! );
//! ```

use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::spi::SpiBus;
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;

use crate::debounce::{DebounceState, DebouncerTrait};
use crate::event::{KeyboardEvent, publish_event_async};
use crate::input_device::{InputDevice, Runnable};
use crate::matrix::{KeyState, MatrixTrait};

/// Maximum number of bytes in the shift register chain (supports up to 32 columns).
const SR_MAX_BYTES: usize = 4;

/// Keyboard matrix driven by 74HC595 shift registers.
///
/// Column scanning is performed over SPI: for each column a bitmask with a
/// single bit set is shifted out and latched, then the row GPIO pins are read.
///
/// `SPI` must implement `embedded_hal_async::spi::SpiBus`.
/// `CS` is the latch / RCLK pin (directly driven, not via SPI CS).
pub struct ShiftRegisterMatrix<
    SPI: SpiBus,
    CS: OutputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize = 0,
    const COL_OFFSET: usize = 0,
> {
    spi: SPI,
    cs: CS,
    row_pins: [In; ROW],
    debouncer: D,
    key_states: [[KeyState; ROW]; COL],
    scan_pos: (usize, usize),
    #[cfg(feature = "async_matrix")]
    rescan_needed: bool,
}

impl<
    SPI: SpiBus,
    CS: OutputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
> ShiftRegisterMatrix<SPI, CS, In, D, ROW, COL, ROW_OFFSET, COL_OFFSET>
{
    /// Number of bytes in the shift-register chain.
    const NUM_BYTES: usize = (COL + 7) / 8;

    /// Create a new shift-register matrix.
    ///
    /// * `spi`      – SPI bus connected to the 595 chain (MOSI + SCK)
    /// * `cs`       – Latch / RCLK pin.  A low→high transition latches data.
    /// * `row_pins` – GPIO input pins for each matrix row (active-high with pull-down)
    /// * `debouncer`– Debouncer instance
    pub fn new(spi: SPI, cs: CS, row_pins: [In; ROW], debouncer: D) -> Self {
        Self {
            spi,
            cs,
            row_pins,
            debouncer,
            key_states: [[KeyState::new(); ROW]; COL],
            scan_pos: (0, 0),
            #[cfg(feature = "async_matrix")]
            rescan_needed: false,
        }
    }

    /// Initialize the shift register by clearing all outputs.
    /// Call this once before starting the scan loop.
    pub async fn init(&mut self) {
        self.clear_columns().await;
    }

    /// Small busy-wait delay (~5µs) for SPI signal settling.
    #[inline(always)]
    fn io_delay() {
        for _ in 0..160 {
            core::hint::spin_loop();
        }
    }

    /// Shift out `data` and latch the 595 outputs.
    ///
    /// The latch pin (RCLK) is pulsed low→high after the SPI transfer,
    /// causing the shift-register contents to appear on the output pins.
    async fn latch(&mut self, data: &[u8]) {
        self.cs.set_low().ok();
        Self::io_delay();
        let _ = self.spi.write(data).await;
        Self::io_delay();
        self.cs.set_high().ok();
        Self::io_delay();
    }

    /// Build a column bitmask: only bit `col_idx` is set.
    ///
    /// Byte order is MSB-first so that the first byte shifted out ends up in
    /// the last 595 in the daisy chain (matching the standard wiring).
    fn col_bitmask(col_idx: usize) -> [u8; SR_MAX_BYTES] {
        let mut buf = [0u8; SR_MAX_BYTES];
        let byte_pos = col_idx / 8;
        let bit_pos = col_idx % 8;
        // MSB-first: the first byte in the buffer goes to the farthest 595
        buf[Self::NUM_BYTES - 1 - byte_pos] = 1 << bit_pos;
        buf
    }

    /// Clear all shift-register outputs.
    async fn clear_columns(&mut self) {
        let zeros = [0u8; SR_MAX_BYTES];
        self.latch(&zeros[..Self::NUM_BYTES]).await;
    }
}

impl<
    SPI: SpiBus,
    CS: OutputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
> InputDevice for ShiftRegisterMatrix<SPI, CS, In, D, ROW, COL, ROW_OFFSET, COL_OFFSET>
{
    type Event = KeyboardEvent;

    async fn read_event(&mut self) -> Self::Event {
        loop {
            let (col_start, row_start) = self.scan_pos;

            for col_idx in col_start..COL {
                // Drive this column high via the shift register
                let bitmask = Self::col_bitmask(col_idx);
                self.latch(&bitmask[..Self::NUM_BYTES]).await;

                let r_start = if col_idx == col_start { row_start } else { 0 };

                for row_idx in r_start..ROW {
                    let pin_high = self.row_pins[row_idx].is_high().ok().unwrap_or(false);

                    let debounce_state = self.debouncer.detect_change_with_debounce(
                        row_idx,
                        col_idx,
                        pin_high,
                        &self.key_states[col_idx][row_idx],
                    );

                    if let DebounceState::Debounced = debounce_state {
                        self.key_states[col_idx][row_idx].toggle_pressed();
                        self.scan_pos = (col_idx, row_idx);
                        #[cfg(feature = "async_matrix")]
                        {
                            self.rescan_needed = true;
                        }
                        // Clear outputs before returning
                        self.clear_columns().await;
                        return KeyboardEvent::key(
                            (row_idx + ROW_OFFSET) as u8,
                            (col_idx + COL_OFFSET) as u8,
                            self.key_states[col_idx][row_idx].pressed,
                        );
                    }

                    #[cfg(feature = "async_matrix")]
                    if self.key_states[col_idx][row_idx].pressed {
                        self.rescan_needed = true;
                    }
                }
            }

            // Full scan complete – clear outputs
            self.clear_columns().await;

            #[cfg(feature = "async_matrix")]
            {
                if !self.rescan_needed {
                    self.wait_for_key().await;
                }
                self.rescan_needed = false;
            }

            self.scan_pos = (0, 0);
        }
    }
}

impl<
    SPI: SpiBus,
    CS: OutputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
> Runnable for ShiftRegisterMatrix<SPI, CS, In, D, ROW, COL, ROW_OFFSET, COL_OFFSET>
{
    async fn run(&mut self) -> ! {
        loop {
            let event = self.read_event().await;
            publish_event_async(event).await;
        }
    }
}

impl<
    SPI: SpiBus,
    CS: OutputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
> MatrixTrait<ROW, COL> for ShiftRegisterMatrix<SPI, CS, In, D, ROW, COL, ROW_OFFSET, COL_OFFSET>
{
    #[cfg(feature = "async_matrix")]
    async fn wait_for_key(&mut self) {
        // Drive all columns high so any key press triggers a row pin change
        let all_high = {
            let mut buf = [0xFFu8; SR_MAX_BYTES];
            // Mask out unused bits in the last byte
            let used_bits = COL % 8;
            if used_bits != 0 {
                buf[0] = (1u8 << used_bits) - 1;
            }
            buf
        };
        self.latch(&all_high[..Self::NUM_BYTES]).await;

        // Wait for any row pin to go high (scoped to drop futs before clear_columns)
        {
            let mut futs: heapless::Vec<_, ROW> = self
                .row_pins
                .iter_mut()
                .map(|pin| pin.wait_for_high())
                .collect();
            let _ =
                embassy_futures::select::select_slice(core::pin::pin!(futs.as_mut_slice())).await;
        }

        // Clear outputs
        self.clear_columns().await;
    }
}
