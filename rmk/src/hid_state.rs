use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

use bitfield_struct::bitfield;

#[bitfield(u8, order = Lsb)]
#[derive(Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HidModifiers {
    #[bits(1)]
    pub(crate) left_ctrl: bool,
    #[bits(1)]
    pub(crate) left_shift: bool,
    #[bits(1)]
    pub(crate) left_alt: bool,
    #[bits(1)]
    pub(crate) left_gui: bool,
    #[bits(1)]
    pub(crate) right_ctrl: bool,
    #[bits(1)]
    pub(crate) right_shift: bool,
    #[bits(1)]
    pub(crate) right_alt: bool,
    #[bits(1)]
    pub(crate) right_gui: bool,
}

impl BitOr for HidModifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self::from_bits(self.into_bits() | rhs.into_bits())
    }
}
impl BitAnd for HidModifiers {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self::from_bits(self.into_bits() & rhs.into_bits())
    }
}
impl Not for HidModifiers {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self::from_bits(!self.into_bits())
    }
}
impl BitAndAssign for HidModifiers {
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs;
    }
}
impl BitOrAssign for HidModifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl HidModifiers {
    pub const fn new_from(
        left_ctrl: bool,
        left_shift: bool,
        left_alt: bool,
        left_gui: bool,
        right_ctrl: bool,
        right_shift: bool,
        right_alt: bool,
        right_gui: bool,
    ) -> Self {
        Self::new()
            .with_left_ctrl(left_ctrl)
            .with_left_shift(left_shift)
            .with_left_alt(left_alt)
            .with_left_gui(left_gui)
            .with_right_ctrl(right_ctrl)
            .with_right_shift(right_shift)
            .with_right_alt(right_alt)
            .with_right_gui(right_gui)
    }
}

#[bitfield(u8, order = Lsb)]
#[derive(Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HidMouseButtons {
    #[bits(1)]
    pub(crate) button1: bool, //left
    #[bits(1)]
    pub(crate) button2: bool, //right
    #[bits(1)]
    pub(crate) button3: bool, //middle
    #[bits(1)]
    pub(crate) button4: bool,
    #[bits(1)]
    pub(crate) button5: bool,
    #[bits(1)]
    pub(crate) button6: bool,
    #[bits(1)]
    pub(crate) button7: bool,
    #[bits(1)]
    pub(crate) button8: bool,
}

impl BitOr for HidMouseButtons {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self::from_bits(self.into_bits() | rhs.into_bits())
    }
}
impl BitAnd for HidMouseButtons {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self::from_bits(self.into_bits() & rhs.into_bits())
    }
}
impl Not for HidMouseButtons {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self::from_bits(!self.into_bits())
    }
}
impl BitAndAssign for HidMouseButtons {
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs;
    }
}
impl BitOrAssign for HidMouseButtons {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl HidMouseButtons {
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
