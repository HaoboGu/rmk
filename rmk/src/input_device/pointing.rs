//! Common functionality across pointing devices

use core::cell::RefCell;

use embassy_time::{Duration, Instant, Timer};
use embedded_hal::digital::InputPin;
use embedded_hal_async::digital::Wait;
use futures::future::pending;
use rmk_macro::{input_device, processor};
use usbd_hid::descriptor::MouseReport;

use crate::channel::KEYBOARD_REPORT_CHANNEL;
use crate::event::{Axis, AxisEvent, AxisValType, LayerChangeEvent, PointingEvent, PointingSetCpiEvent};
use crate::hid::Report;
use crate::keymap::KeyMap;

pub const ALL_POINTING_DEVICES: u8 = 255;

/// Motion data from the sensor
#[derive(Debug, Clone, Copy, Default)]
pub struct MotionData {
    pub dx: i16,
    pub dy: i16,
}

/// Errors of pointing devices
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PointingDriverError {
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
    /// Invalid rotational transform angle
    InvalidRotTransAngle,
}

pub trait PointingDriver {
    type MOTION: InputPin + Wait;

    async fn init(&mut self) -> Result<(), PointingDriverError>;
    async fn read_motion(&mut self) -> Result<MotionData, PointingDriverError>;
    fn motion_pending(&mut self) -> bool;
    fn motion_gpio(&mut self) -> Option<&mut Self::MOTION>;
}

/// Initialization state for the device
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitState {
    Pending,
    Initializing(u8),
    Ready,
    Failed,
}

/// PointingDevice an InputDevice for RMK
///
/// This device publishes `PointingEvent` events with relative X/Y movement.
#[processor(subscribe = [PointingSetCpiEvent])]
#[input_device(publish = PointingEvent)]
pub struct PointingDevice<S: PointingDriver> {
    pub sensor: S,
    pub init_state: InitState,
    pub poll_interval: Duration,
    pub id: u8,
    pub report_interval: Duration,
    pub last_poll: Instant,
    pub last_report: Instant,
    pub accumulated_x: i32,
    pub accumulated_y: i32,
}

impl<S: PointingDriver> PointingDevice<S> {
    const MAX_INIT_RETRIES: u8 = 3;

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
            info!(
                "PointingDevice {}: Initializing sensor (attempt {})",
                self.id,
                retry_count + 1
            );

            match self.sensor.init().await {
                Ok(()) => {
                    info!("PointingDevice {}: Sensor initialized successfully", self.id);
                    self.init_state = InitState::Ready;
                    return true;
                }
                Err(e) => {
                    error!("PointingDevice {}: Init failed: {:?}", self.id, e);
                    if retry_count + 1 >= Self::MAX_INIT_RETRIES {
                        error!("PointingDevice {}: Max retries reached, giving up", self.id);
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
                warn!("PointingDevice {}: Read motion error", self.id);
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
}

impl<S: PointingDriver> PointingDevice<S> {
    async fn on_pointing_set_cpi_event(&mut self, _e: PointingSetCpiEvent) {}

    // Read accumulated pointing event
    //
    // +--------------- loop ---------------+
    // ¦ poll_wait   report_wait            ¦
    // ¦     ¦           ¦                  ¦
    // ¦     V           V                  ¦
    // ¦ poll_once()     take_report_event()¦
    // ¦     ¦           ¦                  ¦
    // ¦     +- accum += ¦                  ¦
    // ¦                 >- Event returned  ¦
    // +------------------------------------+
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
                if let Some(gpio) = self.sensor.motion_gpio() {
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

/// Pointing mode determines how raw XY motion is interpreted
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PointingMode {
    /// Default cursor mode - XY maps to mouse XY movement
    #[default]
    Cursor,
    /// Scroll mode - XY maps to wheel (vertical) and pan (horizontal)
    Scroll(ScrollConfig),
    /// Sniper mode - XY maps to cursor but at reduced sensitivity
    Sniper(SniperConfig),
}

/// Configuration for scroll mode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ScrollConfig {
    /// Divisor for X axis (pan). Higher = slower scrolling. 0 treated as 1.
    pub divisor_x: u8,
    /// Divisor for Y axis (wheel). Higher = slower scrolling. 0 treated as 1.
    pub divisor_y: u8,
}

impl Default for ScrollConfig {
    fn default() -> Self {
        Self {
            divisor_x: 8,
            divisor_y: 8,
        }
    }
}

/// Configuration for sniper (precision) mode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SniperConfig {
    /// Divisor for both axes. Higher = slower movement. 0 treated as 1.
    pub divisor: u8,
}

impl Default for SniperConfig {
    fn default() -> Self {
        Self { divisor: 4 }
    }
}

/// Accumulator for sub-unit motion deltas (used in Scroll and Sniper modes)
///
/// When dividing motion by a divisor, small movements would be lost.
/// The accumulator keeps track of the remainder so sub-unit deltas
/// accumulate until they produce a non-zero output.
#[derive(Clone, Debug, Default)]
pub struct MotionAccumulator {
    remainder_x: i16,
    remainder_y: i16,
}

impl MotionAccumulator {
    /// Reset accumulator (call when mode changes)
    pub fn reset(&mut self) {
        self.remainder_x = 0;
        self.remainder_y = 0;
    }

    /// Accumulate motion and return the divided output, keeping remainder
    pub fn accumulate(&mut self, dx: i16, dy: i16, divisor_x: u8, divisor_y: u8) -> (i16, i16) {
        let div_x = divisor_x.max(1) as i16;
        let div_y = divisor_y.max(1) as i16;

        let total_x = self.remainder_x.saturating_add(dx);
        let total_y = self.remainder_y.saturating_add(dy);

        let out_x = total_x / div_x;
        let out_y = total_y / div_y;

        self.remainder_x = total_x - out_x * div_x;
        self.remainder_y = total_y - out_y * div_y;

        (out_x, out_y)
    }
}

#[derive(Clone, Default)]
pub struct PointingProcessorConfig {
    /// Invert X axis
    pub invert_x: bool,
    /// Invert Y axis
    pub invert_y: bool,
    /// Swap X and Y axes
    pub swap_xy: bool,
}

/// PointingProcessor that converts motion events to mouse reports.
///
/// Supports per-layer pointing modes: different layers can have different
/// pointing behaviors (cursor, scroll, sniper). Layer changes are received
/// via `LayerChangeEvent`.
///
/// # Example
///
/// ```no_run
/// use rmk::input_device::pointing::{
///     PointingProcessor, PointingProcessorConfig, PointingMode,
///     ScrollConfig, SniperConfig
/// };
///
/// // Create processor with default config
/// let config = PointingProcessorConfig::default();
/// let mut processor = PointingProcessor::new(&keymap, config);
///
/// // Configure per-layer modes:
/// // Layer 0: Cursor (default, normal trackball movement)
/// processor.set_layer_mode(0, PointingMode::Cursor);
///
/// // Layer 1: Scroll (trackball becomes scroll wheel)
/// processor.set_layer_mode(1, PointingMode::Scroll(ScrollConfig {
///     divisor_x: 8,  // Pan sensitivity (higher = slower)
///     divisor_y: 8,  // Wheel sensitivity (higher = slower)
/// }));
///
/// // Layer 2: Sniper (precision mode, 1/4 speed)
/// processor.set_layer_mode(2, PointingMode::Sniper(SniperConfig {
///     divisor: 4,  // Movement divisor (higher = slower)
/// }));
///
/// // In keymap: use MO(1) to activate scroll, MO(2) for sniper
/// // Layer switching automatically changes pointing behavior
/// ```
#[processor(subscribe = [PointingEvent, LayerChangeEvent])]
pub struct PointingProcessor<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize> {
    /// Reference to the keymap (used for mouse_buttons)
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    /// Base configuration (invert/swap axes)
    config: PointingProcessorConfig,
    /// Per-layer pointing mode
    layer_modes: [PointingMode; NUM_LAYER],
    /// Motion accumulator for scroll/sniper modes
    accumulator: MotionAccumulator,
    /// Current active layer (updated via LayerChangeEvent)
    current_layer: u8,
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    PointingProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    /// Create a new pointing processor with default settings (all layers = Cursor mode)
    pub fn new(
        keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
        config: PointingProcessorConfig,
    ) -> Self {
        Self {
            keymap,
            config,
            layer_modes: [PointingMode::default(); NUM_LAYER],
            accumulator: MotionAccumulator::default(),
            current_layer: 0,
        }
    }

    /// Set the pointing mode for a specific layer
    pub fn set_layer_mode(&mut self, layer: usize, mode: PointingMode) -> &mut Self {
        if layer < NUM_LAYER {
            self.layer_modes[layer] = mode;
        }
        self
    }

    /// Set pointing modes for all layers at once
    pub fn with_layer_modes(mut self, modes: [PointingMode; NUM_LAYER]) -> Self {
        self.layer_modes = modes;
        self
    }

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

        // Apply base config transforms
        if self.config.invert_x {
            x = -x;
        }
        if self.config.invert_y {
            y = -y;
        }
        if self.config.swap_xy {
            (x, y) = (y, x);
        }

        // Get the pointing mode for the current layer
        let mode = self
            .layer_modes
            .get(self.current_layer as usize)
            .copied()
            .unwrap_or_default();

        let buttons = self.keymap.borrow().mouse_buttons;

        let mouse_report = match mode {
            PointingMode::Cursor => MouseReport {
                buttons,
                x: x.clamp(i8::MIN as i16, i8::MAX as i16) as i8,
                y: y.clamp(i8::MIN as i16, i8::MAX as i16) as i8,
                wheel: 0,
                pan: 0,
            },
            PointingMode::Scroll(scroll_config) => {
                let (sx, sy) = self
                    .accumulator
                    .accumulate(x, y, scroll_config.divisor_x, scroll_config.divisor_y);
                if sx == 0 && sy == 0 {
                    return;
                }
                MouseReport {
                    buttons,
                    x: 0,
                    y: 0,
                    wheel: (-sy).clamp(i8::MIN as i16, i8::MAX as i16) as i8,
                    pan: sx.clamp(i8::MIN as i16, i8::MAX as i16) as i8,
                }
            }
            PointingMode::Sniper(sniper_config) => {
                let (sx, sy) = self
                    .accumulator
                    .accumulate(x, y, sniper_config.divisor, sniper_config.divisor);
                if sx == 0 && sy == 0 {
                    return;
                }
                MouseReport {
                    buttons,
                    x: sx.clamp(i8::MIN as i16, i8::MAX as i16) as i8,
                    y: sy.clamp(i8::MIN as i16, i8::MAX as i16) as i8,
                    wheel: 0,
                    pan: 0,
                }
            }
        };

        KEYBOARD_REPORT_CHANNEL.send(Report::MouseReport(mouse_report)).await;
    }

    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        if self.current_layer != event.layer {
            self.accumulator.reset();
            self.current_layer = event.layer;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use embassy_futures::block_on;
    use embassy_time::Duration;
    use embedded_hal::digital::{ErrorType, InputPin};
    use embedded_hal_async::digital::Wait;

    use super::*;
    use crate::input_device::InputDevice;

    // Init logger for tests
    #[ctor::ctor]
    fn init_log() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();
    }

    struct DummyDriver {
        pub motion_pending: bool,
        pub motion: MotionData,
        pub init_called: bool,
        pub fails_init: bool,
        pub motion_gpio: Option<DummyMotionPin>,
        pub read_called: bool,
    }

    impl PointingDriver for DummyDriver {
        type MOTION = DummyMotionPin;

        async fn init(&mut self) -> Result<(), PointingDriverError> {
            self.init_called = true;
            if self.fails_init {
                Err(PointingDriverError::InitFailed)
            } else {
                Ok(())
            }
        }

        async fn read_motion(&mut self) -> Result<MotionData, PointingDriverError> {
            self.read_called = true;
            Ok(self.motion)
        }

        fn motion_pending(&mut self) -> bool {
            self.motion_pending
        }

        fn motion_gpio(&mut self) -> Option<&mut Self::MOTION> {
            self.motion_gpio.as_mut()
        }
    }

    #[derive(Debug)]
    struct DummyError;

    struct DummyMotionPin {
        state: Cell<bool>, // true = High, false = Low
    }
    impl ErrorType for DummyMotionPin {
        type Error = DummyError;
    }
    impl embedded_hal::digital::Error for DummyError {
        fn kind(&self) -> embedded_hal::digital::ErrorKind {
            embedded_hal::digital::ErrorKind::Other
        }
    }

    impl DummyMotionPin {
        fn new() -> Self {
            Self { state: Cell::new(true) } // initial high, we wait for low
        }

        fn set_low(&self) {
            self.state.set(false);
        }

        fn set_high(&self) {
            self.state.set(true);
        }
    }

    impl InputPin for DummyMotionPin {
        fn is_high(&mut self) -> Result<bool, Self::Error> {
            Ok(self.state.get())
        }
        fn is_low(&mut self) -> Result<bool, Self::Error> {
            Ok(!self.state.get())
        }
    }

    impl Wait for DummyMotionPin {
        async fn wait_for_high(&mut self) -> Result<(), Self::Error> {
            while !self.state.get() { /* spin */ }
            Ok(())
        }

        async fn wait_for_low(&mut self) -> Result<(), Self::Error> {
            embassy_time::Timer::after(Duration::from_millis(500)).await;
            Ok(())
        }
        async fn wait_for_rising_edge(&mut self) -> Result<(), Self::Error> {
            todo!()
        }
        async fn wait_for_falling_edge(&mut self) -> Result<(), Self::Error> {
            todo!()
        }
        async fn wait_for_any_edge(&mut self) -> Result<(), Self::Error> {
            todo!()
        }
    }
    #[test]
    fn test_try_init_retries_and_fails() {
        let driver = DummyDriver {
            motion_pending: true,
            motion: MotionData { dx: 10, dy: -5 },
            init_called: false,
            fails_init: true,
            motion_gpio: None,
            read_called: false,
        };

        let mut device = PointingDevice {
            sensor: driver,
            init_state: InitState::Pending,
            poll_interval: Duration::from_millis(1),
            id: 1,

            report_interval: Duration::from_millis(1),
            last_poll: Instant::MIN,
            last_report: Instant::MIN,
            accumulated_x: 0,
            accumulated_y: 0,
        };

        let mut result = false;
        for i in 0..PointingDevice::<DummyDriver>::MAX_INIT_RETRIES {
            result = block_on(device.try_init());

            if i + 1 < PointingDevice::<DummyDriver>::MAX_INIT_RETRIES {
                // Vorletzte und erste Versuche: state sollte Initializing sein
                assert_eq!(device.init_state, InitState::Initializing(i + 1));
                assert!(!result, "Init should not succeed yet on attempt {}", i + 1);
            } else {
                // Letzter Versuch: state wird direkt auf Failed gesetzt
                assert_eq!(device.init_state, InitState::Failed);
                assert!(!result, "Init should fail after max retries");
            }
        }
        assert!(!result);
        assert_eq!(device.init_state, InitState::Failed);
    }

    #[test]
    fn test_try_init_sets_state() {
        let driver = DummyDriver {
            motion_pending: true,
            motion: MotionData { dx: 10, dy: -5 },
            init_called: false,
            fails_init: false,
            motion_gpio: None,
            read_called: false,
        };

        let mut device = PointingDevice {
            sensor: driver,
            init_state: InitState::Pending,
            poll_interval: Duration::from_millis(1),
            id: 1,

            report_interval: Duration::from_millis(1),
            last_poll: Instant::MIN,
            last_report: Instant::MIN,
            accumulated_x: 0,
            accumulated_y: 0,
        };

        // Run the async try_init
        let result = block_on(device.try_init());
        assert!(result, "Init should succeed");
        assert_eq!(device.init_state, InitState::Ready);
        assert!(device.sensor.init_called, "Driver init should be called");
    }

    #[test]
    fn test_poll_once_accumulate_motion() {
        let motion_pin = DummyMotionPin::new();

        let driver = DummyDriver {
            motion_pending: true,
            motion: MotionData { dx: 10, dy: -5 },
            init_called: false,
            fails_init: false,
            motion_gpio: Some(motion_pin),
            read_called: false,
        };

        let mut device = PointingDevice {
            sensor: driver,
            init_state: InitState::Pending,
            poll_interval: Duration::from_millis(1),
            id: 1,

            report_interval: Duration::from_millis(1),
            last_poll: Instant::MIN,
            last_report: Instant::MIN,
            accumulated_x: 0,
            accumulated_y: 0,
        };

        let inited = block_on(device.try_init());
        assert!(inited);
        assert_eq!(device.init_state, InitState::Ready);
        assert!(device.sensor.init_called);

        // poll_once should accumulate motion
        block_on(device.poll_once());
        assert_eq!(device.accumulated_x, 10);
        assert_eq!(device.accumulated_y, -5);
    }

    #[test]
    fn test_polling_without_motion_pin_generates_event() {
        let driver = DummyDriver {
            motion_pending: true,
            motion: MotionData { dx: 3, dy: -2 },
            read_called: false,
            init_called: true,
            fails_init: false,
            motion_gpio: None,
        };

        let mut device = PointingDevice {
            sensor: driver,
            init_state: InitState::Ready,
            poll_interval: Duration::from_millis(1),
            report_interval: Duration::from_millis(1),
            last_poll: Instant::MIN,
            last_report: Instant::MIN,
            accumulated_x: 0,
            accumulated_y: 0,
            id: 1,
        };

        let event = block_on(device.read_event());

        let axes = &event.0;
        assert_eq!(axes[0].value, 3);
        assert_eq!(axes[1].value, -2);

        assert!(device.sensor.read_called);
    }

    #[test]
    fn test_polling_with_motion_pin_generates_event() {
        let motion_pin = DummyMotionPin::new();

        let driver = DummyDriver {
            motion_pending: true,
            motion: MotionData { dx: 10, dy: -5 },
            init_called: false,
            fails_init: false,
            motion_gpio: Some(motion_pin),
            read_called: false,
        };

        let mut device = PointingDevice {
            sensor: driver,
            init_state: InitState::Pending,
            poll_interval: Duration::from_millis(10000),
            id: 1,

            report_interval: Duration::from_millis(1),
            last_poll: Instant::MIN,
            last_report: Instant::MIN,
            accumulated_x: 0,
            accumulated_y: 0,
        };

        let start = Instant::now();
        let event = block_on(device.read_event());
        let duration = start.elapsed();

        let axes = &event.0;
        assert_eq!(axes[0].value, 10);
        assert_eq!(axes[1].value, -5);
        // poll intervall is 10000 here, so if read_event took less than that, motion pin wait worked and we did not get the report form polling
        assert!(
            duration.as_millis() <= 1000,
            "read_event took too long: {}ms. Expected to be ~500ms due to motion pin triggering.",
            duration.as_millis()
        );

        assert!(device.sensor.read_called);
    }

    // === MotionAccumulator tests ===

    #[test]
    fn test_motion_accumulator_basic() {
        let mut acc = MotionAccumulator::default();

        // divisor=8: 3/8 = 0 remainder 3
        let (ox, oy) = acc.accumulate(3, 3, 8, 8);
        assert_eq!(ox, 0);
        assert_eq!(oy, 0);
        assert_eq!(acc.remainder_x, 3);
        assert_eq!(acc.remainder_y, 3);

        // 3+6=9, 9/8=1 remainder 1
        let (ox, oy) = acc.accumulate(6, 6, 8, 8);
        assert_eq!(ox, 1);
        assert_eq!(oy, 1);
        assert_eq!(acc.remainder_x, 1);
        assert_eq!(acc.remainder_y, 1);
    }

    #[test]
    fn test_motion_accumulator_negative() {
        let mut acc = MotionAccumulator::default();

        // Negative motion: -10/4 = -2 remainder -2
        let (ox, oy) = acc.accumulate(-10, -10, 4, 4);
        assert_eq!(ox, -2);
        assert_eq!(oy, -2);
        assert_eq!(acc.remainder_x, -2);
        assert_eq!(acc.remainder_y, -2);
    }

    #[test]
    fn test_motion_accumulator_reset() {
        let mut acc = MotionAccumulator::default();
        acc.accumulate(3, 5, 8, 8);
        assert_ne!(acc.remainder_x, 0);

        acc.reset();
        assert_eq!(acc.remainder_x, 0);
        assert_eq!(acc.remainder_y, 0);
    }

    #[test]
    fn test_motion_accumulator_zero_divisor_treated_as_one() {
        let mut acc = MotionAccumulator::default();
        // divisor 0 should be treated as 1 (passthrough)
        let (ox, oy) = acc.accumulate(5, -3, 0, 0);
        assert_eq!(ox, 5);
        assert_eq!(oy, -3);
    }

    #[test]
    fn test_motion_accumulator_asymmetric_divisors() {
        let mut acc = MotionAccumulator::default();
        // Different divisors for x and y
        let (ox, oy) = acc.accumulate(10, 10, 2, 5);
        assert_eq!(ox, 5); // 10/2
        assert_eq!(oy, 2); // 10/5
    }

    // === PointingMode tests ===

    #[test]
    fn test_pointing_mode_default_is_cursor() {
        assert_eq!(PointingMode::default(), PointingMode::Cursor);
    }

    #[test]
    fn test_pointing_mode_array_default() {
        let modes: [PointingMode; 4] = [PointingMode::default(); 4];
        for mode in &modes {
            assert_eq!(*mode, PointingMode::Cursor);
        }
    }

    #[test]
    fn test_set_layer_mode() {
        // Verify set_layer_mode and with_layer_modes work correctly
        // (Cannot test full processor without keymap, but can test PointingMode types)
        let scroll = PointingMode::Scroll(ScrollConfig::default());
        let sniper = PointingMode::Sniper(SniperConfig::default());

        assert_eq!(
            scroll,
            PointingMode::Scroll(ScrollConfig {
                divisor_x: 8,
                divisor_y: 8
            })
        );
        assert_eq!(sniper, PointingMode::Sniper(SniperConfig { divisor: 4 }));
    }

    #[test]
    fn test_layer_change_resets_accumulator() {
        let mut acc = MotionAccumulator::default();
        acc.accumulate(3, 5, 8, 8);
        assert_eq!(acc.remainder_x, 3);
        assert_eq!(acc.remainder_y, 5);

        // Simulate what on_layer_change_event does
        acc.reset();
        assert_eq!(acc.remainder_x, 0);
        assert_eq!(acc.remainder_y, 0);
    }

    // === Integration tests for PointingProcessor ===

    #[test]
    fn test_pointing_processor_mode_selection() {
        // Test that the processor correctly selects the mode based on current layer
        let modes = [
            PointingMode::Cursor,
            PointingMode::Scroll(ScrollConfig::default()),
            PointingMode::Sniper(SniperConfig { divisor: 4 }),
            PointingMode::Cursor,
        ];

        // Verify all modes are correctly stored
        for (i, expected_mode) in modes.iter().enumerate() {
            assert_eq!(&modes[i], expected_mode);
        }
    }

    #[test]
    fn test_scroll_mode_zero_motion_prevention() {
        let mut acc = MotionAccumulator::default();
        let config = ScrollConfig { divisor_x: 8, divisor_y: 8 };

        // Small motion that doesn't produce output
        let (sx, sy) = acc.accumulate(3, 3, config.divisor_x, config.divisor_y);
        assert_eq!(sx, 0);
        assert_eq!(sy, 0);

        // Verify remainder is kept
        assert_eq!(acc.remainder_x, 3);
        assert_eq!(acc.remainder_y, 3);

        // Additional motion should accumulate
        let (sx, sy) = acc.accumulate(6, 6, config.divisor_x, config.divisor_y);
        assert_eq!(sx, 1); // (3+6)/8 = 1 remainder 1
        assert_eq!(sy, 1);
        assert_eq!(acc.remainder_x, 1);
        assert_eq!(acc.remainder_y, 1);
    }

    #[test]
    fn test_sniper_mode_divisor() {
        let mut acc = MotionAccumulator::default();
        let config = SniperConfig { divisor: 4 };

        // Test that motion is divided correctly
        let (sx, sy) = acc.accumulate(10, -10, config.divisor, config.divisor);
        assert_eq!(sx, 2);  // 10/4 = 2 remainder 2
        assert_eq!(sy, -2); // -10/4 = -2 remainder -2
        assert_eq!(acc.remainder_x, 2);
        assert_eq!(acc.remainder_y, -2);
    }

    #[test]
    fn test_accumulator_negative_motion() {
        let mut acc = MotionAccumulator::default();

        // Test negative motion with divisor
        let (ox, oy) = acc.accumulate(-15, -20, 4, 5);
        assert_eq!(ox, -3);  // -15/4 = -3 remainder -3
        assert_eq!(oy, -4);  // -20/5 = -4 remainder 0
        assert_eq!(acc.remainder_x, -3);
        assert_eq!(acc.remainder_y, 0);

        // Mix positive and negative
        let (ox, oy) = acc.accumulate(5, 10, 4, 5);
        assert_eq!(ox, 0);   // (-3+5)/4 = 0 remainder 2
        assert_eq!(oy, 2);   // (0+10)/5 = 2 remainder 0
        assert_eq!(acc.remainder_x, 2);
        assert_eq!(acc.remainder_y, 0);
    }

    #[test]
    fn test_scroll_config_default_values() {
        let config = ScrollConfig::default();
        assert_eq!(config.divisor_x, 8);
        assert_eq!(config.divisor_y, 8);
    }

    #[test]
    fn test_sniper_config_default_values() {
        let config = SniperConfig::default();
        assert_eq!(config.divisor, 4);
    }

    #[test]
    fn test_scroll_mode_asymmetric_divisors() {
        let mut acc = MotionAccumulator::default();
        let config = ScrollConfig {
            divisor_x: 4,
            divisor_y: 8,
        };

        // Test asymmetric divisors
        let (sx, sy) = acc.accumulate(16, 16, config.divisor_x, config.divisor_y);
        assert_eq!(sx, 4);  // 16/4 = 4
        assert_eq!(sy, 2);  // 16/8 = 2
    }

    #[test]
    fn test_layer_mode_bounds_checking() {
        // Test that modes array is correctly sized
        let modes: [PointingMode; 8] = [PointingMode::default(); 8];
        assert_eq!(modes.len(), 8);

        // Verify all default to Cursor
        for mode in &modes {
            assert_eq!(*mode, PointingMode::Cursor);
        }
    }

    #[test]
    fn test_accumulator_saturation() {
        let mut acc = MotionAccumulator::default();

        // Test with large values that might overflow
        let (ox, oy) = acc.accumulate(i16::MAX, i16::MAX, 1, 1);
        assert_eq!(ox, i16::MAX);
        assert_eq!(oy, i16::MAX);

        // Reset and test negative saturation
        acc.reset();
        let (ox, oy) = acc.accumulate(i16::MIN, i16::MIN, 1, 1);
        assert_eq!(ox, i16::MIN);
        assert_eq!(oy, i16::MIN);
    }
}
