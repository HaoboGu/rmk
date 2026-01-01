//! Common functionality across pointing devices

use core::cell::RefCell;

use embassy_time::{Duration, Timer};
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
}

pub trait PointingDriver {
    async fn init(&mut self) -> Result<(), PointingDriverError>;
    async fn read_motion(&mut self) -> Result<MotionData, PointingDriverError>;
    async fn set_resolution(&mut self, cpi: u16) -> Result<(), PointingDriverError>;
    async fn set_rot_trans_angle(&mut self, angle: i8) -> Result<(), PointingDriverError>;
    async fn set_liftoff_dist(&mut self, dist: u8) -> Result<(), PointingDriverError>;
    async fn force_awake(&mut self, enable: bool) -> Result<(), PointingDriverError>;
    async fn set_invert_x(&mut self, onoff: bool) -> Result<(), PointingDriverError>;
    async fn set_invert_y(&mut self, onoff: bool) -> Result<(), PointingDriverError>;
    async fn swap_xy(&mut self, onoff: bool) -> Result<(), PointingDriverError>;
    fn motion_pending(&mut self) -> bool;
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
                            if let Err(e) = self.sensor.force_awake(enable).await {
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
                            if let Err(e) = self.sensor.swap_xy(swap).await {
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
    async fn read_event(&mut self) -> Event {
        #[cfg(feature = "controller")]
        {
            use embassy_futures::select::{select, Either};

            loop {
                if self.init_state != InitState::Ready {
                    if !self.try_init().await {
                        Timer::after(Duration::from_millis(100)).await;
                        continue;
                    }
                }

                match select(
                    async {
                        Timer::after(self.poll_interval).await;
                        if self.sensor.motion_pending() {
                            self.sensor.read_motion().await.ok()
                        } else {
                            None
                        }
                    },
                    self.controller_sub.next_message_pure(),
                )
                .await
                {
                    Either::First(motion_result) => {
                        if let Some(motion) = motion_result {
                            if motion.dx != 0 || motion.dy != 0 {
                                return Event::Joystick([
                                    AxisEvent {
                                        typ: AxisValType::Rel,
                                        axis: Axis::X,
                                        value: motion.dx,
                                    },
                                    AxisEvent {
                                        typ: AxisValType::Rel,
                                        axis: Axis::Y,
                                        value: motion.dy,
                                    },
                                    AxisEvent {
                                        typ: AxisValType::Rel,
                                        axis: Axis::Z,
                                        value: 0,
                                    },
                                ]);
                            }
                        }
                    }
                    Either::Second(controller_event) => {
                        self.handle_controller_event(controller_event).await;
                    }
                }
            }
        }
        #[cfg(not(feature = "controller"))]
        {
            loop {
                Timer::after(self.poll_interval).await;

                if self.init_state != InitState::Ready {
                    if !self.try_init().await {
                        continue;
                    }
                }

                if !self.sensor.motion_pending() {
                    continue;
                }

                match self.sensor.read_motion().await {
                    Ok(motion) => {
                        if motion.dx != 0 || motion.dy != 0 {
                            return Event::Joystick([
                                AxisEvent {
                                    typ: AxisValType::Rel,
                                    axis: Axis::X,
                                    value: motion.dx,
                                },
                                AxisEvent {
                                    typ: AxisValType::Rel,
                                    axis: Axis::Y,
                                    value: motion.dy,
                                },
                                AxisEvent {
                                    typ: AxisValType::Rel,
                                    axis: Axis::Z,
                                    value: 0,
                                },
                            ]);
                        }
                    }
                    Err(e) => {
                        warn!("PointingDevice {}: read error: {:?}", self.id, e);
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
