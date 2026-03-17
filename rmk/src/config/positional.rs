#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[derive(Default)]
pub enum Hand {
    #[default]
    Unknown,
    Left,
    Right,
    Bilateral,
}

/// Configuration that's only related to the key's position.
///
/// Now only the hand information is included.
/// In the future more fields can be added here for the future configurator GUI, such as
/// - physical key position and orientation
/// - key size,
/// - key shape,
/// - backlight sequence number, etc.
///
/// IDEA: For Keyboards with low memory, these should be compile time constants to save RAM?
#[derive(Debug)]
pub struct PositionalConfig<const ROW: usize, const COL: usize> {
    pub hand: [[Hand; COL]; ROW],
}

impl<const ROW: usize, const COL: usize> Default for PositionalConfig<ROW, COL> {
    fn default() -> Self {
        Self {
            hand: [[Hand::default(); COL]; ROW],
        }
    }
}

impl<const ROW: usize, const COL: usize> PositionalConfig<ROW, COL> {
    pub fn new(hand: [[Hand; COL]; ROW]) -> Self {
        Self { hand }
    }
}
