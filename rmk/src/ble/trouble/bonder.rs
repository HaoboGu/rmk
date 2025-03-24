use trouble_host::BondInformation;

#[derive(Clone, Debug)]
pub(crate) struct BondInfo {
    pub(crate) slot_num: u8,
    pub(crate) info: BondInformation,
}


