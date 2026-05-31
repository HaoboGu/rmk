//! Azoteq IQS9151 trackpad controller driver.
//!
//! This driver intentionally implements only the generic pointer path:
//! product-number verification, minimal runtime setup, coordinate-frame reads,
//! and relative X/Y movement published as `PointingEvent`.
//!
//! Gestures, virtual keys, scrolling, dynamic scaling, split custom transports,
//! and device-specific configuration images are deliberately out of scope.

use embassy_time::{Duration, Instant, Timer};
use embedded_hal_async::digital::Wait;
use embedded_hal_async::i2c::I2c;
use rmk_macro::input_device;

use crate::event::{Axis, AxisEvent, AxisValType, PointingEvent};
use crate::fmt::Debug;

/// Default 7-bit I2C address used by IQS9151.
pub const I2C_ADDR: u8 = 0x56;
/// Expected IQS9151 product number.
pub const PRODUCT_NUMBER: u16 = 0x09bc;

const ADDR_PRODUCT_NUMBER: u16 = 0x1000;
const ADDR_RELATIVE_X: u16 = 0x1014;
const ADDR_INFO_FLAGS: u16 = 0x1020;
const ADDR_TRACKPAD_FLAGS: u16 = 0x1022;
const ADDR_SYSTEM_CONTROL: u16 = 0x11bc;
const ADDR_CONFIG_SETTINGS: u16 = 0x11be;

const COORD_BLOCK_START: u16 = ADDR_RELATIVE_X;
const COORD_BLOCK_LENGTH: usize = 0x1c;

const INFO_SHOW_RESET: u16 = 1 << 7;
const TP_FINGER_COUNT_MASK: u16 = 0x000f;
const TP_MOVEMENT_DETECTED: u16 = 1 << 4;
const SYS_CTRL_ACK_RESET: u16 = 1 << 7;
const CFG_TP_TOUCH_EVENT_EN: u16 = 1 << 13;
const CFG_TP_EVENT_EN: u16 = 1 << 10;
const CFG_EVENT_MODE: u16 = 1 << 8;

#[input_device(publish = PointingEvent)]
pub struct Iqs9151<I, RDY>
where
    I: I2c,
    I::Error: Debug,
    RDY: Wait,
{
    /// The RMK pointing device id of this device (*not* the I2C bus address).
    pointing_device_id: u8,

    i2c: I,

    window_detection: WindowDetection<RDY>,

    initialized: bool,
}

/// Manner of detecting a communication-ready window.
pub enum WindowDetection<RDY> {
    /// Wait for a low state of the given GPIO connected to the active-low `RDY` pin.
    Rdy(RDY),

    /// Poll every `interval`; the device may clock-stretch if polled mid-cycle.
    Poll { last_poll: Instant, interval_ms: u16 },
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug)]
enum Error<I2cError> {
    I2c { tag: &'static str, inner: I2cError },
    InvalidProductNumber(u16),
    Pin,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct CoordinateFrame {
    relative_x: i16,
    relative_y: i16,
    info_flags: u16,
    trackpad_flags: u16,
}

impl CoordinateFrame {
    fn parse(block: &[u8; COORD_BLOCK_LENGTH]) -> Self {
        Self {
            relative_x: i16::from_le_bytes(unwrap!(block[0x00..0x02].try_into())),
            relative_y: i16::from_le_bytes(unwrap!(block[0x02..0x04].try_into())),
            info_flags: u16::from_le_bytes(unwrap!(
                block[(ADDR_INFO_FLAGS - COORD_BLOCK_START) as usize
                    ..(ADDR_INFO_FLAGS - COORD_BLOCK_START) as usize + 2]
                    .try_into()
            )),
            trackpad_flags: u16::from_le_bytes(unwrap!(
                block[(ADDR_TRACKPAD_FLAGS - COORD_BLOCK_START) as usize
                    ..(ADDR_TRACKPAD_FLAGS - COORD_BLOCK_START) as usize + 2]
                    .try_into()
            )),
        }
    }

    const fn finger_count(self) -> u8 {
        (self.trackpad_flags & TP_FINGER_COUNT_MASK) as u8
    }

    const fn movement_detected(self) -> bool {
        (self.trackpad_flags & TP_MOVEMENT_DETECTED) != 0
    }

    const fn show_reset(self) -> bool {
        (self.info_flags & INFO_SHOW_RESET) != 0
    }
}

impl<I: I2c, RDY> Iqs9151<I, RDY>
where
    I: I2c,
    I::Error: Debug,
    RDY: Wait,
{
    pub fn new(rmk_id: u8, i2c: I, rdy: Option<RDY>) -> Self {
        Self {
            i2c,
            window_detection: match rdy {
                None => WindowDetection::Poll {
                    last_poll: Instant::now(),
                    interval_ms: 10,
                },
                Some(rdy) => WindowDetection::Rdy(rdy),
            },
            initialized: false,
            pointing_device_id: rmk_id,
        }
    }

    async fn wait_ready(&mut self) -> Result<(), Error<I::Error>> {
        match self.window_detection {
            WindowDetection::Rdy(ref mut rdy) => rdy.wait_for_low().await.map_err(|_| Error::Pin),
            WindowDetection::Poll {
                ref mut last_poll,
                interval_ms,
            } => {
                Timer::at(last_poll.saturating_add(Duration::from_millis(u64::from(interval_ms)))).await;
                *last_poll = Instant::now();
                Ok(())
            }
        }
    }

    async fn read_u16(&mut self, tag: &'static str, register: u16) -> Result<u16, Error<I::Error>> {
        let mut bytes = [0u8; 2];
        self.read_block(tag, register, &mut bytes).await?;
        Ok(u16::from_le_bytes(bytes))
    }

    async fn write_u16(&mut self, tag: &'static str, register: u16, value: u16) -> Result<(), Error<I::Error>> {
        let register = register.to_le_bytes();
        let value = value.to_le_bytes();
        self.i2c
            .write(I2C_ADDR, &[register[0], register[1], value[0], value[1]])
            .await
            .map_err(|inner| Error::I2c { tag, inner })
    }

    async fn update_bits_u16(
        &mut self,
        tag: &'static str,
        register: u16,
        mask: u16,
        value: u16,
    ) -> Result<(), Error<I::Error>> {
        let current = self.read_u16(tag, register).await?;
        self.write_u16(tag, register, (current & !mask) | (value & mask)).await
    }

    async fn read_block(&mut self, tag: &'static str, register: u16, bytes: &mut [u8]) -> Result<(), Error<I::Error>> {
        self.i2c
            .write_read(I2C_ADDR, &register.to_le_bytes(), bytes)
            .await
            .map_err(|inner| Error::I2c { tag, inner })
    }

    async fn init(&mut self) -> Result<(), Error<I::Error>> {
        self.wait_ready().await?;
        let product_number = self.read_u16("read_product_number", ADDR_PRODUCT_NUMBER).await?;
        if product_number != PRODUCT_NUMBER {
            return Err(Error::InvalidProductNumber(product_number));
        }

        self.wait_ready().await?;
        let info_flags = self.read_u16("read_info_flags", ADDR_INFO_FLAGS).await?;
        if (info_flags & INFO_SHOW_RESET) != 0 {
            self.wait_ready().await?;
            self.update_bits_u16("ack_reset", ADDR_SYSTEM_CONTROL, SYS_CTRL_ACK_RESET, SYS_CTRL_ACK_RESET)
                .await?;
        }

        self.wait_ready().await?;
        let config_mask = CFG_TP_TOUCH_EVENT_EN | CFG_TP_EVENT_EN | CFG_EVENT_MODE;
        let config_value = match self.window_detection {
            WindowDetection::Rdy(_) => config_mask,
            WindowDetection::Poll { .. } => CFG_TP_TOUCH_EVENT_EN | CFG_TP_EVENT_EN,
        };
        self.update_bits_u16("configure_events", ADDR_CONFIG_SETTINGS, config_mask, config_value)
            .await?;

        self.initialized = true;
        info!("iqs9151 {}: initialized", self.pointing_device_id);
        Ok(())
    }

    async fn read_coordinate_frame(&mut self) -> Result<CoordinateFrame, Error<I::Error>> {
        self.wait_ready().await?;
        let mut block = [0u8; COORD_BLOCK_LENGTH];
        self.read_block("read_coordinate_frame", COORD_BLOCK_START, &mut block)
            .await?;
        Ok(CoordinateFrame::parse(&block))
    }

    async fn read_pointing_event(&mut self) -> PointingEvent {
        loop {
            if !self.initialized
                && let Err(e) = self.init().await
            {
                error!(
                    "iqs9151 {} initialization failed: {:?}; will retry in 1 second",
                    self.pointing_device_id, e,
                );
                Timer::after_secs(1).await;
                continue;
            }

            match self.read_coordinate_frame().await {
                Ok(frame) => {
                    if frame.show_reset() {
                        self.initialized = false;
                        continue;
                    }
                    debug!(
                        "iqs9151 {} frame: fingers={} movement={} dx={} dy={}",
                        self.pointing_device_id,
                        frame.finger_count(),
                        frame.movement_detected(),
                        frame.relative_x,
                        frame.relative_y,
                    );
                    if frame.relative_x != 0 || frame.relative_y != 0 {
                        return PointingEvent([
                            AxisEvent {
                                typ: AxisValType::Rel,
                                axis: Axis::X,
                                value: frame.relative_x,
                            },
                            AxisEvent {
                                typ: AxisValType::Rel,
                                axis: Axis::Y,
                                value: frame.relative_y,
                            },
                            AxisEvent {
                                typ: AxisValType::Rel,
                                axis: Axis::Z,
                                value: 0,
                            },
                        ]);
                    }
                    Timer::after_millis(1).await;
                }
                Err(e) => {
                    error!("iqs9151 {} failure: {:?}", self.pointing_device_id, e);
                    Timer::after_millis(5).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_frame_parses_relative_xy_and_flags() {
        let mut block = [0u8; COORD_BLOCK_LENGTH];
        block[0x00..0x02].copy_from_slice(&(-12_i16).to_le_bytes());
        block[0x02..0x04].copy_from_slice(&34_i16.to_le_bytes());
        block[0x0c..0x0e].copy_from_slice(&INFO_SHOW_RESET.to_le_bytes());
        block[0x0e..0x10].copy_from_slice(&(TP_MOVEMENT_DETECTED | 2).to_le_bytes());

        let frame = CoordinateFrame::parse(&block);

        assert_eq!(frame.relative_x, -12);
        assert_eq!(frame.relative_y, 34);
        assert!(frame.show_reset());
        assert!(frame.movement_detected());
        assert_eq!(frame.finger_count(), 2);
    }
}
