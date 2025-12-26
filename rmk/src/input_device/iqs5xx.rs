//! IQS5xx trackpad driver integration for RMK.

use core::cell::RefCell;

use embassy_time::{Duration, Timer};
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::i2c::I2c;
use rmk_driver_azoteq_iqs5xx::{Event as IqsEvent, Iqs5xx};
use usbd_hid::descriptor::MouseReport;

use crate::channel::KEYBOARD_REPORT_CHANNEL;
use crate::event::{Axis, AxisEvent, AxisValType, Event};
use crate::hid::Report as RmkReport;
use crate::input_device::{InputDevice, InputProcessor, ProcessResult};
use crate::keymap::KeyMap;

pub use rmk_driver_azoteq_iqs5xx::{
    Event as Iqs5xxEvent, Iqs5xx as Iqs5xxDriver, Iqs5xxConfig, Report as Iqs5xxReport,
};

const CUSTOM_TAG_CLICK: u8 = 1;
const CUSTOM_TAG_BUTTON: u8 = 2;
const BUTTON_LEFT: u8 = 1;
const BUTTON_RIGHT: u8 = 2;

#[derive(Debug, Clone, Copy)]
pub struct Iqs5xxProcessorConfig {
    pub scroll_divisor: i16,
    pub natural_scroll_x: bool,
    pub natural_scroll_y: bool,
}

impl Default for Iqs5xxProcessorConfig {
    fn default() -> Self {
        Self {
            scroll_divisor: 32,
            natural_scroll_x: false,
            natural_scroll_y: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InitState {
    Pending,
    Initializing(u8),
    Ready,
    Failed,
}

pub struct Iqs5xxDevice<I2C, RDY, RST>
where
    I2C: I2c,
    RDY: InputPin,
    RST: OutputPin,
{
    driver: Iqs5xx<I2C, RDY, RST>,
    init_state: InitState,
    poll_interval: Duration,
    hold_active: bool,
}

impl<I2C, RDY, RST> Iqs5xxDevice<I2C, RDY, RST>
where
    I2C: I2c,
    RDY: InputPin,
    RST: OutputPin,
{
    const MAX_INIT_RETRIES: u8 = 3;

    pub fn new(i2c: I2C, rdy: RDY, rst: RST, config: Iqs5xxConfig) -> Self {
        Self {
            driver: Iqs5xx::new(i2c, Some(rdy), Some(rst), config),
            init_state: InitState::Pending,
            poll_interval: Duration::from_millis(5),
            hold_active: false,
        }
    }

    pub fn with_poll_interval(i2c: I2C, rdy: RDY, rst: RST, config: Iqs5xxConfig, poll_interval_ms: u64) -> Self {
        Self {
            driver: Iqs5xx::new(i2c, Some(rdy), Some(rst), config),
            init_state: InitState::Pending,
            poll_interval: Duration::from_millis(poll_interval_ms),
            hold_active: false,
        }
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

        if let InitState::Initializing(retry) = self.init_state {
            if self.driver.init().await.is_ok() {
                self.init_state = InitState::Ready;
                return true;
            }

            if retry + 1 >= Self::MAX_INIT_RETRIES {
                self.init_state = InitState::Failed;
                return false;
            }

            self.init_state = InitState::Initializing(retry + 1);
            Timer::after(Duration::from_millis(100)).await;
        }

        false
    }

    fn custom_click(button: u8) -> Event {
        let mut data = [0u8; 16];
        data[0] = CUSTOM_TAG_CLICK;
        data[1] = button;
        Event::Custom(data)
    }

    fn custom_button(button: u8, pressed: bool) -> Event {
        let mut data = [0u8; 16];
        data[0] = CUSTOM_TAG_BUTTON;
        data[1] = button;
        data[2] = pressed as u8;
        Event::Custom(data)
    }
}

impl<I2C, RDY, RST> InputDevice for Iqs5xxDevice<I2C, RDY, RST>
where
    I2C: I2c,
    RDY: InputPin,
    RST: OutputPin,
{
    async fn read_event(&mut self) -> Event {
        loop {
            Timer::after(self.poll_interval).await;

            if self.init_state != InitState::Ready && !self.try_init().await {
                continue;
            }

            let report = match self.driver.try_read_report().await {
                Ok(Some(report)) => report,
                Ok(None) => continue,
                Err(_) => continue,
            };

            if report.sys_info0 & rmk_driver_azoteq_iqs5xx::registers::SYSTEM_INFO_0_SHOW_RESET != 0 {
                let _ = self.driver.acknowledge_reset().await;
                continue;
            }

            let hold_now = (report.events0 & rmk_driver_azoteq_iqs5xx::registers::GESTURE_PRESS_HOLD) != 0;
            if hold_now && !self.hold_active {
                self.hold_active = true;
                return Self::custom_button(BUTTON_LEFT, true);
            }
            if !hold_now && self.hold_active {
                self.hold_active = false;
                return Self::custom_button(BUTTON_LEFT, false);
            }

            match IqsEvent::from_report(&report) {
                IqsEvent::None | IqsEvent::Invalid => continue,
                IqsEvent::Move { x, y } => {
                    if x == 0 && y == 0 {
                        continue;
                    }
                    return Event::Touchpad(crate::event::TouchpadEvent {
                        finger: 0,
                        axis: [
                            AxisEvent {
                                typ: AxisValType::Rel,
                                axis: Axis::X,
                                value: x,
                            },
                            AxisEvent {
                                typ: AxisValType::Rel,
                                axis: Axis::Y,
                                value: y,
                            },
                            AxisEvent {
                                typ: AxisValType::Rel,
                                axis: Axis::Z,
                                value: 0,
                            },
                        ],
                    });
                }
                IqsEvent::Scroll { x, y } => {
                    if x == 0 && y == 0 {
                        continue;
                    }
                    return Event::Touchpad(crate::event::TouchpadEvent {
                        finger: 0,
                        axis: [
                            AxisEvent {
                                typ: AxisValType::Rel,
                                axis: Axis::H,
                                value: x,
                            },
                            AxisEvent {
                                typ: AxisValType::Rel,
                                axis: Axis::V,
                                value: y,
                            },
                            AxisEvent {
                                typ: AxisValType::Rel,
                                axis: Axis::Z,
                                value: 0,
                            },
                        ],
                    });
                }
                IqsEvent::SingleTap { .. } => return Self::custom_click(BUTTON_LEFT),
                IqsEvent::TwoFingerTap => return Self::custom_click(BUTTON_RIGHT),
                _ => continue,
            }
        }
    }
}

pub struct Iqs5xxProcessor<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize> {
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    config: Iqs5xxProcessorConfig,
    buttons: u8,
    scroll_x_acc: i16,
    scroll_y_acc: i16,
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    Iqs5xxProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    pub fn new(
        keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
        config: Iqs5xxProcessorConfig,
    ) -> Self {
        Self {
            keymap,
            config,
            buttons: 0,
            scroll_x_acc: 0,
            scroll_y_acc: 0,
        }
    }

    async fn send_mouse_report(&self, report: MouseReport) {
        KEYBOARD_REPORT_CHANNEL.send(RmkReport::MouseReport(report)).await;
    }

    async fn send_button_state(&self) {
        let report = MouseReport {
            buttons: self.buttons,
            x: 0,
            y: 0,
            wheel: 0,
            pan: 0,
        };
        self.send_mouse_report(report).await;
    }

    async fn handle_click(&mut self, button: u8) {
        self.buttons |= button_mask(button);
        self.send_button_state().await;
        self.buttons &= !button_mask(button);
        self.send_button_state().await;
    }

    async fn handle_button(&mut self, button: u8, pressed: bool) {
        if pressed {
            self.buttons |= button_mask(button);
        } else {
            self.buttons &= !button_mask(button);
        }
        self.send_button_state().await;
    }

    async fn handle_motion(&self, x: i16, y: i16) {
        let report = MouseReport {
            buttons: self.buttons,
            x: x.clamp(i8::MIN as i16, i8::MAX as i16) as i8,
            y: y.clamp(i8::MIN as i16, i8::MAX as i16) as i8,
            wheel: 0,
            pan: 0,
        };
        self.send_mouse_report(report).await;
    }

    async fn handle_scroll(&mut self, mut x: i16, mut y: i16) {
        if self.config.natural_scroll_x {
            x = -x;
        }
        if self.config.natural_scroll_y {
            y = -y;
        }

        self.scroll_x_acc = self.scroll_x_acc.saturating_add(x);
        self.scroll_y_acc = self.scroll_y_acc.saturating_add(y);

        let mut pan = 0i16;
        let mut wheel = 0i16;
        let div = self.config.scroll_divisor.max(1);

        if self.scroll_x_acc.abs() >= div {
            pan = self.scroll_x_acc / div;
            self.scroll_x_acc %= div;
        }
        if self.scroll_y_acc.abs() >= div {
            wheel = self.scroll_y_acc / div;
            self.scroll_y_acc %= div;
        }

        if pan != 0 || wheel != 0 {
            let report = MouseReport {
                buttons: self.buttons,
                x: 0,
                y: 0,
                wheel: wheel.clamp(i8::MIN as i16, i8::MAX as i16) as i8,
                pan: pan.clamp(i8::MIN as i16, i8::MAX as i16) as i8,
            };
            self.send_mouse_report(report).await;
        }
    }
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    InputProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER> for Iqs5xxProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    async fn process(&mut self, event: Event) -> ProcessResult {
        match event {
            Event::Touchpad(tp) => {
                let mut x = 0i16;
                let mut y = 0i16;
                let mut h = 0i16;
                let mut v = 0i16;

                for axis_event in tp.axis.iter() {
                    match axis_event.axis {
                        Axis::X => x = axis_event.value,
                        Axis::Y => y = axis_event.value,
                        Axis::H => h = axis_event.value,
                        Axis::V => v = axis_event.value,
                        _ => {}
                    }
                }

                if h != 0 || v != 0 {
                    self.handle_scroll(h, v).await;
                    return ProcessResult::Stop;
                }

                if x != 0 || y != 0 {
                    self.handle_motion(x, y).await;
                    return ProcessResult::Stop;
                }

                ProcessResult::Stop
            }
            Event::Custom(data) => match data[0] {
                CUSTOM_TAG_CLICK => {
                    self.handle_click(data[1]).await;
                    ProcessResult::Stop
                }
                CUSTOM_TAG_BUTTON => {
                    self.handle_button(data[1], data[2] != 0).await;
                    ProcessResult::Stop
                }
                _ => ProcessResult::Continue(event),
            },
            _ => ProcessResult::Continue(event),
        }
    }

    async fn send_report(&self, report: RmkReport) {
        KEYBOARD_REPORT_CHANNEL.send(report).await;
    }

    fn get_keymap(&self) -> &RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>> {
        self.keymap
    }
}

fn button_mask(button: u8) -> u8 {
    match button {
        BUTTON_LEFT => 1 << 0,
        BUTTON_RIGHT => 1 << 1,
        _ => 0,
    }
}
