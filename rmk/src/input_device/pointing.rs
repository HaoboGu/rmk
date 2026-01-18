//! Common functionality across pointing devices

use core::cell::RefCell;

use embassy_time::{Duration, Instant, Timer};
use usbd_hid::descriptor::MouseReport;

#[cfg(feature = "controller")]
use crate::channel::ControllerSub;
use crate::channel::KEYBOARD_REPORT_CHANNEL;
use crate::event::{Axis, AxisEvent, AxisValType, Event};
#[cfg(feature = "controller")]
use crate::event::{ControllerEvent, PointingEvent};
use crate::hid::Report;
use crate::input_device::{InputDevice, InputProcessor, ProcessResult};
use crate::keymap::KeyMap;
use embedded_hal::digital::InputPin;
use embedded_hal_async::digital::Wait;
use futures::future::pending;

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
    /// Controller event not implement for this device
    NotImplementedError,
    /// Invalid rotational transform angle
    InvalidRotTransAngle,
}

pub trait PointingDriver {
    type MOTION: InputPin + Wait;

    async fn init(&mut self) -> Result<(), PointingDriverError>;
    async fn read_motion(&mut self) -> Result<MotionData, PointingDriverError>;
    async fn set_resolution(&mut self, _cpi: u16) -> Result<(), PointingDriverError> {
        debug!("set_resolution() is not implemented for this sensor.");
        Err(PointingDriverError::NotImplementedError)
    }
    async fn set_rot_trans_angle(&mut self, _angle: i8) -> Result<(), PointingDriverError> {
        debug!("set_rot_trans_angle() is not implemented for this sensor.");
        Err(PointingDriverError::NotImplementedError)
    }
    async fn set_liftoff_dist(&mut self, _dist: u8) -> Result<(), PointingDriverError> {
        debug!("set_liftoff_dist() is not implemented for this sensor.");
        Err(PointingDriverError::NotImplementedError)
    }
    async fn set_force_awake(&mut self, _enable: bool) -> Result<(), PointingDriverError> {
        debug!("set_force_awake() is not implemented for this sensor.");
        Err(PointingDriverError::NotImplementedError)
    }
    async fn set_invert_x(&mut self, _onoff: bool) -> Result<(), PointingDriverError> {
        debug!("set_invert_x() is not implemented for this sensor.");
        Err(PointingDriverError::NotImplementedError)
    }
    async fn set_invert_y(&mut self, _onoff: bool) -> Result<(), PointingDriverError> {
        debug!("set_invert_y() is not implemented for this sensor.");
        Err(PointingDriverError::NotImplementedError)
    }
    async fn set_swap_xy(&mut self, _onoff: bool) -> Result<(), PointingDriverError> {
        debug!("set_swap_xy() is not implemented for this sensor.");
        Err(PointingDriverError::NotImplementedError)
    }
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
/// This device returns `Event::Joystick` events with relative X/Y movement.
pub struct PointingDevice<S: PointingDriver> {
    pub sensor: S,
    pub init_state: InitState,
    pub poll_interval: Duration,
    #[cfg(feature = "controller")]
    pub controller_sub: ControllerSub,
    pub id: u8,
    pub report_interval: Duration,
    pub last_poll: Instant,
    pub last_report: Instant,
    pub accumulated_x: i32,
    pub accumulated_y: i32,
}

impl<S> PointingDevice<S>
where
    S: PointingDriver,
{
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

    fn take_report_event(&mut self) -> Option<Event> {
        if self.accumulated_x == 0 && self.accumulated_y == 0 {
            return None;
        }

        let dx = self.accumulated_x.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        let dy = self.accumulated_y.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        self.accumulated_x = 0;
        self.accumulated_y = 0;

        Some(Event::Joystick([
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

    /// Handle controller events for the pointing device
    #[cfg(feature = "controller")]
    async fn handle_controller_event(&mut self, event: ControllerEvent) -> () {
        match event {
            ControllerEvent::PointingContEvent((device_id, pointing_event)) => {
                if device_id == self.id || device_id == ALL_POINTING_DEVICES {
                    match pointing_event {
                        PointingEvent::PointingSetCpi(cpi) => {
                            debug!("PointingDevice {}: Setting CPI to: {}", self.id, cpi);
                            if let Err(e) = self.sensor.set_resolution(cpi).await {
                                warn!("PointingDevice {}: Failed to set CPI: {:?}", self.id, e);
                            }
                        }
                        PointingEvent::PointingSetPollIntervall(interval_us) => {
                            debug!(
                                "PointingDevice {}: Setting poll interval to: {}μs",
                                self.id, interval_us
                            );
                            self.poll_interval = Duration::from_micros(interval_us);
                        }
                        PointingEvent::PointingSetRotTransAngle(angle) => {
                            debug!(
                                "PointingDevice {}: Setting rotational transform angle to: {}",
                                self.id, angle
                            );
                            if let Err(e) = self.sensor.set_rot_trans_angle(angle).await {
                                warn!("PointingDevice {}: Failed to set rotation angle: {:?}", self.id, e);
                            }
                        }
                        PointingEvent::PointigSetLiftoffDist(dist) => {
                            debug!("PointingDevice {}: Setting liftoff distance to: {}", self.id, dist);
                            if let Err(e) = self.sensor.set_liftoff_dist(dist).await {
                                warn!("PointingDevice {}: Failed to set liftoff distance: {:?}", self.id, e);
                            }
                        }
                        PointingEvent::PointingSetForceAwake(enable) => {
                            debug!("PointingDevice {}: Setting force awake mode to: {}", self.id, enable);
                            if let Err(e) = self.sensor.set_force_awake(enable).await {
                                warn!("PointingDevice {}: Failed to set force awake: {:?}", self.id, e);
                            }
                        }
                        PointingEvent::PointingSetInvertX(invert) => {
                            debug!("PointingDevice {}: Setting invert X to: {}", self.id, invert);
                            if let Err(e) = self.sensor.set_invert_x(invert).await {
                                warn!("PointingDevice {}: Failed to set force awake: {:?}", self.id, e);
                            }
                        }
                        PointingEvent::PointingSetInvertY(invert) => {
                            debug!("PointingDevice {}: Setting invert Y to: {}", self.id, invert);
                            if let Err(e) = self.sensor.set_invert_y(invert).await {
                                warn!("PointingDevice {}: Failed to set force awake: {:?}", self.id, e);
                            }
                        }
                        PointingEvent::PointingSwapXY(swap) => {
                            debug!("PointingDevice {}: Setting swap X/Y to: {}", self.id, swap);
                            if let Err(e) = self.sensor.set_swap_xy(swap).await {
                                warn!("PointingDevice {}: Failed to set force awake: {:?}", self.id, e);
                            }
                        }
                    }
                }
            }
            _ => {
                // Ignore other controller events not meant for pointing device
            }
        }
    }
}

impl<S> InputDevice for PointingDevice<S>
where
    S: PointingDriver,
{
    /*
    +--------------- loop ---------------+
    ¦ poll_wait   report_wait            ¦
    ¦     ¦           ¦                  ¦
    ¦     V           V                  ¦
    ¦ poll_once()     take_report_event()¦
    ¦     ¦           ¦                  ¦
    ¦     +- accum += ¦                  ¦
    ¦                 >- Event returned  ¦
    +------------------------------------+
    */
    async fn read_event(&mut self) -> Event {
        #[cfg(feature = "controller")]
        {
            use embassy_futures::select::{Either3, select3};

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

                match select3(poll_wait, report_wait, self.controller_sub.next_message_pure()).await {
                    Either3::First(_) => {
                        self.poll_once().await;
                        self.last_poll = Instant::now();
                    }
                    Either3::Second(_) => {
                        if let Some(event) = self.take_report_event() {
                            self.last_report = Instant::now();
                            return event;
                        }
                    }
                    Either3::Third(controller_event) => {
                        self.handle_controller_event(controller_event).await;
                    }
                }
            }
        }
        #[cfg(not(feature = "controller"))]
        {
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
}

/// PointingProcessor that converts motion events to mouse reports
pub struct PointingProcessor<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize> {
    /// Reference to the keymap
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    PointingProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    /// Create a new pointing processor with default settings
    pub fn new(keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>) -> Self {
        Self { keymap }
    }

    async fn generate_report(&self, x: i16, y: i16) {
        let mouse_report = MouseReport {
            buttons: 0,
            x: x.clamp(i8::MIN as i16, i8::MAX as i16) as i8,
            y: y.clamp(i8::MIN as i16, i8::MAX as i16) as i8,
            wheel: 0,
            pan: 0,
        };
        self.send_report(Report::MouseReport(mouse_report)).await;
    }
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    InputProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER> for PointingProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    async fn process(&mut self, event: Event) -> ProcessResult {
        match event {
            Event::Joystick(axis_events) => {
                let mut x = 0i16;
                let mut y = 0i16;

                for axis_event in axis_events.iter() {
                    match axis_event.axis {
                        Axis::X => x = axis_event.value,
                        Axis::Y => y = axis_event.value,
                        _ => {}
                    }
                }

                self.generate_report(x, y).await;
                ProcessResult::Stop
            }
            _ => ProcessResult::Continue(event),
        }
    }

    async fn send_report(&self, report: Report) {
        KEYBOARD_REPORT_CHANNEL.send(report).await;
    }

    fn get_keymap(&self) -> &RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>> {
        self.keymap
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use embassy_futures::block_on;
    use embassy_time::Duration;
    use embedded_hal::digital::ErrorType;
    use embedded_hal::digital::InputPin;
    use embedded_hal_async::digital::Wait;

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

        async fn set_resolution(&mut self, _cpi: u16) -> Result<(), PointingDriverError> {
            Ok(())
        }
        async fn set_rot_trans_angle(&mut self, _angle: i8) -> Result<(), PointingDriverError> {
            Ok(())
        }
        async fn set_liftoff_dist(&mut self, _dist: u8) -> Result<(), PointingDriverError> {
            Ok(())
        }
        async fn force_awake(&mut self, _enable: bool) -> Result<(), PointingDriverError> {
            Ok(())
        }
        async fn set_invert_x(&mut self, _onoff: bool) -> Result<(), PointingDriverError> {
            Ok(())
        }
        async fn set_invert_y(&mut self, _onoff: bool) -> Result<(), PointingDriverError> {
            Ok(())
        }
        async fn swap_xy(&mut self, _onoff: bool) -> Result<(), PointingDriverError> {
            Ok(())
        }

        fn motion_pending(&mut self) -> bool {
            self.motion_pending
        }

        fn motion_gpio(&mut self) -> Option<&mut Self::MOTION> {
            self.motion_gpio.as_mut()
        }
    }

    #[cfg(feature = "controller")]
    use crate::channel::CONTROLLER_CHANNEL_FINAL_SIZE;
    #[cfg(feature = "controller")]
    use crate::event::ControllerEvent;
    #[cfg(feature = "controller")]
    use crate::{CONTROLLER_CHANNEL_PUBS, CONTROLLER_CHANNEL_SUBS};
    #[cfg(feature = "controller")]
    use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
    #[cfg(feature = "controller")]
    use embassy_sync::pubsub::{PubSubChannel, Subscriber};

    #[cfg(feature = "controller")]
    fn make_dummy_subscriber() -> Subscriber<
        'static,
        CriticalSectionRawMutex,
        ControllerEvent,
        CONTROLLER_CHANNEL_FINAL_SIZE,
        CONTROLLER_CHANNEL_SUBS,
        CONTROLLER_CHANNEL_PUBS,
        > {
            static CHANNEL: PubSubChannel<
                CriticalSectionRawMutex,
                ControllerEvent,
                CONTROLLER_CHANNEL_FINAL_SIZE,
                CONTROLLER_CHANNEL_SUBS,
                CONTROLLER_CHANNEL_PUBS,
                > = PubSubChannel::new();
            CHANNEL.subscriber().unwrap()
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
            #[cfg(feature = "controller")]
            controller_sub: make_dummy_subscriber(),
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
            #[cfg(feature = "controller")]
            controller_sub: make_dummy_subscriber(),
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
            #[cfg(feature = "controller")]
            controller_sub: make_dummy_subscriber(),
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
            #[cfg(feature = "controller")]
            controller_sub: make_dummy_subscriber(),
        };

        let event = block_on(device.read_event());

        match event {
            Event::Joystick(axes) => {
                assert_eq!(axes[0].value, 3);
                assert_eq!(axes[1].value, -2);
            }
            _ => panic!("expected joystick event"),
        }

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
            #[cfg(feature = "controller")]
            controller_sub: make_dummy_subscriber(),
            report_interval: Duration::from_millis(1),
            last_poll: Instant::MIN,
            last_report: Instant::MIN,
            accumulated_x: 0,
            accumulated_y: 0,
        };

        let start = Instant::now();
        let event = block_on(device.read_event());
        let duration = start.elapsed();

        match event {
            Event::Joystick(axes) => {
                assert_eq!(axes[0].value, 10);
                assert_eq!(axes[1].value, -5);
            }
            _ => panic!("expected joystick event"),
        }
        // poll intervall is 10000 here, so if read_event took less than that, motion pin wait worked and we did not get the report form polling
        assert!(
            duration.as_millis() <= 1000,
            "read_event took too long: {}ms. Expected to be ~500ms due to motion pin triggering.",
            duration.as_millis()
        );

        assert!(device.sensor.read_called);
    }
}
