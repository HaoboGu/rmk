use core::ops::BitOr;

use bitfield_struct::bitfield;
use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

/// To represent all combinations of modifiers, at least 5 bits are needed.
/// 1 bit for Left/Right, 4 bits for modifier type. Represented in LSB format.
///
/// | bit4 | bit3 | bit2 | bit1 | bit0 |
/// | --- | --- | --- | --- | --- |
/// | L/R | GUI | ALT |SHIFT| CTRL|
#[bitfield(u8, order = Lsb, defmt = cfg(feature = "defmt"))]
#[derive(Serialize, Deserialize, MaxSize, Eq, PartialEq)]

pub struct ModifierCombination {
    #[bits(1)]
    pub ctrl: bool,
    #[bits(1)]
    pub shift: bool,
    #[bits(1)]
    pub alt: bool,
    #[bits(1)]
    pub gui: bool,
    #[bits(1)]
    pub right: bool,
    #[bits(3)]
    _reserved: u8,
}

impl BitOr for ModifierCombination {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self::from_bits(self.into_bits() | rhs.into_bits())
    }
}

pub const CTRL: ModifierCombination = ModifierCombination::new().with_ctrl(true);
pub const SHIFT: ModifierCombination = ModifierCombination::new().with_shift(true);
pub const ALT: ModifierCombination = ModifierCombination::new().with_alt(true);
pub const GUI: ModifierCombination = ModifierCombination::new().with_gui(true);
pub const RIGHT: ModifierCombination = ModifierCombination::new().with_right(true);

impl ModifierCombination {
    pub const fn new_from(right: bool, gui: bool, alt: bool, shift: bool, ctrl: bool) -> Self {
        ModifierCombination::new()
            .with_right(right)
            .with_gui(gui)
            .with_alt(alt)
            .with_shift(shift)
            .with_ctrl(ctrl)
    }

    pub fn from_hid_modifiers(modifiers: HidModifiers) -> Self {
        Self::new_from(
            modifiers.right_shift() || modifiers.right_ctrl() || modifiers.right_alt() || modifiers.right_gui(),
            modifiers.left_gui() || modifiers.right_gui(),
            modifiers.left_alt() || modifiers.right_alt(),
            modifiers.left_shift() || modifiers.right_shift(),
            modifiers.left_ctrl() || modifiers.right_ctrl(),
        )
    }

    /// Get modifier hid report bits from modifier combination
    pub fn to_hid_modifiers(self) -> HidModifiers {
        if !self.right() {
            HidModifiers::new()
                .with_left_ctrl(self.ctrl())
                .with_left_shift(self.shift())
                .with_left_alt(self.alt())
                .with_left_gui(self.gui())
        } else {
            HidModifiers::new()
                .with_right_ctrl(self.ctrl())
                .with_right_shift(self.shift())
                .with_right_alt(self.alt())
                .with_right_gui(self.gui())
        }
    }
}
