use trouble_host::BondInformation;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct BondInfo {
    pub(crate) slot_num: u8,
    pub(crate) info: BondInformation,
}


