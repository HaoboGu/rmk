//! 74HC595 Shift Register Matrix
//!
//! A keyboard matrix implementation that uses SPI-connected 74HC595 shift
//! registers as column drivers. Row pins are regular GPIO inputs.
//!
//! The 595 chain length is derived from `COL`: one 595 provides 8 outputs,
//! so two daisy-chained 595s handle up to 16 columns, etc.
//!
//! # Wiring
//!
//! ```text
//!   MCU                  74HC595 (xN)          Matrix
//!   ---------------------------------------------------
//!   SPI MOSI  ------->  SER (data in)
//!   SPI SCK   ------->  SRCLK (shift clock)
//!   CS / Latch ------>  RCLK (storage clock)    COL 0..N
//!   GPIO In   <-------------------------------- ROW 0..M
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use rmk::matrix::hc595_matrix::Hc595Matrix;
//!
//! let mut matrix = Hc595Matrix::<_, _, _, _, 2, 16>::new(
//!     spi_device,   // impl SpiDevice<u8>
//!     latch_pin,    // impl OutputPin
//!     row_pins,     // [impl InputPin; ROW]
//!     debouncer,
//! );
//! matrix.init().await;
//! ```

use embassy_time::{Duration, Timer};
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
use embedded_hal_async::spi::SpiDevice;

use crate::debounce::{DebounceState, DebouncerTrait};
use crate::event::{KeyboardEvent, publish_event_async};
use crate::input_device::{InputDevice, Runnable};
use crate::matrix::{KeyState, MatrixTrait};

const SR_MAX_BYTES: usize = 4;
const SR_CLEAR_SETTLE_US: u64 = 3;
const SR_COLUMN_SETTLE_US: u64 = 40;

pub struct Hc595Matrix<
    SPI: SpiDevice<u8>,
    LATCH: OutputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize = 0,
    const COL_OFFSET: usize = 0,
> {
    spi: SPI,
    latch: LATCH,
    row_pins: [In; ROW],
    debouncer: D,
    key_states: [[KeyState; ROW]; COL],
    scan_pos: (usize, usize),
    #[cfg(feature = "async_matrix")]
    rescan_needed: bool,
}

impl<
    SPI: SpiDevice<u8>,
    LATCH: OutputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
> Hc595Matrix<SPI, LATCH, In, D, ROW, COL, ROW_OFFSET, COL_OFFSET>
{
    const NUM_BYTES: usize = (COL + 7) / 8;

    pub fn new(spi: SPI, latch: LATCH, row_pins: [In; ROW], debouncer: D) -> Self {
        Self {
            spi,
            latch,
            row_pins,
            debouncer,
            key_states: [[KeyState::new(); ROW]; COL],
            scan_pos: (0, 0),
            #[cfg(feature = "async_matrix")]
            rescan_needed: false,
        }
    }

    pub async fn init(&mut self) {
        self.clear_columns().await;
    }

    #[inline(always)]
    fn io_delay() {
        for _ in 0..960 {
            core::hint::spin_loop();
        }
    }

    async fn latch(&mut self, data: &[u8]) {
        self.latch.set_low().ok();
        Self::io_delay();
        let _ = self.spi.write(data).await;
        Self::io_delay();
        self.latch.set_high().ok();
        Self::io_delay();
    }

    fn col_bitmask(col_idx: usize) -> [u8; SR_MAX_BYTES] {
        let mut buf = [0u8; SR_MAX_BYTES];
        let byte_pos = col_idx / 8;
        let bit_pos = col_idx % 8;
        buf[Self::NUM_BYTES - 1 - byte_pos] = 1 << bit_pos;
        buf
    }

    async fn clear_columns(&mut self) {
        let zeros = [0u8; SR_MAX_BYTES];
        self.latch(&zeros[..Self::NUM_BYTES]).await;
    }
}

impl<
    SPI: SpiDevice<u8>,
    LATCH: OutputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
> InputDevice for Hc595Matrix<SPI, LATCH, In, D, ROW, COL, ROW_OFFSET, COL_OFFSET>
{
    type Event = KeyboardEvent;

    async fn read_event(&mut self) -> Self::Event {
        loop {
            let (col_start, row_start) = self.scan_pos;

            for col_idx in col_start..COL {
                self.clear_columns().await;
                Timer::after(Duration::from_micros(SR_CLEAR_SETTLE_US)).await;

                let bitmask = Self::col_bitmask(col_idx);
                self.latch(&bitmask[..Self::NUM_BYTES]).await;
                Timer::after(Duration::from_micros(SR_COLUMN_SETTLE_US)).await;

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

            self.clear_columns().await;

            #[cfg(feature = "async_matrix")]
            {
                if !self.rescan_needed {
                    self.wait_for_key().await;
                }
                self.rescan_needed = false;
            }

            Timer::after(Duration::from_millis(1)).await;

            self.scan_pos = (0, 0);
        }
    }
}

impl<
    SPI: SpiDevice<u8>,
    LATCH: OutputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
> Runnable for Hc595Matrix<SPI, LATCH, In, D, ROW, COL, ROW_OFFSET, COL_OFFSET>
{
    async fn run(&mut self) -> ! {
        loop {
            let event = self.read_event().await;
            publish_event_async(event).await;
        }
    }
}

impl<
    SPI: SpiDevice<u8>,
    LATCH: OutputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
> MatrixTrait<ROW, COL> for Hc595Matrix<SPI, LATCH, In, D, ROW, COL, ROW_OFFSET, COL_OFFSET>
{
    #[cfg(feature = "async_matrix")]
    async fn wait_for_key(&mut self) {
        self.clear_columns().await;
        Timer::after(Duration::from_millis(1)).await;
    }
}
