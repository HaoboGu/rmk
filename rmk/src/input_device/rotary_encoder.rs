//! General rotary encoder
//!
//! The rotary encoder implementation is adapted from: <https://github.com/leshow/rotary-encoder-hal/blob/master/src/lib.rs>
use core::cell::RefCell;

use embedded_hal::digital::InputPin;
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};
use usbd_hid::descriptor::{MediaKeyboardReport, MouseReport};

use super::{InputDevice, InputProcessor, ProcessResult};
use crate::action::{Action, KeyAction};
use crate::channel::KEYBOARD_REPORT_CHANNEL;
use crate::event::{Event, RotaryEncoderEvent};
use crate::hid::Report;
use crate::keycode::{ConsumerKey, KeyCode};
use crate::keymap::KeyMap;
use crate::descriptor::KeyboardReport;

/// Holds current/old state and both [`InputPin`](https://docs.rs/embedded-hal/latest/embedded_hal/digital/trait.InputPin.html)
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct RotaryEncoder<A, B, P> {
    pin_a: A,
    pin_b: B,
    state: u8,
    phase: P,
    /// The index of the rotary encoder
    id: u8,
}

/// The encoder direction is either `Clockwise`, `CounterClockwise`, or `None`
#[derive(Serialize, Deserialize, Clone, Copy, Debug, MaxSize, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Direction {
    /// A clockwise turn
    Clockwise,
    /// A counterclockwise turn
    CounterClockwise,
    /// No change
    None,
}

/// Allows customizing which Quadrature Phases should be considered movements
/// and in which direction or ignored.
pub trait Phase {
    /// Given the current state `s`, return the direction.
    fn direction(&mut self, s: u8) -> Direction;
}

/// Default implementation of `Phase`.
pub struct DefaultPhase;

/// The useful values of `s` are:
/// - 0b0001 | 0b0111 | 0b1000 | 0b1110
/// - 0b0010 | 0b0100 | 0b1011 | 0b1101
impl Phase for DefaultPhase {
    fn direction(&mut self, s: u8) -> Direction {
        match s {
            0b0001 | 0b0111 | 0b1000 | 0b1110 => Direction::Clockwise,
            0b0010 | 0b0100 | 0b1011 | 0b1101 => Direction::CounterClockwise,
            _ => Direction::None,
        }
    }
}

/// Phase implementation for E8H7 encoder
pub struct E8H7Phase;
impl Phase for E8H7Phase {
    fn direction(&mut self, s: u8) -> Direction {
        match s {
            0b0010 | 0b1101 => Direction::Clockwise,
            0b0001 | 0b1110 => Direction::CounterClockwise,
            _ => Direction::None,
        }
    }
}

/// Phase implementation based on configurable resolution
pub struct ResolutionPhase {
    resolution: u8,
    lut: [i8; 16],
    pulses: i8,
}

impl ResolutionPhase {
    pub fn new(resolution: u8, reverse: bool) -> Self {
        // This lookup table is based on the QMK implementation
        // Each entry corresponds to a state transition and provides +1, -1, or 0 pulse
        let mut lut = [0, -1, 1, 0, 1, 0, 0, -1, -1, 0, 0, 1, 0, 1, -1, 0];
        if reverse {
            lut = lut.map(|x| x * -1);
        }
        Self {
            resolution,
            lut,
            pulses: 0,
        }
    }
}

impl Phase for ResolutionPhase {
    fn direction(&mut self, s: u8) -> Direction {
        // Only proceed if there was a state change
        if (s & 0xC) != (s & 0x3) {
            // Add pulse value from the lookup table
            self.pulses += self.lut[s as usize & 0xF];
            // Check if we've reached the resolution threshold
            if self.pulses >= self.resolution as i8 {
                self.pulses %= self.resolution as i8;
                return Direction::CounterClockwise;
            } else if self.pulses <= -(self.resolution as i8) {
                self.pulses %= self.resolution as i8;
                return Direction::Clockwise;
            }
        }

        Direction::None
    }
}

impl<A, B> RotaryEncoder<A, B, DefaultPhase>
where
    A: InputPin,
    B: InputPin,
{
    /// Accepts two [`InputPin`](https://docs.rs/embedded-hal/latest/embedded_hal/digital/trait.InputPin.html)s, these will be read on every `update()`.
    pub fn new(pin_a: A, pin_b: B, id: u8) -> Self {
        Self {
            pin_a,
            pin_b,
            state: 0u8,
            phase: DefaultPhase,
            id,
        }
    }
}

/// Create a resolution-based rotary encoder
impl<A, B> RotaryEncoder<A, B, ResolutionPhase>
where
    A: InputPin,
    B: InputPin,
{
    /// Creates a new encoder with the specified resolution
    pub fn with_resolution(pin_a: A, pin_b: B, resolution: u8, reverse: bool, id: u8) -> Self {
        Self {
            pin_a,
            pin_b,
            state: 0u8,
            phase: ResolutionPhase::new(resolution, reverse),
            id,
        }
    }
}

impl<A: InputPin, B: InputPin, P: Phase> RotaryEncoder<A, B, P> {
    /// Accepts two [`InputPin`](https://docs.rs/embedded-hal/latest/embedded_hal/digital/trait.InputPin.html)s, these will be read on every `update()`, while using `phase` to determine the direction.
    pub fn with_phase(pin_a: A, pin_b: B, phase: P, id: u8) -> Self {
        Self {
            pin_a,
            pin_b,
            state: 0u8,
            phase,
            id,
        }
    }

    /// Call `update` to evaluate the next state of the encoder, propagates errors from `InputPin` read
    pub fn update(&mut self) -> Direction {
        // use mask to get previous state value
        let mut s = self.state & 0b11;

        let (a_is_low, b_is_low) = (self.pin_a.is_low(), self.pin_b.is_low());

        // move in the new state
        match a_is_low {
            Ok(true) => s |= 0b0100,
            Ok(false) => {}
            Err(_) => return Direction::None,
        }
        match b_is_low {
            Ok(true) => s |= 0b1000,
            Ok(false) => {}
            Err(_) => return Direction::None,
        }

        // move new state in
        self.state = s >> 2;

        // Use the phase implementation
        self.phase.direction(s)
    }

    /// Returns a reference to the first pin. Can be used to clear interrupt.
    pub fn pin_a(&mut self) -> &mut A {
        &mut self.pin_a
    }

    /// Returns a reference to the second pin. Can be used to clear interrupt.
    pub fn pin_b(&mut self) -> &mut B {
        &mut self.pin_b
    }

    /// Returns a reference to both pins. Can be used to clear interrupt.
    pub fn pins(&mut self) -> (&mut A, &mut B) {
        (&mut self.pin_a, &mut self.pin_b)
    }

    /// Consumes this `Rotary`, returning the underlying pins `A` and `B`.
    pub fn into_inner(self) -> (A, B) {
        (self.pin_a, self.pin_b)
    }
}

impl<
        #[cfg(feature = "async_matrix")] A: InputPin + Wait,
        #[cfg(not(feature = "async_matrix"))] A: InputPin,
        #[cfg(feature = "async_matrix")] B: InputPin + Wait,
        #[cfg(not(feature = "async_matrix"))] B: InputPin,
        P: Phase,
    > InputDevice for RotaryEncoder<A, B, P>
{
    async fn read_event(&mut self) -> Event {
        // Read until a valid rotary encoder event is detected
        loop {
            #[cfg(feature = "async_matrix")]
            {
                let (pin_a, pin_b) = self.pins();
                embassy_futures::select::select(pin_a.wait_for_any_edge(), pin_b.wait_for_any_edge()).await;
            }

            let direction = self.update();

            if direction != Direction::None {
                return Event::RotaryEncoder(RotaryEncoderEvent { id: self.id, direction });
            }
        }
    }
}

/// Rotary encoder event processor
pub struct RotaryEncoderProcessor<
    'a,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
> {
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    RotaryEncoderProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    pub fn new(keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>) -> Self {
        Self { keymap }
    }
}

// FIXME: now the encoder cannot process complicate key actions, because it's not worth to re-implement them again for encoders.
// The solution might be separate `Keyboard` to the `Keyboard` device part and a `KeyManager`
// The `Keyboard` part is responsible for getting `KeyAction`, and the `KeyManager` is responsible for processing all the key actions, from `Keyboard`, `Encoder`, etc.
impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    InputProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
    for RotaryEncoderProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    async fn process(&mut self, event: Event) -> ProcessResult {
        match event {
            Event::RotaryEncoder(e) => {
                let action = if let Some(encoder_action) = self.get_keymap().borrow().get_encoder_with_layer_cache(e) {
                    match e.direction {
                        Direction::Clockwise => encoder_action.clockwise(),
                        Direction::CounterClockwise => encoder_action.counter_clockwise(),
                        Direction::None => return ProcessResult::Stop,
                    }
                } else {
                    return ProcessResult::Stop;
                };

                // Accept only limited keys for rotary encoder
                if let KeyAction::Single(Action::Key(keycode)) = action {
                    match keycode {
                        k if keycode.is_consumer() => {
                            self.tap_media_key(k.as_consumer_control_usage_id()).await;
                        }
                        KeyCode::MouseWheelUp => {
                            self.send_report(Report::MouseReport(MouseReport {
                                buttons: 0,
                                x: 0,
                                y: 0,
                                wheel: 1,
                                pan: 0,
                            }))
                            .await;
                            embassy_time::Timer::after_millis(2).await;
                            self.send_report(Report::MouseReport(MouseReport {
                                buttons: 0,
                                x: 0,
                                y: 0,
                                wheel: 0,
                                pan: 0,
                            }))
                            .await;
                        }
                        KeyCode::MouseWheelDown => {
                            self.send_report(Report::MouseReport(MouseReport {
                                buttons: 0,
                                x: 0,
                                y: 0,
                                wheel: -1,
                                pan: 0,
                            }))
                            .await;
                            embassy_time::Timer::after_millis(2).await;
                            self.send_report(Report::MouseReport(MouseReport {
                                buttons: 0,
                                x: 0,
                                y: 0,
                                wheel: 0,
                                pan: 0,
                            }))
                            .await;
                        }
                        // Horizontal scrolling
                        KeyCode::MouseWheelLeft => {
                            self.send_report(Report::MouseReport(MouseReport {
                                buttons: 0,
                                x: 0,
                                y: 0,
                                wheel: 0,
                                pan: -1,
                            }))
                            .await;
                            embassy_time::Timer::after_millis(2).await;
                            self.send_report(Report::MouseReport(MouseReport {
                                buttons: 0,
                                x: 0,
                                y: 0,
                                wheel: 0,
                                pan: 0,
                            }))
                            .await;
                        }
                        KeyCode::MouseWheelRight => {
                            self.send_report(Report::MouseReport(MouseReport {
                                buttons: 0,
                                x: 0,
                                y: 0,
                                wheel: 0,
                                pan: 1,
                            }))
                            .await;
                            embassy_time::Timer::after_millis(2).await;
                            self.send_report(Report::MouseReport(MouseReport {
                                buttons: 0,
                                x: 0,
                                y: 0,
                                wheel: 0,
                                pan: 0,
                            }))
                            .await;
                        }
                        k if keycode.is_basic() => {
                            self.tap_key(k).await;
                        }
                        _ => (),
                    }
                }

                ProcessResult::Stop
            }
            _ => ProcessResult::Continue(event),
        }
    }

    async fn send_report(&self, report: Report) {
        KEYBOARD_REPORT_CHANNEL.sender().send(report).await
    }

    fn get_keymap(&self) -> &RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>> {
        self.keymap
    }
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    RotaryEncoderProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    async fn tap_media_key(&self, media: ConsumerKey) {
        self.send_report(Report::MediaKeyboardReport(MediaKeyboardReport {
            usage_id: media as u16,
        }))
        .await;
        embassy_time::Timer::after_millis(2).await;
        self.send_report(Report::MediaKeyboardReport(MediaKeyboardReport { usage_id: 0 }))
            .await;
    }

    // Send a keycode report for a single key
    async fn tap_key(&self, keycode: KeyCode) {
        self.send_report(Report::KeyboardReport(KeyboardReport {
            modifier: 0,
            reserved: 0,
            leds: 0,
            keycodes: [keycode as u8, 0, 0, 0, 0, 0],
        }))
        .await;
        embassy_time::Timer::after_millis(2).await;
        self.send_report(Report::KeyboardReport(KeyboardReport {
            modifier: 0,
            reserved: 0,
            leds: 0,
            keycodes: [0u8; 6],
        }))
        .await;
    }
}

#[cfg(test)]
mod test {
    use super::*;
    // Init logger for tests

    #[ctor::ctor]
    fn init_log() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();
    }

    #[test]
    fn test_resolutin_phase() {
        // Check with E8H7 phase
        let mut default_phase = E8H7Phase {};
        let mut resolution_phase = ResolutionPhase::new(2, true);
        // Clockwise sequence
        for item in [0b100, 0b1101, 0b1011, 0b10] {
            let d = default_phase.direction(item);
            let d2 = resolution_phase.direction(item);
            info!("item: {:b}, {:?} {:?}", item, d, d2);
            assert_eq!(d, d2);
        }
        // Counterclockwise sequence
        for item in [0b1000, 0b1110, 0b111, 0b1] {
            let d = default_phase.direction(item);
            let d2 = resolution_phase.direction(item);
            info!("item: {:b}, {:?} {:?}", item, d, d2);
            assert_eq!(d, d2);
        }

        // Check with default phase
        let mut default_phase = DefaultPhase {};
        let mut resolution_phase = ResolutionPhase::new(1, false);
        for item in 0u8..16 {
            let d = default_phase.direction(item);
            let d2 = resolution_phase.direction(item);
            info!("item: {:b}, {:?} {:?}", item, d, d2);
            assert_eq!(d, d2);
        }
    }
}
