use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

use bitfield_struct::bitfield;
use serde::{Deserialize, Serialize};

/// Indicators defined in the HID spec 11.1
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum LedIndicatorType {
    NumLock,
    CapsLock,
    ScrollLock,
    Compose,
    Kana,
}

#[bitfield(u8, defmt = cfg(feature = "defmt"))]
#[derive(Eq, PartialEq, Serialize, Deserialize)]

pub struct LedIndicator {
    #[bits(1)]
    pub num_lock: bool,
    #[bits(1)]
    pub caps_lock: bool,
    #[bits(1)]
    pub scroll_lock: bool,
    #[bits(1)]
    pub compose: bool,
    #[bits(1)]
    pub kana: bool,
    #[bits(3)]
    _reserved: u8,
}

impl BitOr for LedIndicator {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self::from_bits(self.into_bits() | rhs.into_bits())
    }
}
impl BitAnd for LedIndicator {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self::from_bits(self.into_bits() & rhs.into_bits())
    }
}
impl Not for LedIndicator {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self::from_bits(!self.into_bits())
    }
}
impl BitAndAssign for LedIndicator {
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs;
    }
}
impl BitOrAssign for LedIndicator {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl LedIndicator {
    pub const NUM_LOCK: Self = Self::new().with_num_lock(true);
    pub const CAPS_LOCK: Self = Self::new().with_caps_lock(true);
    pub const SCROLL_LOCK: Self = Self::new().with_scroll_lock(true);
    pub const COMPOSE: Self = Self::new().with_compose(true);
    pub const KANA: Self = Self::new().with_kana(true);

    pub const fn new_from(num_lock: bool, caps_lock: bool, scroll_lock: bool, compose: bool, kana: bool) -> Self {
        Self::new()
            .with_num_lock(num_lock)
            .with_caps_lock(caps_lock)
            .with_scroll_lock(scroll_lock)
            .with_compose(compose)
            .with_kana(kana)
    }
}
