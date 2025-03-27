use trouble_host::{prelude::*, BondInformation, LongTermKey};

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct BondInfo {
    pub(crate) slot_num: u8,
    pub(crate) removed: bool,
    pub(crate) info: BondInformation,
}

impl Default for BondInfo {
    fn default() -> Self {
        Self {
            slot_num: 0,
            removed: false,
            info: BondInformation {
                ltk: LongTermKey(0),
                address: BdAddr::default(),
            },
        }
    }
}
