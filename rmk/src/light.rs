use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

use bitfield_struct::bitfield;
use embassy_usb::class::hid::HidReader;
use embassy_usb::driver::Driver;
use serde::{Deserialize, Serialize};

use crate::hid::{HidError, HidReaderTrait};

#[bitfield(u8, defmt = cfg(feature = "defmt"))]
#[derive(Eq, PartialEq, Serialize, Deserialize)]

pub struct LedIndicator {
    #[bits(1)]
    pub(crate) num_lock: bool,
    #[bits(1)]
    pub(crate) caps_lock: bool,
    #[bits(1)]
    pub(crate) scroll_lock: bool,
    #[bits(1)]
    pub(crate) compose: bool,
    #[bits(1)]
    pub(crate) kana: bool,
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

pub(crate) struct UsbLedReader<'a, 'd, D: Driver<'d>> {
    hid_reader: &'a mut HidReader<'d, D, 1>,
}

impl<'a, 'd, D: Driver<'d>> UsbLedReader<'a, 'd, D> {
    pub(crate) fn new(hid_reader: &'a mut HidReader<'d, D, 1>) -> Self {
        Self { hid_reader }
    }
}

impl<'d, D: Driver<'d>> HidReaderTrait for UsbLedReader<'_, 'd, D> {
    type ReportType = LedIndicator;

    async fn read_report(&mut self) -> Result<Self::ReportType, HidError> {
        let mut buf = [0u8; 1];
        self.hid_reader.read(&mut buf).await.map_err(HidError::UsbReadError)?;

        Ok(LedIndicator::from_bits(buf[0]))
    }
}
