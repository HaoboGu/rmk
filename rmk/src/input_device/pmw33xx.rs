// PMW3360/ PMW3389 Mouse Sensor Driver
//
// Ported from kot149s PMW3610 driver:
// https://github.com/kot149/pmw3610-rs
// Which is ported from the Zephyr driver implementation:
// https://github.com/zephyrproject-rtos/zephyr/blob/d31c6e95033fd6b3763389edba6a655245ae1328/drivers/input/input_pmw3610.c

use embassy_time::{Duration, Timer};
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::spi::SpiBus;

use crate::input_device::pointing::{InitState, MotionData, PointingDevice, PointingDriver, PointingDriverError};

// ============================================================================
// Burst register offsets
// ============================================================================
const BURST_MOTION_FLAGS: usize = 0;
#[allow(dead_code)]
const BURST_OBSERVATION: usize = 1;
const BURST_DELTA_X_L: usize = 2;
const BURST_DELTA_X_H: usize = 3;
const BURST_DELTA_Y_L: usize = 4;
const BURST_DELTA_Y_H: usize = 5;
const BURST_DATA_LEN: usize = 6;

// ============================================================================
// Constants
// ============================================================================
const SPI_WRITE: u8 = 0x80; // BIT(7)
const MOTION_STATUS_MOTION: u8 = 0x80; // BIT(7)
const MOTION_STATUS_LIFTED: u8 = 0x08; // BIT(4)
const POWER_UP_RESET_VAL: u8 = 0x5a;

// Timing constants
const RESET_DELAY_MS: u64 = 50;

// SPI timing constants (from PMW3360 datasheet)
const T_NCS_SCLK_US: u64 = 1;
const T_SRAD_US: u64 = 160;
const T_SRAD_MOTBR_US: u64 = 35;
const T_SRX_US: u64 = 20 - T_NCS_SCLK_US;
const T_SWX_US: u64 = 180 - T_SCLK_NCS_WR_US;
const T_SCLK_NCS_WR_US: u64 = 35 - T_NCS_SCLK_US;
const T_BEXIT_US: u64 = 1;
const T_BRSEP_US: u64 = 15;

// PMW3360/ 3389 registers are almost alike. This Enum contains a few extra registers that are not available for the 3360
#[allow(dead_code)]
#[derive(Eq, PartialEq, Debug)]
enum Register {
    ProductId,
    RevisionId,
    Motion,
    DeltaXL,
    DeltaXH,
    DeltaYL,
    DeltaYH,
    SQUAL,
    RawDataSum,
    MaximumRawData,
    MinimumRawData,
    ShutterLower,
    ShutterUpper,
    Control,
    Config1L,
    Config1H,
    Config2,
    AngleTune,
    FrameCapture,
    SromEnable,
    RunDownshift,
    Rest1RateLower,
    Rest1RateUpper,
    Rest1Downshift,
    Rest2RateLower,
    Rest2RateUpper,
    Rest2Downshift,
    Rest3RateLower,
    Rest3RateUpper,
    Observation,
    DataOutLower,
    DataOutUpper,
    RawDataDump,
    SromId,
    MinSqRun,
    RawDataThreshold,
    Control2,
    Config5L,
    Config5H,
    PowerUpReset,
    Shutdown,
    InverseProductId,
    LiftCutoffTune3,
    AngleSnap,
    LiftCutoffTune1,
    MotionBurst,
    LiftCutoffTuneTimeout,
    LiftCutoffTuneMinLength,
    SromLoadBurst,
    LiftConfig,
    RawDataBurst,
    LiftCutoffTune2,
    LiftCutoffCalTimeout,
    LiftCutoffCalMinLen,
    PWMPeriodCnt,
    PwmWidthCnt,
}

impl Register {
    fn value(&self) -> u8 {
        match self {
            Register::ProductId => 0x00,
            Register::RevisionId => 0x01,
            Register::Motion => 0x02,
            Register::DeltaXL => 0x03,
            Register::DeltaXH => 0x04,
            Register::DeltaYL => 0x05,
            Register::DeltaYH => 0x06,
            Register::SQUAL => 0x07,
            Register::RawDataSum => 0x08,
            Register::MaximumRawData => 0x09,
            Register::MinimumRawData => 0x0a,
            Register::ShutterLower => 0x0b,
            Register::ShutterUpper => 0x0c,
            Register::Control => 0x0d,
            Register::Config1L => 0x0e,
            Register::Config1H => 0x0f,
            Register::Config2 => 0x10,
            Register::AngleTune => 0x11,
            Register::FrameCapture => 0x12,
            Register::SromEnable => 0x13,
            Register::RunDownshift => 0x14,
            Register::Rest1RateLower => 0x15,
            Register::Rest1RateUpper => 0x16,
            Register::Rest1Downshift => 0x17,
            Register::Rest2RateLower => 0x18,
            Register::Rest2RateUpper => 0x19,
            Register::Rest2Downshift => 0x1a,
            Register::Rest3RateLower => 0x1b,
            Register::Rest3RateUpper => 0x1c,
            Register::Observation => 0x24,
            Register::DataOutLower => 0x25,
            Register::DataOutUpper => 0x26,
            Register::RawDataDump => 0x29,
            Register::SromId => 0x2a,
            Register::MinSqRun => 0x2b,
            Register::RawDataThreshold => 0x2c,
            Register::Control2 => 0x2d,
            Register::Config5L => 0x2f,
            Register::Config5H => 0x2f,
            Register::PowerUpReset => 0x3a,
            Register::Shutdown => 0x3b,
            Register::InverseProductId => 0x3f,
            Register::LiftCutoffTune3 => 0x41,
            Register::AngleSnap => 0x42,
            Register::LiftCutoffTune1 => 0x4a,
            Register::MotionBurst => 0x50,
            Register::LiftCutoffTuneTimeout => 0x58,
            Register::LiftCutoffTuneMinLength => 0x5a,
            Register::SromLoadBurst => 0x62,
            Register::LiftConfig => 0x63,
            Register::RawDataBurst => 0x64,
            Register::LiftCutoffTune2 => 0x65,
            Register::LiftCutoffCalTimeout => 0x71,
            Register::LiftCutoffCalMinLen => 0x72,
            Register::PWMPeriodCnt => 0x73,
            Register::PwmWidthCnt => 0x74,
        }
    }
}

pub trait Pmw33xxSpec {
    // sensor type (60 or 89)
    const TYPENAME: &str;
    const PRODUCT_ID: u8;
    // firmware signature
    const FW_SIG_PID: u8;
    const FW_SIG_INV_PID: u8;
    // resolution constants
    const RES_MIN: u16;
    const RES_MAX: u16;
    const RES_STEP: u16;

    /// Write resolution register
    async fn write_resolution<SPI, CS, MOTION, SPEC>(
        driver: &mut Pmw33xx<'_, SPI, CS, MOTION, SPEC>,
        cpi: u16,
    ) -> Result<(), Pmw33xxError>
    where
        SPI: SpiBus,
        CS: OutputPin,
        MOTION: InputPin,
        SPEC: Pmw33xxSpec;
}

pub struct Pmw3360Spec;

impl Pmw33xxSpec for Pmw3360Spec {
    const TYPENAME: &str = "60";
    const PRODUCT_ID: u8 = 0x42;
    const FW_SIG_PID: u8 = Self::PRODUCT_ID;
    const FW_SIG_INV_PID: u8 = 0xBD;
    const RES_MIN: u16 = 100;
    const RES_MAX: u16 = 12000;
    const RES_STEP: u16 = 100;

    async fn write_resolution<SPI, CS, MOTION, SPEC>(
        driver: &mut Pmw33xx<'_, SPI, CS, MOTION, SPEC>,
        cpi: u16,
    ) -> Result<(), Pmw33xxError>
    where
        SPI: SpiBus,
        CS: OutputPin,
        MOTION: InputPin,
        SPEC: Pmw33xxSpec,
    {
        driver.write_reg(Register::Config1H, ((cpi / Self::RES_STEP) - 1) as u8).await?;
        Ok(())
    }
}

pub struct Pmw3389Spec;

impl Pmw33xxSpec for Pmw3389Spec {
    const TYPENAME: &str = "89";
    const PRODUCT_ID: u8 = 0x47;
    const FW_SIG_PID: u8 = Self::PRODUCT_ID;
    const FW_SIG_INV_PID: u8 = 0xB8;
    const RES_MIN: u16 = 50;
    const RES_MAX: u16 = 16000;
    const RES_STEP: u16 = 50;

    async fn write_resolution<SPI, CS, MOTION, SPEC>(
        driver: &mut Pmw33xx<'_, SPI, CS, MOTION, SPEC>,
        cpi: u16,
    ) -> Result<(), Pmw33xxError>
    where
        SPI: SpiBus,
        CS: OutputPin,
        MOTION: InputPin,
        SPEC: Pmw33xxSpec,
    {
        if !(Self::RES_MIN..=Self::RES_MAX).contains(&cpi) {
            return Err(Pmw33xxError::InvalidCpi);
        }

        let cpi = (cpi / Self::RES_STEP) - 1;
        driver.write_reg(Register::Config1H, (cpi >> 8) as u8).await?;
        driver.write_reg(Register::Config1L, (cpi & 0xFF) as u8).await?;

        Ok(())
    }
}


/// PMW33xx configuration
#[derive(Clone)]
pub struct Pmw33xxConfig {
    /// CPI resolution (100-12000, step 100)
    pub res_cpi: u16,
    /// rot_trans_angle (-127 to 127
    pub rot_trans_angle: i8,
    /// liftoff distance
    pub liftoff_dist: u8,
    /// Invert X axis
    pub invert_x: bool,
    /// Invert Y axis
    pub invert_y: bool,
    /// Swap X and Y axes
    pub swap_xy: bool,
}

impl Default for Pmw33xxConfig {
    fn default() -> Self {
        Self {
            res_cpi: 1600,
            rot_trans_angle: 0,
            liftoff_dist: 0x02,
            invert_x: false,
            invert_y: false,
            swap_xy: false,
        }
    }
}

/// PMW33xx error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Pmw33xxError {
    /// SPI communication error
    Spi,
    /// Invalid product ID detected
    InvalidProductId(u8),
    /// Initialization failed
    InitFailed,
    /// Invalid CPI value
    InvalidCpi,
    /// Invalid firmware signature detected
    InvalidFwSignature((u8, u8)),
}

/// PMW33xx driver using embedded-hal SPI traits
pub struct Pmw33xx<'a, SPI, CS, MOTION, SPEC>
where
    SPI: SpiBus,
    CS: OutputPin,
    MOTION: InputPin,
    SPEC: Pmw33xxSpec,
{
    spi: SPI,
    cs: CS,
    motion_gpio: Option<MOTION>,
    config: Pmw33xxConfig,
    in_burst: bool,
    srom_firmware: Option<&'a [u8]>,
    _spec: core::marker::PhantomData<SPEC>,
}

impl<'a, SPI, CS, MOTION, SPEC> Pmw33xx<'a, SPI, CS, MOTION, SPEC>
where
    SPI: SpiBus,
    CS: OutputPin,
    MOTION: InputPin,
    SPEC: Pmw33xxSpec,
{
    /// Create a new PMW33xx driver instance
    pub fn new(spi: SPI, cs: CS, motion_gpio: Option<MOTION>, config: Pmw33xxConfig) -> Self {
        Self {
            spi,
            cs,
            motion_gpio,
            config,
            in_burst: false,
            srom_firmware: None,
            _spec: core::marker::PhantomData,
        }
    }

    /// Create a new PMW33xx driver instance with firmware (SROM)
    pub fn new_with_firmware(
        spi: SPI,
        cs: CS,
        motion_gpio: Option<MOTION>,
        config: Pmw33xxConfig,
        firmware: &'a [u8],
    ) -> Self {
        Self {
            spi,
            cs,
            motion_gpio,
            config,
            in_burst: false,
            srom_firmware: Some(firmware),
            _spec: core::marker::PhantomData,
        }
    }

    /// Self check firmware signature of the sensor
    pub async fn check_fw_signature(&mut self) -> Result<(), Pmw33xxError> {
        let product_id = self.read_reg(Register::ProductId).await?;
        let inverse_product_id = self.read_reg(Register::InverseProductId).await?;

        if product_id == SPEC::FW_SIG_PID && inverse_product_id == SPEC::FW_SIG_INV_PID {
            Ok(())
        } else {
            error!(
                "PMW33{}: Firmware signature check failed, expected: {}, {} got: {}, {}",
                SPEC::TYPENAME, SPEC::FW_SIG_PID, SPEC::FW_SIG_INV_PID, product_id, inverse_product_id
            );
            Err(Pmw33xxError::InvalidFwSignature((product_id, inverse_product_id)))
        }
    }

    /// Set sensor resolution in CPI (100-12000, step 100)
    pub async fn set_resolution(&mut self, cpi: u16) -> Result<(), Pmw33xxError> {
        if !(SPEC::RES_MIN..=SPEC::RES_MAX).contains(&cpi) {
            return Err(Pmw33xxError::InvalidCpi);
        }

        SPEC::write_resolution(self, cpi).await?;

        debug!("PMW33{}: Resolution set to {} CPI", SPEC::TYPENAME, cpi);

        Ok(())
    }

    /// Set sensor rotational transform angle (-127 to 127)
    pub async fn set_rot_trans_angle(&mut self, angle: i8) -> Result<(), Pmw33xxError> {
        self.write_reg(Register::AngleTune, angle as u8).await?;

        debug!("PMW33{}: Rotational transform angle set to {}",SPEC::TYPENAME, angle);

        Ok(())
    }

    /// Set sensor liftoff distance
    pub async fn set_liftoff_dist(&mut self, dist: u8) -> Result<(), Pmw33xxError> {
        self.write_reg(Register::LiftConfig, dist).await?;

        debug!("PMW33{}: Liftoff distance set to {}", SPEC::TYPENAME, dist);

        Ok(())
    }

    #[inline(always)]
    fn short_delay() {
        for _ in 0..64 {
            core::hint::spin_loop();
        }
    }

    async fn read_reg(&mut self, register: Register) -> Result<u8, Pmw33xxError> {
        let _ = self.cs.set_low();
        Timer::after(Duration::from_micros(T_NCS_SCLK_US)).await;

        // Send address with read bit (bit 7 = 0)
        self.spi
            .write(&[register.value() & 0x7f])
            .await
            .map_err(|_| Pmw33xxError::Spi)?;

        Timer::after(Duration::from_micros(T_SRAD_US)).await;

        let mut value = [0u8];
        self.spi.read(&mut value).await.map_err(|_| Pmw33xxError::Spi)?;

        Self::short_delay();
        let _ = self.cs.set_high();

        Timer::after(Duration::from_micros(T_SRX_US)).await;

        Ok(value[0])
    }

    async fn read_burst(&mut self, register: Register, data: &mut [u8]) -> Result<(), Pmw33xxError> {
        let _ = self.cs.set_low();
        Timer::after(Duration::from_micros(T_NCS_SCLK_US)).await;

        // Send address with read bit (bit 7 = 0)
        self.spi
            .write(&[register.value() & 0x7f])
            .await
            .map_err(|_| Pmw33xxError::Spi)?;

        Timer::after(Duration::from_micros(T_SRAD_MOTBR_US)).await;

        self.spi.read(data).await.map_err(|_| Pmw33xxError::Spi)?;

        Self::short_delay();
        let _ = self.cs.set_high();

        Timer::after(Duration::from_micros(T_BEXIT_US)).await;

        Ok(())
    }

    async fn write_reg(&mut self, register: Register, value: u8) -> Result<(), Pmw33xxError> {
        let _ = self.cs.set_low();
        Timer::after(Duration::from_micros(T_NCS_SCLK_US)).await;

        // Send address with write bit (bit 7 = 1)
        self.spi
            .write(&[register.value() | SPI_WRITE, value])
            .await
            .map_err(|_| Pmw33xxError::Spi)?;

        Timer::after(Duration::from_micros(T_SCLK_NCS_WR_US)).await;
        let _ = self.cs.set_high();

        Timer::after(Duration::from_micros(T_SWX_US)).await;

        Ok(())
    }

    async fn configure(&mut self) -> Result<(), Pmw33xxError> {
        // Power-up reset
        self.write_reg(Register::PowerUpReset, POWER_UP_RESET_VAL).await?;
        Timer::after(Duration::from_millis(RESET_DELAY_MS)).await;

        // Verify product ID
        let val = self.read_reg(Register::ProductId).await?;
        if val != SPEC::PRODUCT_ID {
            error!("PWM33{}: Invalid product id: {:#02}", SPEC::TYPENAME, val);
            return Err(Pmw33xxError::InvalidProductId(val));
        }
        info!("PMW33{} detected, product ID: {:#02x}", SPEC::TYPENAME, val);

        // Power-up init sequence
        // Read motion registers to clear them
        self.read_reg(Register::Motion).await?;
        self.read_reg(Register::DeltaXL).await?;
        self.read_reg(Register::DeltaXH).await?;
        self.read_reg(Register::DeltaYL).await?;
        self.read_reg(Register::DeltaYH).await?;

        if let Some(firmware) = self.srom_firmware {
            self.upload_firmware(firmware).await?;
        }

        self.set_resolution(self.config.res_cpi as u16).await?;
        self.write_reg(Register::Config2, 0x00).await?;
        self.set_rot_trans_angle(self.config.rot_trans_angle).await?;
        self.set_liftoff_dist(self.config.liftoff_dist).await?;

        self.check_fw_signature().await?;

        info!("PMW33{} initialized successfully", SPEC::TYPENAME);
        Ok(())
    }

    async fn upload_firmware(&mut self, firmware: &[u8]) -> Result<(), Pmw33xxError> {
        self.write_reg(Register::Config2, 0x00).await?; // disable REST mode

        let srom_id = firmware[1];
        info!("PMW33{}: Uploading SROM firmware with SROM-Id 0x{:02x}", SPEC::TYPENAME, srom_id);

        self.write_reg(Register::SromEnable, 0x1d).await?;
        Timer::after(Duration::from_millis(10)).await;
        self.write_reg(Register::SromEnable, 0x18).await?;

        let _ = self.cs.set_low();
        Timer::after(Duration::from_micros(T_NCS_SCLK_US)).await;

        self.spi
            .write(&[Register::SromLoadBurst.value() | SPI_WRITE])
            .await
            .map_err(|_| Pmw33xxError::Spi)?;

        Timer::after(Duration::from_micros(T_SCLK_NCS_WR_US)).await;

        for &byte in firmware {
            debug!("PMW33{}: Uploading srom byte: 0x{:02x}", SPEC::TYPENAME, byte);
            self.spi.write(&[byte]).await.map_err(|_| Pmw33xxError::Spi)?;
            Timer::after(Duration::from_micros(T_BRSEP_US)).await;
        }

        let _ = self.cs.set_high();

        Timer::after(Duration::from_micros(T_BEXIT_US)).await;

        let flashed_srom_id = self.read_reg(Register::SromId).await?;
        if srom_id != flashed_srom_id {
            error!(
                "PMW33{}: SROM Firmware upload failed, expected SROM-Id 0x{:02x}, but got 0x{:02x} from the sensor.", SPEC::TYPENAME,
                srom_id, flashed_srom_id
            );
        } else {
            info!("PMW33{}: Upload successfull, new SROM-Id: 0x{:02x}", SPEC::TYPENAME, flashed_srom_id);
        }

        self.write_reg(Register::Config2, 0x00).await?;

        Ok(())
    }
}

impl<'a, SPI, CS, MOTION, SPEC> PointingDriver for Pmw33xx<'a, SPI, CS, MOTION, SPEC>
where
    SPI: SpiBus,
    CS: OutputPin,
    MOTION: InputPin,
    SPEC: Pmw33xxSpec,
{
    /// Initialize the sensor (public API)
    async fn init(&mut self) -> Result<(), PointingDriverError> {
        // Set initial pin states
        let _ = self.cs.set_high();
        Timer::after(Duration::from_millis(1)).await;

        self.configure().await.map_err(|_| PointingDriverError::InitFailed)
    }

    /// Read motion data from the sensor (motion work handler)
    async fn read_motion(&mut self) -> Result<MotionData, PointingDriverError> {
        if !self.in_burst {
            self.write_reg(Register::MotionBurst, 0x00)
                .await
                .map_err(|_| PointingDriverError::Spi)?;
            self.in_burst = true;
        }

        let mut burst_data = [0u8; BURST_DATA_LEN];
        self.read_burst(Register::MotionBurst, &mut burst_data[..BURST_DATA_LEN])
            .await
            .map_err(|_| PointingDriverError::Spi)?;

        debug!("PMW33{}: Burst raw data {:?}", SPEC::TYPENAME, burst_data);

        // panic recovery, sometimes burst mode works weird.
        if (burst_data[BURST_MOTION_FLAGS] & 0b111) != 0x00 {
            debug!("PMW33{}: Burst panic recovery", SPEC::TYPENAME);
            self.in_burst = false;
        }

        if (burst_data[BURST_MOTION_FLAGS] & MOTION_STATUS_MOTION) == 0x00 {
            return Ok(MotionData::default());
        }
        if (burst_data[BURST_MOTION_FLAGS] & MOTION_STATUS_LIFTED) != 0x00 {
            return Ok(MotionData::default());
        }

        let mut dx: i16 = i16::from_le_bytes([burst_data[BURST_DELTA_X_L], burst_data[BURST_DELTA_X_H]]);
        let mut dy: i16 = i16::from_le_bytes([burst_data[BURST_DELTA_Y_L], burst_data[BURST_DELTA_Y_H]]);

        if self.config.invert_x {
            dx = dx * (-1);
        }
        if self.config.invert_y {
            dy = dy * (-1);
        }
        if self.config.swap_xy {
            (dx, dy) = (dy, dx);
        }

        debug!("PMW33{} motion: x: {}, y: {}", SPEC::TYPENAME, dx, dy);

        Ok(MotionData { dx, dy })
    }

    /// Check if motion is pending (motion GPIO is active low)
    fn motion_pending(&mut self) -> bool {
        match &mut self.motion_gpio {
            Some(gpio) => gpio.is_low().unwrap_or(true),
            None => true,
        }
    }
}

/// PMW33xx as an InputDevice for RMK
impl<'a, SPI, CS, MOTION, SPEC> PointingDevice<Pmw33xx<'a, SPI, CS, MOTION, SPEC>>
where
    SPI: SpiBus,
    CS: OutputPin,
    MOTION: InputPin,
    SPEC: Pmw33xxSpec,
{
    /// Create a new PMW33xx device
    pub fn new(spi: SPI, cs: CS, motion_gpio: Option<MOTION>, config: Pmw33xxConfig) -> Self {
        Self {
            sensor: Pmw33xx::new(spi, cs, motion_gpio, config),
            init_state: InitState::Pending,
            poll_interval: Duration::from_micros(500),
        }
    }

    /// Create a new PMW33xx device with custom poll interval
    pub fn with_poll_interval(
        spi: SPI,
        cs: CS,
        motion_gpio: Option<MOTION>,
        config: Pmw33xxConfig,
        poll_interval_us: u64,
    ) -> Self {
        Self {
            sensor: Pmw33xx::new(spi, cs, motion_gpio, config),
            init_state: InitState::Pending,
            poll_interval: Duration::from_micros(poll_interval_us),
        }
    }

    /// Create a new PMW33xx device with SROM firmware
    ///
    /// Firmware is downloaded to the sensor on every startup
    pub fn new_with_firmware(
        spi: SPI,
        cs: CS,
        motion_gpio: Option<MOTION>,
        config: Pmw33xxConfig,
        firmware: &'a [u8],
    ) -> Self {
        Self {
            sensor: Pmw33xx::new_with_firmware(spi, cs, motion_gpio, config, firmware),
            init_state: InitState::Pending,
            poll_interval: Duration::from_micros(500),
        }
    }

    /// Create a new PMW33xx device with SROM firmware and custom poll intervall
    ///
    /// Firmware is downloaded to the sensor on every startup
    pub fn new_with_firmware_poll_interval(
        spi: SPI,
        cs: CS,
        motion_gpio: Option<MOTION>,
        config: Pmw33xxConfig,
        poll_interval_us: u64,
        firmware: &'a [u8],
    ) -> Self {
        Self {
            sensor: Pmw33xx::new_with_firmware(spi, cs, motion_gpio, config, firmware),
            init_state: InitState::Pending,
            poll_interval: Duration::from_micros(poll_interval_us),
        }
    }
}

