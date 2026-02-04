//! PMW3610 Low-Power Mouse Sensor Driver
//!
//! Ported from the Zephyr driver implementation:
//! https://github.com/zephyrproject-rtos/zephyr/blob/d31c6e95033fd6b3763389edba6a655245ae1328/drivers/input/input_pmw3610.c

use core::cell::RefCell;

use embassy_time::{Duration, Instant, Timer};
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::digital::Wait;
use embedded_hal_async::spi::SpiBus;
use futures::future::pending;
use rmk_macro::{input_device, input_processor};
use usbd_hid::descriptor::MouseReport;

pub use crate::driver::bitbang_spi::{BitBangError, BitBangSpiBus};
use crate::event::{Axis, AxisEvent, AxisValType, PointingEvent};
use crate::hid::Report;
use crate::input_device::InputProcessor;
use crate::keymap::KeyMap;

// ============================================================================
// Page 0 registers
// ============================================================================
const PMW3610_PROD_ID: u8 = 0x00;
const PMW3610_MOTION: u8 = 0x02;
const PMW3610_DELTA_XY_H: u8 = 0x05;
const PMW3610_PERFORMANCE: u8 = 0x11;
const PMW3610_BURST_READ: u8 = 0x12;
const PMW3610_RUN_DOWNSHIFT: u8 = 0x1b;
const PMW3610_REST1_RATE: u8 = 0x1c;
const PMW3610_REST1_DOWNSHIFT: u8 = 0x1d;
const PMW3610_OBSERVATION1: u8 = 0x2d;
const PMW3610_SMART_MODE: u8 = 0x32;
const PMW3610_POWER_UP_RESET: u8 = 0x3a;
const PMW3610_SPI_CLK_ON_REQ: u8 = 0x41;
const PMW3610_SPI_PAGE0: u8 = 0x7f;

// ============================================================================
// Page 1 registers
// ============================================================================
const PMW3610_RES_STEP: u8 = 0x05;
const PMW3610_SPI_PAGE1: u8 = 0x7f;

// ============================================================================
// Burst register offsets
// ============================================================================
const BURST_MOTION: usize = 0;
const BURST_DELTA_X_L: usize = 1;
const BURST_DELTA_Y_L: usize = 2;
const BURST_DELTA_XY_H: usize = 3;
const BURST_DELTA_X_H: usize = 4;
const BURST_SHUTTER_HI: usize = 5;
const BURST_SHUTTER_LO: usize = 6;

const BURST_DATA_LEN_NORMAL: usize = BURST_DELTA_XY_H + 1;
const BURST_DATA_LEN_SMART: usize = BURST_SHUTTER_LO + 1;

// ============================================================================
// Init sequence values
// ============================================================================
const OBSERVATION1_INIT_MASK: u8 = 0x0f;
const PERFORMANCE_INIT: u8 = 0x0d;
const RUN_DOWNSHIFT_INIT: u8 = 0x04;
const REST1_RATE_INIT: u8 = 0x04;
const REST1_DOWNSHIFT_INIT: u8 = 0x0f;

// ============================================================================
// Constants
// ============================================================================
const PRODUCT_ID_PMW3610: u8 = 0x3e;
const SPI_WRITE: u8 = 0x80;
const MOTION_STATUS_MOTION: u8 = 0x80;
const SPI_CLOCK_ON_REQ_ON: u8 = 0xba;
const SPI_CLOCK_ON_REQ_OFF: u8 = 0xb5;
const RES_STEP_SWAP_XY_BIT: u8 = 7;
const RES_STEP_INV_X_BIT: u8 = 6;
const RES_STEP_INV_Y_BIT: u8 = 5;
const RES_STEP_RES_MASK: u8 = 0x1f;
const PERFORMANCE_FMODE_MASK: u8 = 0x0f << 4;
const PERFORMANCE_FMODE_NORMAL: u8 = 0x00 << 4;
const PERFORMANCE_FMODE_FORCE_AWAKE: u8 = 0x0f << 4;
const POWER_UP_RESET_VAL: u8 = 0x5a;
const SPI_PAGE0_1: u8 = 0xff;
const SPI_PAGE1_0: u8 = 0x00;
const SHUTTER_SMART_THRESHOLD: u16 = 45;
const SMART_MODE_ENABLE: u8 = 0x00;
const SMART_MODE_DISABLE: u8 = 0x80;

const PMW3610_DATA_SIZE_BITS: usize = 12;

// Timing constants
const RESET_DELAY_MS: u64 = 10;
const INIT_OBSERVATION_DELAY_MS: u64 = 10;
const CLOCK_ON_DELAY_US: u64 = 300;

// SPI timing constants (from PMW3610 datasheet)
const T_NCS_SCLK_US: u64 = 1;
const T_SRAD_US: u64 = 5;
const T_SRX_US: u64 = 2;
const T_SWX_US: u64 = 35;
const T_SCLK_NCS_WR_US: u64 = 20;
const T_BEXIT_US: u64 = 2;

// Resolution constants
const RES_STEP: u16 = 200;
const RES_MIN: u16 = 200;
const RES_MAX: u16 = 3200;

/// PMW3610 configuration
#[derive(Clone)]
pub struct Pmw3610Config {
    /// CPI resolution (200-3200, step 200). Set to -1 to use default.
    pub res_cpi: i16,
    /// Invert X axis
    pub invert_x: bool,
    /// Invert Y axis
    pub invert_y: bool,
    /// Swap X and Y axes
    pub swap_xy: bool,
    /// Force awake mode (disable power saving)
    pub force_awake: bool,
    /// Enable smart mode for better tracking on shiny surfaces
    pub smart_mode: bool,
}

impl Default for Pmw3610Config {
    fn default() -> Self {
        Self {
            res_cpi: -1,
            invert_x: false,
            invert_y: false,
            swap_xy: false,
            force_awake: false,
            smart_mode: false,
        }
    }
}

/// PMW3610 error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Pmw3610Error {
    /// SPI communication error
    Spi,
    /// Invalid product ID detected
    InvalidProductId(u8),
    /// Initialization failed
    InitFailed,
    /// Invalid CPI value
    InvalidCpi,
}

/// Motion data from the sensor
#[derive(Debug, Clone, Copy, Default)]
pub struct MotionData {
    pub dx: i16,
    pub dy: i16,
}

/// PMW3610 driver using embedded-hal SPI traits
pub struct Pmw3610<SPI: SpiBus, CS: OutputPin, MOTION: InputPin + Wait> {
    spi: SPI,
    cs: CS,
    motion_gpio: Option<MOTION>,
    config: Pmw3610Config,
    smart_flag: bool,
}

impl<SPI: SpiBus, CS: OutputPin, MOTION: InputPin + Wait> Pmw3610<SPI, CS, MOTION> {
    /// Create a new PMW3610 driver instance
    pub fn new(spi: SPI, cs: CS, motion_gpio: Option<MOTION>, config: Pmw3610Config) -> Self {
        Self {
            spi,
            cs,
            motion_gpio,
            config,
            smart_flag: false,
        }
    }

    /// Check if motion is pending (motion GPIO is active low)
    pub fn motion_pending(&mut self) -> bool {
        match &mut self.motion_gpio {
            Some(gpio) => gpio.is_low().unwrap_or(true),
            None => true,
        }
    }

    #[inline(always)]
    fn short_delay() {
        for _ in 0..64 {
            core::hint::spin_loop();
        }
    }

    async fn read_reg(&mut self, addr: u8) -> Result<u8, Pmw3610Error> {
        let _ = self.cs.set_low();
        Timer::after(Duration::from_micros(T_NCS_SCLK_US)).await;

        self.spi.write(&[addr & 0x7f]).await.map_err(|_| Pmw3610Error::Spi)?;

        Timer::after(Duration::from_micros(T_SRAD_US)).await;

        let mut value = [0u8];
        self.spi.read(&mut value).await.map_err(|_| Pmw3610Error::Spi)?;

        Self::short_delay();
        let _ = self.cs.set_high();

        Timer::after(Duration::from_micros(T_SRX_US)).await;

        Ok(value[0])
    }

    async fn read_burst(&mut self, addr: u8, data: &mut [u8]) -> Result<(), Pmw3610Error> {
        let _ = self.cs.set_low();
        Timer::after(Duration::from_micros(T_NCS_SCLK_US)).await;

        self.spi.write(&[addr & 0x7f]).await.map_err(|_| Pmw3610Error::Spi)?;

        Timer::after(Duration::from_micros(T_SRAD_US)).await;

        self.spi.read(data).await.map_err(|_| Pmw3610Error::Spi)?;

        Self::short_delay();
        let _ = self.cs.set_high();

        Timer::after(Duration::from_micros(T_BEXIT_US)).await;

        Ok(())
    }

    async fn write_reg(&mut self, addr: u8, value: u8) -> Result<(), Pmw3610Error> {
        let _ = self.cs.set_low();
        Timer::after(Duration::from_micros(T_NCS_SCLK_US)).await;

        self.spi
            .write(&[addr | SPI_WRITE, value])
            .await
            .map_err(|_| Pmw3610Error::Spi)?;

        Timer::after(Duration::from_micros(T_SCLK_NCS_WR_US)).await;
        let _ = self.cs.set_high();

        Timer::after(Duration::from_micros(T_SWX_US)).await;

        Ok(())
    }

    async fn spi_clk_on(&mut self) -> Result<(), Pmw3610Error> {
        self.write_reg(PMW3610_SPI_CLK_ON_REQ, SPI_CLOCK_ON_REQ_ON).await?;
        Timer::after(Duration::from_micros(CLOCK_ON_DELAY_US)).await;
        Ok(())
    }

    async fn spi_clk_off(&mut self) -> Result<(), Pmw3610Error> {
        self.write_reg(PMW3610_SPI_CLK_ON_REQ, SPI_CLOCK_ON_REQ_OFF).await
    }

    /// Set sensor resolution in CPI (200-3200, step 200)
    pub async fn set_resolution(&mut self, cpi: u16) -> Result<(), Pmw3610Error> {
        if !(RES_MIN..=RES_MAX).contains(&cpi) {
            return Err(Pmw3610Error::InvalidCpi);
        }

        self.spi_clk_on().await?;

        self.write_reg(PMW3610_SPI_PAGE0, SPI_PAGE0_1).await?;

        let mut val = self.read_reg(PMW3610_RES_STEP).await?;
        val &= !RES_STEP_RES_MASK;
        val |= (cpi / RES_STEP) as u8;

        self.write_reg(PMW3610_RES_STEP, val).await?;
        self.write_reg(PMW3610_SPI_PAGE1, SPI_PAGE1_0).await?;

        self.spi_clk_off().await?;

        debug!("PMW3610: Resolution set to {} CPI", cpi);
        Ok(())
    }

    /// Set force awake mode
    pub async fn force_awake(&mut self, enable: bool) -> Result<(), Pmw3610Error> {
        let mut val = self.read_reg(PMW3610_PERFORMANCE).await?;
        val &= !PERFORMANCE_FMODE_MASK;
        if enable {
            val |= PERFORMANCE_FMODE_FORCE_AWAKE;
        } else {
            val |= PERFORMANCE_FMODE_NORMAL;
        }

        self.spi_clk_on().await?;
        self.write_reg(PMW3610_PERFORMANCE, val).await?;
        self.spi_clk_off().await?;

        Ok(())
    }

    async fn configure(&mut self) -> Result<(), Pmw3610Error> {
        self.write_reg(PMW3610_POWER_UP_RESET, POWER_UP_RESET_VAL).await?;
        Timer::after(Duration::from_millis(RESET_DELAY_MS)).await;

        let val = self.read_reg(PMW3610_PROD_ID).await?;
        if val != PRODUCT_ID_PMW3610 {
            error!("Invalid product id: {:#02x}", val);
            return Err(Pmw3610Error::InvalidProductId(val));
        }
        info!("PMW3610 detected, product ID: {:#02x}", val);

        self.spi_clk_on().await?;

        self.write_reg(PMW3610_OBSERVATION1, 0).await?;
        Timer::after(Duration::from_millis(INIT_OBSERVATION_DELAY_MS)).await;

        let val = self.read_reg(PMW3610_OBSERVATION1).await?;
        if (val & OBSERVATION1_INIT_MASK) != OBSERVATION1_INIT_MASK {
            error!("Unexpected OBSERVATION1 value: {:#02x}", val);
            return Err(Pmw3610Error::InitFailed);
        }

        for reg in PMW3610_MOTION..=PMW3610_DELTA_XY_H {
            self.read_reg(reg).await?;
        }

        self.write_reg(PMW3610_PERFORMANCE, PERFORMANCE_INIT).await?;
        self.write_reg(PMW3610_RUN_DOWNSHIFT, RUN_DOWNSHIFT_INIT).await?;
        self.write_reg(PMW3610_REST1_RATE, REST1_RATE_INIT).await?;
        self.write_reg(PMW3610_REST1_DOWNSHIFT, REST1_DOWNSHIFT_INIT).await?;

        self.write_reg(PMW3610_SPI_PAGE0, SPI_PAGE0_1).await?;

        let mut res_step_val = self.read_reg(PMW3610_RES_STEP).await?;

        if self.config.swap_xy {
            res_step_val |= 1 << RES_STEP_SWAP_XY_BIT;
        } else {
            res_step_val &= !(1 << RES_STEP_SWAP_XY_BIT);
        }

        if self.config.invert_x {
            res_step_val |= 1 << RES_STEP_INV_X_BIT;
        } else {
            res_step_val &= !(1 << RES_STEP_INV_X_BIT);
        }

        if self.config.invert_y {
            res_step_val |= 1 << RES_STEP_INV_Y_BIT;
        } else {
            res_step_val &= !(1 << RES_STEP_INV_Y_BIT);
        }

        self.write_reg(PMW3610_RES_STEP, res_step_val).await?;
        self.write_reg(PMW3610_SPI_PAGE1, SPI_PAGE1_0).await?;

        self.spi_clk_off().await?;

        if self.config.res_cpi > 0 {
            self.set_resolution(self.config.res_cpi as u16).await?;
        }

        self.force_awake(self.config.force_awake).await?;

        info!("PMW3610 initialized successfully");
        Ok(())
    }

    /// Initialize the sensor (public API)
    pub async fn init(&mut self) -> Result<(), Pmw3610Error> {
        let _ = self.cs.set_high();
        Timer::after(Duration::from_millis(1)).await;

        self.configure().await
    }

    /// Read motion data from the sensor
    pub async fn read_motion(&mut self) -> Result<MotionData, Pmw3610Error> {
        let burst_data_len = if self.config.smart_mode {
            BURST_DATA_LEN_SMART
        } else {
            BURST_DATA_LEN_NORMAL
        };

        let mut burst_data = [0u8; BURST_DATA_LEN_SMART];
        self.read_burst(PMW3610_BURST_READ, &mut burst_data[..burst_data_len])
            .await?;

        if (burst_data[BURST_MOTION] & MOTION_STATUS_MOTION) == 0x00 {
            return Ok(MotionData::default());
        }

        let x = ((burst_data[BURST_DELTA_XY_H] as u16) << 4) & 0xf00 | (burst_data[BURST_DELTA_X_L] as u16);
        let y = ((burst_data[BURST_DELTA_XY_H] as u16) << 8) & 0xf00 | (burst_data[BURST_DELTA_Y_L] as u16);

        let dx = Self::sign_extend(x, PMW3610_DATA_SIZE_BITS - 1);
        let dy = Self::sign_extend(y, PMW3610_DATA_SIZE_BITS - 1);

        if self.config.smart_mode {
            let shutter_val = ((burst_data[BURST_SHUTTER_HI] as u16) << 8) | (burst_data[BURST_SHUTTER_LO] as u16);

            if self.smart_flag && shutter_val < SHUTTER_SMART_THRESHOLD {
                self.spi_clk_on().await?;
                self.write_reg(PMW3610_SMART_MODE, SMART_MODE_ENABLE).await?;
                self.spi_clk_off().await?;
                self.smart_flag = false;
            } else if !self.smart_flag && shutter_val > SHUTTER_SMART_THRESHOLD {
                self.spi_clk_on().await?;
                self.write_reg(PMW3610_SMART_MODE, SMART_MODE_DISABLE).await?;
                self.spi_clk_off().await?;
                self.smart_flag = true;
            }
        }

        Ok(MotionData { dx, dy })
    }

    fn sign_extend(value: u16, bits: usize) -> i16 {
        let sign_bit = 1 << bits;
        if value & sign_bit != 0 {
            (value | !((1 << (bits + 1)) - 1)) as i16
        } else {
            value as i16
        }
    }
}

/// Initialization state for the device
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InitState {
    Pending,
    Initializing(u8),
    Ready,
    Failed,
}

/// PMW3610 as an InputDevice for RMK
///
/// This device returns `Event::Joystick` events with relative X/Y movement.
#[input_device(publish = PointingEvent)]
pub struct Pmw3610Device<SPI: SpiBus, CS: OutputPin, MOTION: InputPin + Wait> {
    sensor: Pmw3610<SPI, CS, MOTION>,
    init_state: InitState,
    poll_interval: Duration,
    report_interval: Duration,
    last_poll: Instant,
    last_report: Instant,
    accumulated_x: i32,
    accumulated_y: i32,
}

impl<SPI: SpiBus, CS: OutputPin, MOTION: InputPin + Wait> Pmw3610Device<SPI, CS, MOTION> {
    const MAX_INIT_RETRIES: u8 = 3;
    const DEFAULT_POLL_INTERVAL_US: u64 = 500;
    const DEFAULT_REPORT_HZ: u16 = 125;

    /// Create a new PMW3610 device for RMK
    pub fn new(spi: SPI, cs: CS, motion_gpio: Option<MOTION>, config: Pmw3610Config) -> Self {
        Self::with_poll_interval_and_report_hz(
            spi,
            cs,
            motion_gpio,
            config,
            Self::DEFAULT_POLL_INTERVAL_US,
            Self::DEFAULT_REPORT_HZ,
        )
    }

    /// Create a new PMW3610 device with custom report rate (Hz)
    pub fn with_report_hz(
        spi: SPI,
        cs: CS,
        motion_gpio: Option<MOTION>,
        config: Pmw3610Config,
        report_hz: u16,
    ) -> Self {
        Self::with_poll_interval_and_report_hz(spi, cs, motion_gpio, config, Self::DEFAULT_POLL_INTERVAL_US, report_hz)
    }

    /// Create a new PMW3610 device with custom poll interval
    pub fn with_poll_interval(
        spi: SPI,
        cs: CS,
        motion_gpio: Option<MOTION>,
        config: Pmw3610Config,
        poll_interval_us: u64,
    ) -> Self {
        Self::with_poll_interval_and_report_hz(spi, cs, motion_gpio, config, poll_interval_us, Self::DEFAULT_REPORT_HZ)
    }

    /// Create a new PMW3610 device with custom poll interval and report rate
    pub fn with_poll_interval_and_report_hz(
        spi: SPI,
        cs: CS,
        motion_gpio: Option<MOTION>,
        config: Pmw3610Config,
        poll_interval_us: u64,
        report_hz: u16,
    ) -> Self {
        let report_interval = Duration::from_hz(report_hz as u64);

        // Polling should be more frequent than reporting
        let poll_interval = Duration::from_micros(poll_interval_us).min(report_interval);

        Self {
            sensor: Pmw3610::new(spi, cs, motion_gpio, config),
            init_state: InitState::Pending,
            poll_interval,
            report_interval,
            last_poll: Instant::MIN,
            last_report: Instant::MIN,
            accumulated_x: 0,
            accumulated_y: 0,
        }
    }

    async fn poll_once(&mut self) {
        if self.init_state != InitState::Ready && !self.try_init().await {
            return;
        }

        if !self.sensor.motion_pending() {
            return;
        }

        match self.sensor.read_motion().await {
            Ok(motion) => {
                self.accumulated_x = self.accumulated_x.saturating_add(motion.dx as i32);
                self.accumulated_y = self.accumulated_y.saturating_add(motion.dy as i32);
            }
            Err(_e) => {
                warn!("PMW3610 read error");
            }
        }
    }

    fn take_report_event(&mut self) -> Option<PointingEvent> {
        if self.accumulated_x == 0 && self.accumulated_y == 0 {
            return None;
        }

        let dx = self.accumulated_x.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        let dy = self.accumulated_y.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        self.accumulated_x = 0;
        self.accumulated_y = 0;

        Some(PointingEvent([
            AxisEvent {
                typ: AxisValType::Rel,
                axis: Axis::X,
                value: dx,
            },
            AxisEvent {
                typ: AxisValType::Rel,
                axis: Axis::Y,
                value: dy,
            },
            AxisEvent {
                typ: AxisValType::Rel,
                axis: Axis::Z,
                value: 0,
            },
        ]))
    }

    async fn try_init(&mut self) -> bool {
        match self.init_state {
            InitState::Ready => return true,
            InitState::Failed => return false,
            InitState::Pending => {
                self.init_state = InitState::Initializing(0);
            }
            InitState::Initializing(_) => {}
        }

        if let InitState::Initializing(retry_count) = self.init_state {
            info!("PMW3610: Initializing sensor (attempt {})", retry_count + 1);

            match self.sensor.init().await {
                Ok(()) => {
                    info!("PMW3610: Sensor initialized successfully");
                    self.init_state = InitState::Ready;
                    return true;
                }
                Err(_e) => {
                    error!("PMW3610: Init failed: {:?}", _e);
                    if retry_count + 1 >= Self::MAX_INIT_RETRIES {
                        error!("PMW3610: Max retries reached, giving up");
                        self.init_state = InitState::Failed;
                        return false;
                    }
                    self.init_state = InitState::Initializing(retry_count + 1);
                    Timer::after(Duration::from_millis(100)).await;
                    return false;
                }
            }
        }

        false
    }

    async fn read_pointing_event(&mut self) -> PointingEvent {
        use embassy_futures::select::{Either, select};

        if self.last_poll == Instant::MIN {
            self.last_poll = Instant::now();
        }
        if self.last_report == Instant::MIN {
            self.last_report = Instant::now();
        }

        loop {
            let poll_wait = async {
                if let Some(gpio) = self.sensor.motion_gpio.as_mut() {
                    let _ = gpio.wait_for_low().await;
                } else {
                    Timer::after(
                        self.poll_interval
                            .checked_sub(self.last_poll.elapsed())
                            .unwrap_or(Duration::MIN),
                    )
                    .await;
                }
            };

            let report_wait = async {
                if self.accumulated_x != 0 || self.accumulated_y != 0 {
                    Timer::after(
                        self.report_interval
                            .checked_sub(self.last_report.elapsed())
                            .unwrap_or(Duration::MIN),
                    )
                    .await;
                } else {
                    // Don't schedule report if there's no accumulated motion
                    pending::<()>().await;
                }
            };

            match select(poll_wait, report_wait).await {
                Either::First(_) => {
                    self.poll_once().await;
                    self.last_poll = Instant::now();
                }
                Either::Second(_) => {
                    if let Some(event) = self.take_report_event() {
                        self.last_report = Instant::now();
                        return event;
                    }
                }
            }
        }
    }
}

/// PMW3610 Processor that converts motion events to mouse reports
#[input_processor(subscribe = [PointingEvent])]
pub struct Pmw3610Processor<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize> {
    /// Reference to the keymap
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    Pmw3610Processor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    /// Create a new PMW3610 processor with default settings
    pub fn new(keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>) -> Self {
        Self { keymap }
    }

    async fn generate_report(&self, x: i16, y: i16) {
        let buttons = self.keymap.borrow().mouse_buttons;
        let mouse_report = MouseReport {
            buttons,
            x: x.clamp(i8::MIN as i16, i8::MAX as i16) as i8,
            y: y.clamp(i8::MIN as i16, i8::MAX as i16) as i8,
            wheel: 0,
            pan: 0,
        };
        self.send_report(Report::MouseReport(mouse_report)).await;
    }
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    Pmw3610Processor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    async fn on_pointing_event(&mut self, event: PointingEvent) {
        let mut x = 0i16;
        let mut y = 0i16;

        for axis_event in event.0.iter() {
            match axis_event.axis {
                Axis::X => x = axis_event.value,
                Axis::Y => y = axis_event.value,
                _ => {}
            }
        }

        self.generate_report(x, y).await;
    }
}
