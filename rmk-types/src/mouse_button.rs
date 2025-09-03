//! Mouse button state and operations.
//!
//! This module handles mouse button combinations and states, supporting up to
//! 8 mouse buttons.
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

use bitfield_struct::bitfield;

/// Mouse buttons
#[bitfield(u8, order = Lsb, defmt = cfg(feature = "defmt"))]
#[derive(Eq, PartialEq)]

pub struct MouseButtons {
    #[bits(1)]
    pub button1: bool, //left
    #[bits(1)]
    pub button2: bool, //right
    #[bits(1)]
    pub button3: bool, //middle
    #[bits(1)]
    pub button4: bool,
    #[bits(1)]
    pub button5: bool,
    #[bits(1)]
    pub button6: bool,
    #[bits(1)]
    pub button7: bool,
    #[bits(1)]
    pub button8: bool,
}

impl BitOr for MouseButtons {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self::from_bits(self.into_bits() | rhs.into_bits())
    }
}
impl BitAnd for MouseButtons {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self::from_bits(self.into_bits() & rhs.into_bits())
    }
}
impl Not for MouseButtons {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self::from_bits(!self.into_bits())
    }
}
impl BitAndAssign for MouseButtons {
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs;
    }
}
impl BitOrAssign for MouseButtons {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl MouseButtons {
    pub const BUTTON1: Self = Self::new().with_button1(true);
    pub const BUTTON2: Self = Self::new().with_button2(true);
    pub const BUTTON3: Self = Self::new().with_button3(true);
    pub const BUTTON4: Self = Self::new().with_button4(true);
    pub const BUTTON5: Self = Self::new().with_button5(true);
    pub const BUTTON6: Self = Self::new().with_button6(true);
    pub const BUTTON7: Self = Self::new().with_button7(true);
    pub const BUTTON8: Self = Self::new().with_button8(true);

    pub const fn new_from(
        button1: bool,
        button2: bool,
        button3: bool,
        button4: bool,
        button5: bool,
        button6: bool,
        button7: bool,
        button8: bool,
    ) -> Self {
        Self::new()
            .with_button1(button1)
            .with_button2(button2)
            .with_button3(button3)
            .with_button4(button4)
            .with_button5(button5)
            .with_button6(button6)
            .with_button7(button7)
            .with_button8(button8)
    }
}
