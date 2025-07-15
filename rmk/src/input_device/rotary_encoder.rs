//! General rotary encoder
//!
//! The rotary encoder implementation is adapted from: <https://github.com/leshow/rotary-encoder-hal/blob/master/src/lib.rs>
use embedded_hal::digital::InputPin;
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

use super::InputDevice;
use crate::event::{Event, KeyboardEvent};

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
    /// The last action of the rotary encoder.
    /// When it's not `None`, the rotary encoder needs to emit a release event.
    last_action: Option<Direction>,
}

/// The encoder direction is either `Clockwise`, `CounterClockwise`, or `None`
#[derive(Serialize, Deserialize, Clone, Copy, Debug, MaxSize, PartialEq, Eq)]
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
            lut = lut.map(|x| -x);
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
            last_action: None,
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
            last_action: None,
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
            last_action: None,
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
        if let Some(last_action) = self.last_action {
            embassy_time::Timer::after_millis(5).await;
            let e = Event::Key(KeyboardEvent::rotary_encoder(self.id, last_action, false));
            self.last_action = None;
            return e;
        }

        loop {
            #[cfg(feature = "async_matrix")]
            {
                let (pin_a, pin_b) = self.pins();
                embassy_futures::select::select(pin_a.wait_for_any_edge(), pin_b.wait_for_any_edge()).await;
            }

            let direction = self.update();

            if direction != Direction::None {
                self.last_action = Some(direction);
                return Event::Key(KeyboardEvent::rotary_encoder(self.id, direction, true));
            }

            #[cfg(not(feature = "async_matrix"))]
            {
                // Wait for 20ms to avoid busy loop
                embassy_time::Timer::after_millis(20).await;
            }
        }
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
