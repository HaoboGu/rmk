use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

use bitfield_struct::bitfield;
use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

/// The bit representation of the modifier combination.
#[bitfield(u8, order = Lsb, defmt = cfg(feature = "defmt"))]
#[derive(Serialize, Deserialize, MaxSize, Eq, PartialEq)]

pub struct ModifierCombination {
    #[bits(1)]
    pub left_ctrl: bool,
    #[bits(1)]
    pub left_shift: bool,
    #[bits(1)]
    pub left_alt: bool,
    #[bits(1)]
    pub left_gui: bool,
    #[bits(1)]
    pub right_ctrl: bool,
    #[bits(1)]
    pub right_shift: bool,
    #[bits(1)]
    pub right_alt: bool,
    #[bits(1)]
    pub right_gui: bool,
}

impl BitOr for ModifierCombination {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self::from_bits(self.into_bits() | rhs.into_bits())
    }
}

impl BitAnd for ModifierCombination {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self::from_bits(self.into_bits() & rhs.into_bits())
    }
}

impl BitAndAssign for ModifierCombination {
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs;
    }
}

impl BitOrAssign for ModifierCombination {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl Not for ModifierCombination {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self::from_bits(!self.into_bits())
    }
}

impl ModifierCombination {
    pub const LCTRL: Self = Self::new().with_left_ctrl(true);
    pub const LSHIFT: Self = Self::new().with_left_shift(true);
    pub const LALT: Self = Self::new().with_left_alt(true);
    pub const LGUI: Self = Self::new().with_left_gui(true);

    pub const RCTRL: Self = Self::new().with_right_ctrl(true);
    pub const RSHIFT: Self = Self::new().with_right_shift(true);
    pub const RALT: Self = Self::new().with_right_alt(true);
    pub const RGUI: Self = Self::new().with_right_gui(true);

    pub const fn new_from(right: bool, gui: bool, alt: bool, shift: bool, ctrl: bool) -> Self {
        if right {
            ModifierCombination::new()
                .with_right_gui(gui)
                .with_right_alt(alt)
                .with_right_shift(shift)
                .with_right_ctrl(ctrl)
        } else {
            ModifierCombination::new()
                .with_left_gui(gui)
                .with_left_alt(alt)
                .with_left_shift(shift)
                .with_left_ctrl(ctrl)
        }
    }

    pub const fn new_from_vals(
        left_ctrl: bool,
        left_shift: bool,
        left_alt: bool,
        left_gui: bool,
        right_ctrl: bool,
        right_shift: bool,
        right_alt: bool,
        right_gui: bool,
    ) -> Self {
        ModifierCombination::new()
            .with_left_ctrl(left_ctrl)
            .with_left_shift(left_shift)
            .with_left_alt(left_alt)
            .with_left_gui(left_gui)
            .with_right_ctrl(right_ctrl)
            .with_right_shift(right_shift)
            .with_right_alt(right_alt)
            .with_right_gui(right_gui)
    }

    /// Convert current modifier into packed bits:
    ///
    /// | bit4 | bit3 | bit2 | bit1 | bit0 |
    /// | --- | --- | --- | --- | --- |
    /// | L/R | GUI | ALT |SHIFT| CTRL|
    ///
    /// WARN: Since the packed version cannot represent the state that BOTH left and right modifier is present,
    /// the left side has higher priority
    pub const fn into_packed_bits(self) -> u8 {
        let bits = self.into_bits();
        if bits == 0 {
            return 0;
        }
        let left_bits = bits & 0x0F; // Extract left side modifiers (bits 0-3)
        let right_bits = bits >> 4; // Extract right side modifiers (bits 4-7)

        // If left side has any modifiers, use left; otherwise use right with bit 4 set
        if left_bits != 0 {
            left_bits
        } else {
            right_bits | 0x10 // Set bit 4 to indicate right side
        }
    }

    /// Convert packed bits back into ModifierCombination:
    ///
    /// | bit4 | bit3 | bit2 | bit1 | bit0 |
    /// | --- | --- | --- | --- | --- |
    /// | L/R | GUI | ALT |SHIFT| CTRL|
    ///
    /// If bit4 is 0, modifiers are applied to left side, otherwise right side
    pub const fn from_packed_bits(bits: u8) -> Self {
        let modifier_bits = bits & 0x0F; // Extract modifier bits (0-3)
        let is_right = (bits & 0x10) != 0; // Check if bit 4 is set (right side)

        if is_right {
            Self::from_bits(modifier_bits << 4) // Shift to right side position
        } else {
            Self::from_bits(modifier_bits) // Use as left side
        }
    }
}
