use darling::FromMeta;

/// List of functions that can be overwritten
#[derive(Debug, Clone, Copy, FromMeta)]
pub enum Overwritten {
    Usb,
    ChipConfig,
    ChipInit,
    Entry,
}
