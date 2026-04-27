#[cfg(feature = "vial")]
pub(crate) mod vial;

pub(crate) use vial::{VialService as HostService, run_ble_host};
