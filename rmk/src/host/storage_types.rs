use rmk_types::action::{EncoderAction, KeyAction};

use crate::COMBO_MAX_LENGTH;
use crate::fork::Fork;

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct KeymapKey {
    pub(crate) row: u8,
    pub(crate) col: u8,
    pub(crate) layer: u8,
    pub(crate) action: KeyAction,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct EncoderConfig {
    /// Encoder index
    pub(crate) idx: u8,
    /// Layer
    pub(crate) layer: u8,
    /// Encoder action
    pub(crate) action: EncoderAction,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct ComboData {
    /// Combo index
    pub(crate) idx: usize,
    /// Combo components
    pub(crate) actions: [KeyAction; COMBO_MAX_LENGTH],
    /// Combo output
    pub(crate) output: KeyAction,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct ForkData {
    /// Fork index
    pub(crate) idx: usize,
    /// Fork instance
    pub(crate) fork: Fork,
}
